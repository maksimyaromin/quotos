//! R4-1: making the panel window a **non-activating `NSPanel`**, which is
//! what stops a tray click from throwing the captain out of a full-screen
//! Space.
//!
//! ## What the 2026-08-15 screencast shows, and why the previous fixes missed
//!
//! Round 3 (`1f0d55d`, `d6f91ac`) attacked this as a *window membership*
//! problem: give the window `CanJoinAllSpaces | FullScreenAuxiliary`, raise it
//! to `NSStatusWindowLevel`, and `orderFrontRegardless()` it. All three are
//! necessary and all three are kept. None of them is the trigger.
//!
//! The captain's video (`data/quotos-panel-ux-p1/evidence/video2-…`) shows
//! macOS running its **Space-transition animation** at t≈6.6 — the full-screen
//! browser slides away and the panel opens on the desktop Space behind it.
//! That animation is not something a window's collection behaviour can cause.
//! It is what macOS does when a *different application becomes active* while a
//! full-screen Space is frontmost: a full-screen Space belongs to one
//! application, so activating another one leaves it. Cmd-Tab does exactly the
//! same thing, visibly, for the same reason.
//!
//! And Quotos asks for precisely that, on every single open.
//! `WebviewWindow::set_focus()` bottoms out in `tao`'s `util::set_focus`,
//! which is `makeKeyAndOrderFront:` **followed by
//! `activateIgnoringOtherApps: YES`** (`tao-0.35.3/.../util/async.rs:231`).
//! Round 3 knew about that call and deliberately kept it, because removing it
//! left the window non-key — and a non-key window breaks click-away-to-close
//! (it rides on `Focused(false)`), the rename field and the sign-in field.
//! That was a real trade, honestly made; this module is what removes the need
//! to make it at all.
//!
//! ## The counterfactual
//!
//! An ordinary `NSWindow` can only be key while its application is active, so
//! "key" and "don't activate" are genuinely incompatible — *for an
//! `NSWindow`*. `NSPanel` with `NSWindowStyleMaskNonactivatingPanel` is the
//! AppKit type that exists specifically to break that tie: it can hold
//! keyboard focus while another application stays active. It is what every
//! menu-bar popover and every Spotlight-alike is; it is why they can be typed
//! into over a full-screen app without the Space sliding away.
//!
//! So the change is one substitution, not a new mechanism:
//!
//! * the window becomes a non-activating panel (`make_nonactivating_panel`),
//! * and the show path stops calling `set_focus()` — i.e. stops activating the
//!   application — using [`order_front_without_activating`] instead.
//!
//! **What would falsify it:** if a tray click from a full-screen Space still
//! slides the Space away while `NSApp.isActive` stays `false` through the
//! click, then activation was not the trigger and the cause is elsewhere.
//! `QUOTOS_DEBUG_POS=1` prints `app_active` in exactly that window, and
//! `QUOTOS_PANEL_MODE=window` restores the old `NSWindow`-plus-activate path
//! for a same-session A/B without a rebuild.
//!
//! **What is verified from inside this machine's fence, and what is not.** The
//! full-screen end-to-end is the captain's to confirm — an agent here may not
//! put an app into full screen. What *is* checked here (see `RESULT.md`) is
//! the mechanism, not the symptom, as a same-machine A/B against
//! `QUOTOS_PANEL_MODE=window`:
//!
//! | | frontmost app after the panel opens | panel `isKeyWindow` |
//! |---|---|---|
//! | old path (`=window`) | **quotos-app** — it stole activation | true |
//! | this path | **unchanged** (whatever was in front) | true |
//!
//! Read from *another* process (`NSWorkspace.frontmostApplication`), because
//! `-[NSApplication isActive]` read from inside an accessory app reports
//! `true` in **both** cases and is therefore worthless as a discriminator here
//! — a trap worth knowing about before trusting the `app_active` field in the
//! `QUOTOS_DEBUG_POS` trace. `tao`'s `Focused(true)` (i.e.
//! `windowDidBecomeKey`) still arrives on this path with another app frontmost,
//! which an ordinary `NSWindow` could not do. Those are the exact conditions
//! under which macOS has no reason to leave a full-screen Space; that it then
//! doesn't is the captain's observation to make.
//!
//! Two things measured along the way, both worth not rediscovering:
//!
//! * Setting the non-activating bit **without** the class swap does not
//!   quietly do nothing — `-[NSWindow setStyleMask:]` raises, and an ObjC
//!   exception crossing the Rust FFI boundary aborts the process outright
//!   ("panic in a function that cannot unwind"). That is why the swap is
//!   verified by reading the class back before the mask is ever written.
//! * The window already carries a KVO isa-swizzle when `setup` runs — its real
//!   class is `NSKVONotifying_TaoWindow`, overriding `setContentView:` (plus
//!   KVO's usual `class`/`dealloc`/`_isKVOA`). The swap displaces it, so a
//!   later `contentView` change would go unobserved and AppKit may log the
//!   familiar "deallocated while key value observers were still registered"
//!   line at exit. Tauri sets the content view once, during window creation
//!   and therefore before this runs, so there is no later change to miss.
//!
//! ## Why swapping the class is safe here
//!
//! Tauri/`tao` create a plain `NSWindow` subclass (`TaoWindow`) and there is no
//! configuration switch for "make it a panel", so the class is swapped after
//! creation — the same approach `tauri-nspanel` takes. Three things make that
//! sound rather than a stunt, and each is checked rather than assumed:
//!
//! * **Instance size.** `object_setClass` is only safe if the new class is no
//!   larger than what the object was allocated with. Measured on this machine:
//!   `class_getInstanceSize(NSWindow) == class_getInstanceSize(NSPanel) == 520`
//!   — `NSPanel` adds no storage of its own — and `TaoWindow` is *larger*
//!   still (it adds a `focusable` ivar). The replacement class declares no
//!   ivars, so it is 520 too. (`objc2`'s own `AnyObject::set_class` is not used
//!   for this: it `debug_assert!`s *equal* sizes and requires the new class to
//!   be a subclass of the old one, neither of which holds — the raw runtime
//!   call is the honest one to reach for.)
//! * **The methods being replaced.** `TaoWindow` overrides exactly three:
//!   `canBecomeKeyWindow`/`canBecomeMainWindow` (both re-implemented below) and
//!   `sendEvent:`, whose entire body is a no-op unless
//!   `isMovableByWindowBackground` is set — Tauri never sets it (nothing in
//!   `tauri`/`tauri-runtime-wry` calls `with_movable_by_window_background`), and
//!   this app's header drag goes through `startDragging()`, which calls
//!   `performWindowDragWithEvent:` directly.
//! * **What is *not* touched.** The window delegate is a separate object and
//!   stays attached, so `tao`'s `Focused`/`Moved`/`Resized` events keep firing
//!   exactly as before — including the `windowDidResignKey` that
//!   click-away-to-close depends on.
//!
//! One `tao` API does become unusable after the swap: `set_focusable()` writes
//! the `focusable` ivar by name and would not find it. Nothing in this app
//! calls it (the window is focusable, always).

