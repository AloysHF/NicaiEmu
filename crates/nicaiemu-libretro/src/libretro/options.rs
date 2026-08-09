// Core option registration and parsing for the NicaiEmu libretro core.

use super::callbacks;
use super::constants::*;
use super::types::retro_variable;
use std::ffi::{c_void, CStr};
use std::ptr;

/// Option keys exposed to the frontend.
pub const OPTION_VOLUME: &CStr = c"nicaiemu_volume";
pub const OPTION_REPEAT_DELAY: &CStr = c"nicaiemu_repeat_delay";
pub const OPTION_REPEAT_PERIOD: &CStr = c"nicaiemu_repeat_period";
pub const OPTION_TOUCH_INPUT: &CStr = c"nicaiemu_touch_input";
pub const OPTION_DEBUG_LOGGING: &CStr = c"nicaiemu_debug_logging";

/// Resolved core option values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreOptions {
    pub volume: u32,
    pub repeat_delay: u32,
    pub repeat_period: u32,
    pub touch_input: bool,
    pub debug_logging: bool,
}

impl Default for CoreOptions {
    fn default() -> Self {
        Self {
            volume: 100,
            repeat_delay: 10,
            repeat_period: 15,
            touch_input: true,
            debug_logging: false,
        }
    }
}

/// Register the core's configurable options with the frontend.
///
/// Uses the legacy `RETRO_ENVIRONMENT_SET_VARIABLES` interface, which every
/// libretro frontend supports. Each value string is "Description; " plus a
/// pipe-separated list of choices whose first entry is the default.
pub fn set_core_options() {
    let variables = core_option_variables();

    // The frontend copies the data during the call, so a stack array is fine.
    callbacks::environment(
        RETRO_ENVIRONMENT_SET_VARIABLES,
        variables.as_ptr() as *const _ as *mut c_void,
    );
}

/// The option list advertised to the frontend, terminated by a null entry.
fn core_option_variables() -> [retro_variable; 6] {
    [
        retro_variable {
            key: OPTION_VOLUME.as_ptr(),
            value: c"Audio Volume (%); 100|90|80|70|60|50|40|30|20|10|0".as_ptr(),
        },
        retro_variable {
            key: OPTION_REPEAT_DELAY.as_ptr(),
            value: c"Key Auto-Repeat Delay (frames); 10|0|2|4|6|8|12|16|20|24|30|45|60".as_ptr(),
        },
        retro_variable {
            key: OPTION_REPEAT_PERIOD.as_ptr(),
            value: c"Key Auto-Repeat Period (frames); 15|1|2|3|4|5|6|8|10|12|20|30".as_ptr(),
        },
        retro_variable {
            key: OPTION_TOUCH_INPUT.as_ptr(),
            value: c"Touch/Pointer Input; enabled|disabled".as_ptr(),
        },
        retro_variable {
            key: OPTION_DEBUG_LOGGING.as_ptr(),
            value: c"CPU/HLE Debug Logging; disabled|enabled".as_ptr(),
        },
        // Terminator.
        retro_variable {
            key: ptr::null(),
            value: ptr::null(),
        },
    ]
}

/// Read a single core option value from the frontend by key.
///
/// Returns `None` if the option is unset or the frontend does not support
/// variables.
pub fn get_core_option(key: &CStr) -> Option<String> {
    let mut variable = retro_variable {
        key: key.as_ptr(),
        value: ptr::null(),
    };
    let ok = callbacks::environment(
        RETRO_ENVIRONMENT_GET_VARIABLE,
        &mut variable as *mut _ as *mut c_void,
    );
    if ok && !variable.value.is_null() {
        unsafe {
            CStr::from_ptr(variable.value)
                .to_str()
                .ok()
                .map(str::to_owned)
        }
    } else {
        None
    }
}

/// Ask the frontend whether any core option changed since the last query.
pub fn core_options_changed() -> bool {
    let mut updated = false;
    let ok = callbacks::environment(
        RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,
        &mut updated as *mut _ as *mut c_void,
    );
    ok && updated
}

