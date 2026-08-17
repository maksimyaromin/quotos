#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginItemStatus {
    Enabled,
    RequiresApproval,
    Disabled,
    NotFound,
}

impl LoginItemStatus {
    pub fn is_registered(self) -> bool {
        matches!(self, Self::Enabled | Self::RequiresApproval)
    }
}

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
    unsafe extern "C" {}

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
        assert_eq!(status_from_raw(99), LoginItemStatus::Disabled);
    }

    #[test]
    fn registered_means_visible_in_login_items() {
        assert!(LoginItemStatus::Enabled.is_registered());
        assert!(LoginItemStatus::RequiresApproval.is_registered());
        assert!(!LoginItemStatus::Disabled.is_registered());
        assert!(!LoginItemStatus::NotFound.is_registered());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_real_status_call_answers_without_crashing() {
        let _ = status();
    }
}