#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
#[cfg(target_os = "macos")]
use objc2::{msg_send, sel};

/// The runtime-registered `NSPanel` subclass' name. Also the idempotency
/// check: a window already of this class is left alone.
#[cfg(target_os = "macos")]
const PANEL_CLASS_NAME: &std::ffi::CStr = c"QuotosNonActivatingPanel";

/// `NSWindowStyleMaskNonactivatingPanel`. Only honoured on an `NSPanel`; set
/// on a plain `NSWindow` it is silently inert, which is the whole reason the
/// class swap above has to happen first.
#[cfg(target_os = "macos")]
const NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL: usize = 1 << 7;

/// Always key-able. `TaoWindow`'s own version reads a `focusable` ivar that
/// does not exist on this class; this window is always focusable, so the
/// constant answer is also the correct one.
#[cfg(target_os = "macos")]
extern "C" fn can_become_key_window(_this: &AnyObject, _sel: Sel) -> Bool {
    Bool::YES
}

/// Never the *main* window. A menu bar popover is an accessory surface, not
/// the application's document window — and claiming main-ness is one more way
/// to look to AppKit like an app that wants to be brought forward.
#[cfg(target_os = "macos")]
extern "C" fn can_become_main_window(_this: &AnyObject, _sel: Sel) -> Bool {
    Bool::NO
}

#[cfg(target_os = "macos")]
fn panel_class() -> Option<&'static AnyClass> {
    if let Some(existing) = AnyClass::get(PANEL_CLASS_NAME) {
        return Some(existing);
    }
    let superclass = AnyClass::get(c"NSPanel")?;
    let mut builder = ClassBuilder::new(PANEL_CLASS_NAME, superclass)?;
    unsafe {
        // `as extern "C" fn(_, _) -> _` rather than the fully-written type:
        // spelling the lifetime out produces a non-higher-ranked fn pointer,
        // which `MethodImplementation` doesn't accept. (Same shape `tao`'s own
        // class declaration uses.)
        builder.add_method(sel!(canBecomeKeyWindow), can_become_key_window as extern "C" fn(_, _) -> _);
        builder.add_method(sel!(canBecomeMainWindow), can_become_main_window as extern "C" fn(_, _) -> _);
    }
    Some(builder.register())
}

