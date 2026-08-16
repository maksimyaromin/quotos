//! Launch at login, through SMAppService, available on macOS 13 and newer.
//! This is the OS's own login-item registry, surfaced in System Settings
//! under General, then Login Items.
//!
//! The OS is the single owner of this state. Quotos stores nothing. The
//! menu checkbox is drawn from [`status`] at build time and re-read after
//! every toggle, so a registration the OS refused reads as still off
//! instead of lying. The realistic way a registration gets refused is a
//! bare `cargo` or dev binary that is not an `.app` bundle, since
//! `SMAppService` only registers bundles. Registration applies to the app
//! bundle itself. The statusline helper copy in `statusline.rs` lives at a
//! different path on disk and is never registered.
//!
//! `ServiceManagement` is not linked by anything else in the dependency
//! tree, so the `#[link]` block below is the entire link directive.
//! `status_item_render`'s `text` module takes the same zero-new-crates approach
//! with Core Text.

/// What the OS currently says about Quotos's login item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginItemStatus {
    /// Registered and active. Quotos starts at login.
    Enabled,
    /// Registered, but macOS withholds it until the user approves it in
    /// System Settings.
    RequiresApproval,
    /// Not registered.
    Disabled,
    /// The OS cannot resolve this app as a registrable login item. This
    /// happens for an unbundled dev binary, or on a macOS version too old
    /// to have `SMAppService`.
    NotFound,
}

impl LoginItemStatus {
    /// Whether the login item exists at all from the user's point of view,
    /// which is what the menu checkbox shows. `RequiresApproval` counts:
    /// the item is registered and visible in System Settings, merely
    /// awaiting consent there. Unchecking the box for this state would
    /// misreport what toggling it again actually does, which is
    /// unregister rather than register.
    pub fn is_registered(self) -> bool {
        matches!(self, Self::Enabled | Self::RequiresApproval)
    }
}

/// `SMAppServiceStatus`'s raw values, per `ServiceManagement/SMAppService.h`.
/// `0` is `notRegistered`. An unknown future value also degrades to
/// [`LoginItemStatus::Disabled`], so the checkbox reads unchecked and
/// toggling it attempts a plain register, which is the correct recovery.
fn status_from_raw(raw: isize) -> LoginItemStatus {
    match raw {
        1 => LoginItemStatus::Enabled,
        2 => LoginItemStatus::RequiresApproval,
        3 => LoginItemStatus::NotFound,
        _ => LoginItemStatus::Disabled,
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSError;

    #[link(name = "ServiceManagement", kind = "framework")]
    extern "C" {}

    /// Returns `None` on a macOS version old enough to lack `SMAppService`,
    /// meaning before macOS 13. The class lookup is the availability check
    /// itself, so nothing here can panic on an older system.
    fn main_app_service() -> Option<Retained<AnyObject>> {
        let class = AnyClass::get(c"SMAppService")?;
        Some(unsafe { msg_send![class, mainAppService] })
    }

    pub fn status_raw() -> Option<isize> {
        let service = main_app_service()?;
        Some(unsafe { msg_send![&*service, status] })
    }

    pub fn set_registered(register: bool) -> Result<(), String> {
        let Some(service) = main_app_service() else {
            return Err("launch at login needs macOS 13 or newer".to_string());
        };
        let result: Result<(), Retained<NSError>> = unsafe {
            if register {
                msg_send![&*service, registerAndReturnError: _]
            } else {
                msg_send![&*service, unregisterAndReturnError: _]
            }
        };
        result.map_err(|error| error.localizedDescription().to_string())
    }
}

/// What the OS currently says. Read fresh on every call, never cached.
pub fn status() -> LoginItemStatus {
    #[cfg(target_os = "macos")]
    {
        platform::status_raw().map_or(LoginItemStatus::NotFound, status_from_raw)
    }
    #[cfg(not(target_os = "macos"))]
    {
        LoginItemStatus::NotFound
    }
}

/// Registers or unregisters Quotos as a login item. On success the change
/// is already durable in the OS's own registry, so read [`status`] back
/// for what to display rather than assuming the request took effect.
pub fn set_registered(register: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        platform::set_registered(register)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = register;
        Err("launch at login is a macOS feature".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_status_values_map_per_the_servicemanagement_header() {
        assert_eq!(status_from_raw(1), LoginItemStatus::Enabled);
        assert_eq!(status_from_raw(2), LoginItemStatus::RequiresApproval);
        assert_eq!(status_from_raw(3), LoginItemStatus::NotFound);
        assert_eq!(status_from_raw(0), LoginItemStatus::Disabled);
        // An unknown future value must degrade to "off", never crash or
        // read as registered.
        assert_eq!(status_from_raw(99), LoginItemStatus::Disabled);
    }

    #[test]
    fn registered_means_visible_in_login_items() {
        assert!(LoginItemStatus::Enabled.is_registered());
        assert!(LoginItemStatus::RequiresApproval.is_registered());
        assert!(!LoginItemStatus::Disabled.is_registered());
        assert!(!LoginItemStatus::NotFound.is_registered());
    }

    /// Read-only smoke of the real ObjC path: proves `ServiceManagement`
    /// actually links and the selectors resolve at runtime. Deliberately
    /// never calls `set_registered`. Mutating the login items of whatever
    /// machine runs the tests is not a test's business.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_real_status_call_answers_without_crashing() {
        let _ = status();
    }
}
