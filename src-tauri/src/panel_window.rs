#[cfg(target_os = "macos")]
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
#[cfg(target_os = "macos")]
use objc2::{msg_send, sel};

#[cfg(target_os = "macos")]
const PANEL_CLASS_NAME: &std::ffi::CStr = c"QuotosNonActivatingPanel";

#[cfg(target_os = "macos")]
const NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL: usize = 1 << 7;

#[cfg(target_os = "macos")]
extern "C" fn can_become_key_window(_this: &AnyObject, _sel: Sel) -> Bool {
    Bool::YES
}

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
        if !std::ptr::eq(object.class(), class) {
            return false;
        }
    }

    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    unsafe {
        let mask: usize = msg_send![ns_window, styleMask];
        let _: () =
            msg_send![ns_window, setStyleMask: mask | NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL];
        let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
        let _: () = msg_send![ns_window, setBecomesKeyOnlyIfNeeded: false];
    }
    true
}

#[cfg(not(target_os = "macos"))]
pub fn make_nonactivating_panel(_window: &tauri::WebviewWindow) -> bool {
    false
}

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
