//! Makes the panel window a non-activating `NSPanel`, the AppKit type built
//! to hold keyboard focus without activating its application.
//!
//! A full-screen Space belongs to one application, and activating a
//! different one while a full-screen Space is frontmost makes macOS leave
//! that Space, the same transition Cmd-Tab produces. An ordinary `NSWindow`
//! can only become key while its application is active, so
//! `WebviewWindow::set_focus()` triggered that transition on every panel
//! open: it is `tao`'s `makeKeyAndOrderFront:` followed by
//! `activateIgnoringOtherApps: YES`. `NSWindowStyleMaskNonactivatingPanel`
//! lifts the constraint, but only on an `NSPanel`; set on a plain
//! `NSWindow` it is silently inert.
//!
//! Tauri and `tao` create a plain `NSWindow` subclass with no switch for
//! "make it a panel", so [`make_nonactivating_panel`] swaps the window's
//! class after creation, the same approach `tauri-nspanel` takes.
//! `object_setClass` is only safe when the new class is no larger than the
//! object's original allocation: `NSPanel` adds no instance storage over
//! `NSWindow`, the original window class is larger since it adds a
//! `focusable` ivar, and the replacement class declares no ivars of its
//! own, so the runtime size check in `make_nonactivating_panel` passes
//! today and fails closed if a future AppKit changes that. The original
//! class overrides `canBecomeKeyWindow` and `canBecomeMainWindow`, both
//! reimplemented below, and `sendEvent:`, whose body is a no-op unless
//! `isMovableByWindowBackground` is set, which Tauri never sets since this
//! app drags through `startDragging()` instead. The window delegate is a
//! separate object and stays attached, so `tao`'s `Focused`, `Moved` and
//! `Resized` events keep firing unchanged. `set_focusable()` becomes
//! unusable after the swap, since it writes the now-absent `focusable`
//! ivar by name; nothing here calls it.
//!
//! Setting the non-activating style bit on a plain `NSWindow` raises an
//! Objective-C exception, and an exception crossing the Rust FFI boundary
//! aborts the process, so the class swap is verified by reading the class
//! back before the style mask is ever written. The window already carries
//! a KVO isa-swizzle when `setup` runs; the class swap displaces it, which
//! is safe only because Tauri sets the content view once, during window
//! creation, before this ever runs.
//!
//! `QUOTOS_PANEL_MODE=window` restores the old activate-on-show path for a
//! same-session comparison without a rebuild. `-[NSApplication isActive]`
//! reads `true` for an accessory app regardless of which path ran, so
//! telling them apart needs `NSWorkspace.frontmostApplication` read from a
//! separate process.

#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
#[cfg(target_os = "macos")]
use objc2::{msg_send, sel};

/// Also the idempotency check: a window already of this class is left
/// alone.
#[cfg(target_os = "macos")]
const PANEL_CLASS_NAME: &std::ffi::CStr = c"QuotosNonActivatingPanel";

#[cfg(target_os = "macos")]
const NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL: usize = 1 << 7;

/// This window is always focusable, so this always answers yes. `tao`'s
/// own implementation instead reads a `focusable` ivar this class lacks.
#[cfg(target_os = "macos")]
extern "C" fn can_become_key_window(_this: &AnyObject, _sel: Sel) -> Bool {
    Bool::YES
}

/// A menu bar popover is never the application's main window.
#[cfg(target_os = "macos")]
extern "C" fn can_become_main_window(_this: &AnyObject, _sel: Sel) -> Bool {
    Bool::NO
}

#[cfg(target_os = "macos")]
fn log_class_before_conversion(object: &AnyObject) {
    if std::env::var_os("QUOTOS_DEBUG_POS").is_some() {
        eprintln!(
            "quotos-pos: window class before conversion = {}",
            object.class().name().to_string_lossy()
        );
    }
}

#[cfg(target_os = "macos")]
fn panel_class() -> Option<&'static AnyClass> {
    if let Some(existing) = AnyClass::get(PANEL_CLASS_NAME) {
        return Some(existing);
    }
    let superclass = AnyClass::get(c"NSPanel")?;
    let mut builder = ClassBuilder::new(PANEL_CLASS_NAME, superclass)?;
    unsafe {
        // extern "C" fn(_, _) -> _ keeps the pointer type higher-ranked;
        // spelling the lifetime out produces a type MethodImplementation
        // rejects. tao's own class declaration uses the same cast.
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

/// Idempotent, and safe to call on every show. `false` means the show path
/// must keep using the old activate-the-app route rather than silently
/// ending up with a window that can never take keyboard focus.
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
    log_class_before_conversion(object);
    if !std::ptr::eq(object.class(), class) {
        if class.instance_size() > object.class().instance_size() {
            return false;
        }
        unsafe { objc2::ffi::object_setClass(ptr as *mut AnyObject, class) };
        // A KVO isa-swizzle can put the original class back between calls,
        // so the swap is confirmed by reading it back rather than assumed.
        if !std::ptr::eq(object.class(), class) {
            return false;
        }
    }

    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    unsafe {
        let mask: usize = msg_send![ns_window, styleMask];
        let _: () =
            msg_send![ns_window, setStyleMask: mask | NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL];
        // NSPanel defaults to hiding when its application deactivates;
        // the app's detached mode depends on this panel staying visible
        // while another application is active.
        let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
        // becomesKeyOnlyIfNeeded would suppress windowDidResignKey until
        // something inside the panel demanded key focus, and
        // click-away-to-close depends on that event.
        let _: () = msg_send![ns_window, setBecomesKeyOnlyIfNeeded: false];
    }
    true
}

#[cfg(not(target_os = "macos"))]
pub fn make_nonactivating_panel(_window: &tauri::WebviewWindow) -> bool {
    false
}

/// Shows and focuses the panel without activating the application.
/// `orderFrontRegardless()` fronts the window regardless of which
/// application is active; `makeKeyWindow()` then gives it keyboard focus,
/// which only a non-activating panel can accept while its app is inactive.
/// Deliberately not `WebviewWindow::set_focus()`, whose second half is
/// `activateIgnoringOtherApps: YES`.
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
