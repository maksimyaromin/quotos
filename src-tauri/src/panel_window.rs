//! Makes the panel window a non-activating `NSPanel`, which is what stops
//! a tray click from taking focus away from a full-screen Space.
//!
//! ## Why activation is the trigger
//!
//! A full-screen Space belongs to one application. Activating a different
//! application while a full-screen Space is frontmost makes macOS leave
//! that Space, the same visible transition Cmd-Tab produces. Opening the
//! panel used to do exactly that on every open:
//! `WebviewWindow::set_focus()` bottoms out in `tao`'s `util::set_focus`,
//! which is `makeKeyAndOrderFront:` followed by
//! `activateIgnoringOtherApps: YES`. That call cannot simply be removed
//! from an ordinary `NSWindow`: without it the window is never key, which
//! breaks click-away-to-close, since that rides on `Focused(false)`, along
//! with the rename field and the sign-in field. An ordinary `NSWindow` can
//! only be key while its application is active, so "key" and "do not
//! activate" are incompatible for that class specifically.
//!
//! `NSPanel` with `NSWindowStyleMaskNonactivatingPanel` is the AppKit type
//! built to break that tie. It can hold keyboard focus while another
//! application stays active, which is why every menu-bar popover and every
//! Spotlight-alike can be typed into over a full-screen app without the
//! Space sliding away.
//!
//! The fix is one substitution: the window becomes a non-activating panel,
//! see [`make_nonactivating_panel`], and the show path stops calling
//! `set_focus()`, meaning it stops activating the application, using
//! [`order_front_without_activating`] instead.
//!
//! `QUOTOS_PANEL_MODE=window` restores the old `NSWindow`-plus-activate
//! path for a same-session comparison without a rebuild.
//! `QUOTOS_DEBUG_POS=1` prints `app_active` for that comparison, though
//! `-[NSApplication isActive]` read from inside an accessory app reports
//! `true` regardless of whether this app actually activated, so that field
//! alone cannot distinguish the two paths.
//! `NSWorkspace.frontmostApplication`, read from a separate process, does
//! distinguish them.
//!
//! ## Two failure modes worth keeping in mind
//!
//! Setting the non-activating style bit without first swapping the class
//! does not quietly do nothing. `-[NSWindow setStyleMask:]` raises for
//! that flag on a plain `NSWindow`, and an Objective-C exception crossing
//! the Rust FFI boundary aborts the process outright, since it is a panic
//! in a function that cannot unwind. This is why the swap is verified by
//! reading the class back before the mask is ever written.
//!
//! The window already carries a KVO isa-swizzle when `setup` runs. Its
//! real class overrides `setContentView:` along with KVO's usual `class`,
//! `dealloc`, and `_isKVOA`. The class swap below displaces that
//! isa-swizzle, so a later `contentView` change would go unobserved and
//! AppKit may log "deallocated while key value observers were still
//! registered" at exit. Tauri sets the content view once, during window
//! creation and therefore before this runs, so there is no later change to
//! miss.
//!
//! ## Why swapping the class is safe here
//!
//! Tauri and `tao` create a plain `NSWindow` subclass with no
//! configuration switch for "make it a panel", so the class is swapped
//! after creation, the same approach `tauri-nspanel` takes. Three things
//! make that sound:
//!
//! * `object_setClass` is only safe if the new class is no larger than
//!   what the object was allocated with. `NSPanel` adds no instance
//!   storage of its own over `NSWindow`, while the original window class
//!   is larger, since it adds a `focusable` ivar. The replacement class
//!   declares no ivars either, so the size check
//!   [`make_nonactivating_panel`] performs at runtime passes, and a future
//!   AppKit where `NSPanel` gained storage would fail closed instead of
//!   corrupting memory. `objc2`'s own `AnyObject::set_class` is not used
//!   here: it asserts equal sizes and requires the new class to be a
//!   subclass of the old one, neither of which holds, so the raw runtime
//!   call is used directly.
//! * The original window class overrides exactly three methods:
//!   `canBecomeKeyWindow` and `canBecomeMainWindow`, both reimplemented
//!   below, and `sendEvent:`, whose entire body is a no-op unless
//!   `isMovableByWindowBackground` is set. Tauri never sets it, and this
//!   app's header drag goes through `startDragging()`, which calls
//!   `performWindowDragWithEvent:` directly.
//! * The window delegate is a separate object and stays attached, so
//!   `tao`'s `Focused`, `Moved`, and `Resized` events keep firing exactly
//!   as before, including the `windowDidResignKey` that
//!   click-away-to-close depends on.
//!
//! One `tao` API becomes unusable after the swap: `set_focusable()` writes
//! the `focusable` ivar by name and would not find it. Nothing in this app
//! calls it, since the window is always focusable.

#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
#[cfg(target_os = "macos")]
use objc2::{msg_send, sel};

/// The runtime-registered `NSPanel` subclass' name. Also the idempotency
/// check: a window already of this class is left alone.
#[cfg(target_os = "macos")]
const PANEL_CLASS_NAME: &std::ffi::CStr = c"QuotosNonActivatingPanel";