/// Parse the current frontend selections, falling back to defaults for any
/// option the frontend leaves unset or reports with an invalid value.
pub fn read_core_options(mut get: impl FnMut(&CStr) -> Option<String>) -> CoreOptions {
    let mut options = CoreOptions::default();
    if let Some(volume) = get(OPTION_VOLUME).and_then(|value| value.parse::<u32>().ok()) {
        options.volume = volume.min(100);
    }
    if let Some(delay) = get(OPTION_REPEAT_DELAY).and_then(|value| value.parse::<u32>().ok()) {
        options.repeat_delay = delay;
    }
    if let Some(period) = get(OPTION_REPEAT_PERIOD).and_then(|value| value.parse::<u32>().ok()) {
        options.repeat_period = period.max(1);
    }
    if let Some(touch) = get(OPTION_TOUCH_INPUT) {
        if touch == "enabled" {
            options.touch_input = true;
        } else if touch == "disabled" {
            options.touch_input = false;
        }
    }
    if let Some(debug) = get(OPTION_DEBUG_LOGGING) {
        if debug == "enabled" {
            options.debug_logging = true;
        } else if debug == "disabled" {
            options.debug_logging = false;
        }
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a fake option getter that answers from a key-value map.
    fn fake_get<'a>(values: &'a [(&'a CStr, &'a str)]) -> impl FnMut(&CStr) -> Option<String> + 'a {
        move |key| {
            values
                .iter()
                .find(|(candidate, _)| *candidate == key)
                .map(|(_, value)| value.to_string())
        }
    }

    #[test]
    fn variable_list_is_terminated_and_has_unique_keys() {
        let variables = core_option_variables();

        let mut keys = Vec::new();
        for variable in variables.iter().take(variables.len() - 1) {
            let key = unsafe { CStr::from_ptr(variable.key) }.to_str().unwrap();
            assert!(!key.is_empty(), "option key must not be empty");
            assert!(!keys.contains(&key), "duplicate option key: {key}");
            keys.push(key);
        }
        assert!(variables.last().unwrap().key.is_null());
        assert!(variables.last().unwrap().value.is_null());
    }

    #[test]
    fn unset_options_fall_back_to_defaults() {
        let options = read_core_options(fake_get(&[]));
        assert_eq!(options, CoreOptions::default());
    }

    #[test]
    fn frontend_values_override_defaults() {
        let options = read_core_options(fake_get(&[
            (OPTION_VOLUME, "70"),
            (OPTION_REPEAT_DELAY, "20"),
            (OPTION_REPEAT_PERIOD, "6"),
            (OPTION_TOUCH_INPUT, "disabled"),
            (OPTION_DEBUG_LOGGING, "enabled"),
        ]));
        assert_eq!(
            options,
            CoreOptions {
                volume: 70,
                repeat_delay: 20,
                repeat_period: 6,
                touch_input: false,
                debug_logging: true,
            }
        );
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let options = read_core_options(fake_get(&[
            (OPTION_VOLUME, "150"),
            (OPTION_REPEAT_PERIOD, "0"),
        ]));
        assert_eq!(options.volume, 100);
        assert_eq!(options.repeat_period, 1);
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let options = read_core_options(fake_get(&[
            (OPTION_VOLUME, "loud"),
            (OPTION_REPEAT_DELAY, "-3"),
            (OPTION_REPEAT_PERIOD, "soon"),
            (OPTION_TOUCH_INPUT, "sometimes"),
            (OPTION_DEBUG_LOGGING, "verbose"),
        ]));
        assert_eq!(options, CoreOptions::default());
    }

    #[test]
    fn value_strings_start_with_the_default_choice() {
        let cases = [
            (OPTION_VOLUME, "100"),
            (OPTION_REPEAT_DELAY, "10"),
            (OPTION_REPEAT_PERIOD, "15"),
            (OPTION_TOUCH_INPUT, "enabled"),
            (OPTION_DEBUG_LOGGING, "disabled"),
        ];
        for (key, expected_default) in cases {
            let variables = core_option_variables();
            let variable = variables
                .iter()
                .find(|variable| unsafe { CStr::from_ptr(variable.key) } == key)
                .expect("option key must be advertised");
            let value = unsafe { CStr::from_ptr(variable.value) }.to_str().unwrap();
            let choices = value.split("; ").nth(1).unwrap();
            assert_eq!(
                choices.split('|').next().unwrap(),
                expected_default,
                "first choice of {key:?} must be its default"
            );
        }
    }
}