/// Turns the panel window into a non-activating `NSPanel`, in place.
/// Idempotent, and safe to call on every show. Returns whether the window is
/// (now) a non-activating panel — `false` means the show path must keep using
/// the old activate-the-app route rather than silently ending up with a window
/// that can never take keyboard focus.
///
/// Opt out with `QUOTOS_PANEL_MODE=window` (see the module doc: this exists so
/// the one behavioural change can be A/B'd against a real full-screen Space
/// without a rebuild).
#[cfg(target_os = "macos")]
pub fn make_nonactivating_panel(window: &tauri::WebviewWindow) -> bool {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::MainThreadMarker;

    if std::env::var("QUOTOS_PANEL_MODE").ok().as_deref() == Some("window") {
        return false;
    }
    if MainThreadMarker::new().is_none() {
        return false;
    }
    let Ok(ptr) = window.ns_window() else { return false };
    if ptr.is_null() {
        return false;
    }
    let Some(class) = panel_class() else { return false };

    let object: &AnyObject = unsafe { &*(ptr as *const AnyObject) };
    if std::env::var_os("QUOTOS_DEBUG_POS").is_some() {
        eprintln!("quotos-pos: window class before conversion = {}", object.class().name().to_string_lossy());
    }
    if !std::ptr::eq(object.class(), class) {
        // Guards the one real memory-safety precondition rather than trusting
        // the reasoning in the module doc: never grow an already-allocated
        // object. (`TaoWindow` is bigger than `NSPanel`, so this passes; a
        // future AppKit where `NSPanel` gained storage would fail closed and
        // keep the old activate-the-app path instead of corrupting memory.)
        if class.instance_size() > object.class().instance_size() {
            return false;
        }
        unsafe { objc2::ffi::object_setClass(ptr as *mut AnyObject, class) };
        // Read back rather than assume. The style-mask write below is only
        // safe on a real panel — AppKit *aborts the process* if the
        // non-activating bit is set on anything else (measured, see the module
        // doc) — so "the swap took" has to be a checked fact, not an
        // expectation. KVO can also legitimately put the original class back
        // (see the module doc), which is exactly the case this catches on a
        // later call.
        if !std::ptr::eq(object.class(), class) {
            return false;
        }
    }

    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    unsafe {
        let mask: usize = msg_send![ns_window, styleMask];
        let _: () = msg_send![ns_window, setStyleMask: mask | NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL];
        // `NSPanel`'s documented default is to vanish when its application is
        // deactivated. This panel's whole point is to stay put while another
        // application keeps working, and detached mode (I7) depends on it
        // outright. Set explicitly rather than trusting whatever the flag
        // happened to be left at by `NSWindow`'s own initialiser.
        let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
        // `becomesKeyOnlyIfNeeded` would keep the panel from taking key focus
        // until something inside it demanded it — which would also mean no
        // `windowDidResignKey`, and click-away-to-close rides on exactly that.
        let _: () = msg_send![ns_window, setBecomesKeyOnlyIfNeeded: false];
    }
    true
}

#[cfg(not(target_os = "macos"))]
pub fn make_nonactivating_panel(_window: &tauri::WebviewWindow) -> bool {
    false
}

/// Shows and focuses the panel **without activating the application** — the
/// half of the fix that actually keeps the captain on his Space.
///
/// `orderFrontRegardless()` puts the window at the front of its level here and
/// now, whoever is active; `makeKeyWindow()` then gives it keyboard focus,
/// which only a non-activating panel can accept while its app is inactive.
/// Deliberately *not* `WebviewWindow::set_focus()`, whose second half is
/// `activateIgnoringOtherApps: YES`.
#[cfg(target_os = "macos")]
pub fn order_front_without_activating(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::MainThreadMarker;

    let (Some(_mtm), Ok(ptr)) = (MainThreadMarker::new(), window.ns_window()) else { return };
    if ptr.is_null() {
        return;
    }
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    ns_window.orderFrontRegardless();
    ns_window.makeKeyWindow();
}

#[cfg(not(target_os = "macos"))]
pub fn order_front_without_activating(_window: &tauri::WebviewWindow) {}