/// `NSWindowStyleMaskNonactivatingPanel`. Only honored on an `NSPanel`.
/// Set on a plain `NSWindow` it is silently inert, which is the whole
/// reason the class swap above has to happen first.
#[cfg(target_os = "macos")]
const NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL: usize = 1 << 7;

/// Always key-able. `TaoWindow`'s own version reads a `focusable` ivar
/// that does not exist on this class. This window is always focusable, so
/// the constant answer is also the correct one.
#[cfg(target_os = "macos")]
extern "C" fn can_become_key_window(_this: &AnyObject, _sel: Sel) -> Bool {
    Bool::YES
}

/// Never the main window. A menu bar popover is an accessory surface, not
/// the application's document window, and claiming main-ness is one more
/// way to look to AppKit like an app that wants to be brought forward.
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
        // Cast as extern "C" fn(_, _) -> _ rather than writing the fully
        // spelled-out type. Spelling the lifetime out produces a
        // non-higher-ranked fn pointer, which MethodImplementation does
        // not accept. tao's own class declaration uses the same shape.
        builder.add_method(
            sel!(canBecomeKeyWindow),
            can_become_key_window as extern "C" fn(_, _) -> _,
        );
        builder.add_method(
            sel!(canBecomeMainWindow),
            can_become_main_window as extern "C" fn(_, _) -> _,
        );
    }
    Some(builder.register())
}

/// Turns the panel window into a non-activating `NSPanel`, in place.
/// Idempotent, and safe to call on every show. Returns whether the window
/// is now a non-activating panel. `false` means the show path must keep
/// using the old activate-the-app route rather than silently ending up
/// with a window that can never take keyboard focus.
///
/// Opt out with `QUOTOS_PANEL_MODE=window`. See the module doc for why
/// this exists: it lets the one behavioral change be compared against a
/// real full-screen Space without a rebuild.
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
    let Ok(ptr) = window.ns_window() else {
        return false;
    };
    if ptr.is_null() {
        return false;
    }
    let Some(class) = panel_class() else {
        return false;
    };

    let object: &AnyObject = unsafe { &*(ptr as *const AnyObject) };
    if std::env::var_os("QUOTOS_DEBUG_POS").is_some() {
        eprintln!(
            "quotos-pos: window class before conversion = {}",
            object.class().name().to_string_lossy()
        );
    }
    if !std::ptr::eq(object.class(), class) {
        // This checks the one real memory-safety precondition directly,
        // rather than trusting the reasoning in the module doc: never grow
        // an already-allocated object. The original window class is
        // bigger than NSPanel, so this passes today. A future AppKit
        // where NSPanel gained storage would fail closed here and keep
        // the old activate-the-app path instead of corrupting memory.
        if class.instance_size() > object.class().instance_size() {
            return false;
        }
        unsafe { objc2::ffi::object_setClass(ptr as *mut AnyObject, class) };
        // Reads the class back rather than assuming the swap took. The
        // style-mask write below is only safe on a real panel, since
        // AppKit aborts the process if the non-activating bit is set on
        // anything else, so this has to be a checked fact rather than an
        // expectation. A KVO isa-swizzle can also legitimately put the
        // original class back between calls, which is exactly the case
        // this catches on a later call.
        if !std::ptr::eq(object.class(), class) {
            return false;
        }
    }

    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    unsafe {
        let mask: usize = msg_send![ns_window, styleMask];
        let _: () =
            msg_send![ns_window, setStyleMask: mask | NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL];
        // NSPanel's documented default is to vanish when its application
        // is deactivated. This panel's whole point is to stay put while
        // another application keeps working, and the app's detached mode
        // depends on it outright. Set explicitly rather than trusting
        // whatever the flag happened to be left at by NSWindow's own
        // initializer.
        let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
        // becomesKeyOnlyIfNeeded would keep the panel from taking key
        // focus until something inside it demanded it, which would also
        // mean no windowDidResignKey, and click-away-to-close rides on
        // exactly that.
        let _: () = msg_send![ns_window, setBecomesKeyOnlyIfNeeded: false];
    }
    true
}

#[cfg(not(target_os = "macos"))]
pub fn make_nonactivating_panel(_window: &tauri::WebviewWindow) -> bool {
    false
}

/// Shows and focuses the panel without activating the application, the
/// half of the fix that actually keeps the user's current Space from
/// changing.
///
/// `orderFrontRegardless()` puts the window at the front of its level here
/// and now, whoever is active. `makeKeyWindow()` then gives it keyboard
/// focus, which only a non-activating panel can accept while its app is
/// inactive. This is deliberately not `WebviewWindow::set_focus()`, whose
/// second half is `activateIgnoringOtherApps: YES`.
#[cfg(target_os = "macos")]
pub fn order_front_without_activating(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::MainThreadMarker;

    let (Some(_mtm), Ok(ptr)) = (MainThreadMarker::new(), window.ns_window()) else {
        return;
    };
    if ptr.is_null() {
        return;
    }
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    ns_window.orderFrontRegardless();
    ns_window.makeKeyWindow();
}

#[cfg(not(target_os = "macos"))]
pub fn order_front_without_activating(_window: &tauri::WebviewWindow) {}
