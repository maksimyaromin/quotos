//! Makes the panel window a non-activating `NSPanel`, the AppKit type built
//! to hold keyboard focus without activating its application. See
//! docs/platform-constraints.md for the class swap and its safety proof.

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

/// `orderFrontRegardless()` fronts the window without activating the app;
/// `makeKeyWindow()` then gives it focus, which only a non-activating panel
/// accepts while inactive. Not `WebviewWindow::set_focus()`, which activates.
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
