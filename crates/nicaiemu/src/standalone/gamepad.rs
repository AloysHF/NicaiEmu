// Standalone physical gamepad-to-guest-key mapping via gilrs.

use gilrs::{Axis, Button, EventType, Gilrs};
use nicaiemu_core::NicaiMachine;

const STICK_DEADZONE: f32 = 0.5;

// Phone keypad ABI guest key codes.
const GUEST_Q: u8 = 12;
const GUEST_E: u8 = 13;
const GUEST_OK: u8 = 14;
const GUEST_LEFT: u8 = 15;
const GUEST_RIGHT: u8 = 16;
const GUEST_UP: u8 = 17;
const GUEST_DOWN: u8 = 18;
const GUEST_N: u8 = 19;
const GUEST_M: u8 = 20;

/// Polls the first connected physical gamepad and maps it onto phone keys.
///
/// The layout mirrors the libretro core's RetroPad mapping one-to-one:
/// South/East/Start confirm, North/X plus the left shoulder are the left soft
/// key, West/Y plus the right shoulder are the right soft key, Select plus the
/// left trigger fire the star key (guest 19), and the right trigger fires the
/// hash key (guest 20). Left/right sticks also act as a digital D-pad once
/// past [`STICK_DEADZONE`].
pub struct GamepadMapper {
    gilrs: Option<Gilrs>,
}

impl GamepadMapper {
    pub fn new(enabled: bool) -> Self {
        let gilrs = if enabled {
            match Gilrs::new() {
                Ok(gilrs) => {
                    log::info!("Gamepad support enabled");
                    Some(gilrs)
                }
                Err(error) => {
                    log::warn!("Gamepad support unavailable: {error}");
                    None
                }
            }
        } else {
            None
        };
        Self { gilrs }
    }

    /// Force-press any guest key held on a connected pad.
    ///
    /// Only sets keys to pressed; releasing is left to the keyboard mapper so
    /// keyboard and gamepad combine as a logical OR.
    pub fn hold_pressed(&mut self, machine: &mut NicaiMachine) {
        for key in self.pressed_keys() {
            machine.set_key(key, true);
        }
    }

    /// Guest keys currently held on the first connected pad.
    pub fn pressed_keys(&mut self) -> Vec<u8> {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return Vec::new();
        };

        while let Some(event) = gilrs.next_event() {
            match event.event {
                EventType::Connected => {
                    log::info!("Gamepad connected: {}", gilrs.gamepad(event.id).name());
                }
                EventType::Disconnected => {
                    log::info!("Gamepad disconnected: {}", event.id);
                }
                _ => {}
            }
        }

        for (_id, gamepad) in gilrs.gamepads() {
            if !gamepad.is_connected() {
                continue;
            }
            return map_guest_keys(
                |button| gamepad.is_pressed(button),
                |axis| gamepad.value(axis),
            );
        }
        Vec::new()
    }
}

/// Map pad buttons/axes onto guest keys.
///
/// Accepts closures so the mapping can be unit-tested without a physical pad.
fn map_guest_keys(is_pressed: impl Fn(Button) -> bool, axis: impl Fn(Axis) -> f32) -> Vec<u8> {
    let mut keys = Vec::new();

    if is_pressed(Button::DPadUp)
        || axis(Axis::LeftStickY) > STICK_DEADZONE
        || axis(Axis::RightStickY) > STICK_DEADZONE
    {
        keys.push(GUEST_UP);
    }
    if is_pressed(Button::DPadDown)
        || axis(Axis::LeftStickY) < -STICK_DEADZONE
        || axis(Axis::RightStickY) < -STICK_DEADZONE
    {
        keys.push(GUEST_DOWN);
    }
    if is_pressed(Button::DPadLeft)
        || axis(Axis::LeftStickX) < -STICK_DEADZONE
        || axis(Axis::RightStickX) < -STICK_DEADZONE
    {
        keys.push(GUEST_LEFT);
    }
    if is_pressed(Button::DPadRight)
        || axis(Axis::LeftStickX) > STICK_DEADZONE
        || axis(Axis::RightStickX) > STICK_DEADZONE
    {
        keys.push(GUEST_RIGHT);
    }

    // RetroPad B/A/Start all confirm, matching the libretro core.
    if is_pressed(Button::South) || is_pressed(Button::East) || is_pressed(Button::Start) {
        keys.push(GUEST_OK);
    }
    // RetroPad X plus the left shoulder (L1) duplicate the left soft key.
    if is_pressed(Button::North) || is_pressed(Button::LeftTrigger) {
        keys.push(GUEST_Q);
    }
    // RetroPad Y plus the right shoulder (R1) duplicate the right soft key.
    if is_pressed(Button::West) || is_pressed(Button::RightTrigger) {
        keys.push(GUEST_E);
    }
    // Select plus the left trigger (L2) fire the star key (guest `*`).
    if is_pressed(Button::Select) || is_pressed(Button::LeftTrigger2) {
        keys.push(GUEST_N);
    }
    // The right trigger (R2) fires the hash key (guest `#`).
    if is_pressed(Button::RightTrigger2) {
        keys.push(GUEST_M);
    }

    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none_pressed(_button: Button) -> bool {
        false
    }

    fn zero_axis(_axis: Axis) -> f32 {
        0.0
    }

    #[test]
    fn disabled_mapper_reports_no_keys() {
        let mut mapper = GamepadMapper::new(false);
        assert!(mapper.pressed_keys().is_empty());
    }

    #[test]
    fn idle_pad_reports_no_keys() {
        assert!(map_guest_keys(none_pressed, zero_axis).is_empty());
    }

    #[test]
    fn stick_deadzone_threshold_rejects_small_values() {
        assert!(0.4_f32.abs() < STICK_DEADZONE);
        assert!(0.5_f32.abs() >= STICK_DEADZONE);
    }

    #[test]
    fn sticks_past_deadzone_map_to_directions() {
        let keys = map_guest_keys(none_pressed, |axis| match axis {
            Axis::LeftStickY => 0.8,
            Axis::LeftStickX => -0.9,
            _ => 0.0,
        });
        assert_eq!(keys, [GUEST_UP, GUEST_LEFT]);
    }

    #[test]
    fn face_buttons_follow_retropad_layout() {
        let south = map_guest_keys(|b| b == Button::South, zero_axis);
        assert_eq!(south, [GUEST_OK]);

        let east = map_guest_keys(|b| b == Button::East, zero_axis);
        assert_eq!(east, [GUEST_OK]);

        let north = map_guest_keys(|b| b == Button::North, zero_axis);
        assert_eq!(north, [GUEST_Q]);

        let west = map_guest_keys(|b| b == Button::West, zero_axis);
        assert_eq!(west, [GUEST_E]);

        let start = map_guest_keys(|b| b == Button::Start, zero_axis);
        assert_eq!(start, [GUEST_OK]);
    }

    #[test]
    fn shoulders_duplicate_soft_keys() {
        let left = map_guest_keys(|b| b == Button::LeftTrigger, zero_axis);
        assert_eq!(left, [GUEST_Q]);

        let right = map_guest_keys(|b| b == Button::RightTrigger, zero_axis);
        assert_eq!(right, [GUEST_E]);
    }

    #[test]
    fn select_and_left_trigger_map_to_star_key() {
        let select = map_guest_keys(|b| b == Button::Select, zero_axis);
        assert_eq!(select, [GUEST_N]);

        let left_trigger = map_guest_keys(|b| b == Button::LeftTrigger2, zero_axis);
        assert_eq!(left_trigger, [GUEST_N]);
    }

    #[test]
    fn right_trigger_maps_to_hash_key() {
        let right_trigger = map_guest_keys(|b| b == Button::RightTrigger2, zero_axis);
        assert_eq!(right_trigger, [GUEST_M]);
    }

    /// Every non-digit guest key must be reachable from the pad, keeping the
    /// standalone mapper in lockstep with the libretro RetroPad layout.
    #[test]
    fn every_non_digit_guest_key_is_reachable() {
        let buttons = [
            Button::DPadUp,
            Button::DPadDown,
            Button::DPadLeft,
            Button::DPadRight,
            Button::South,
            Button::East,
            Button::North,
            Button::West,
            Button::Start,
            Button::Select,
            Button::LeftTrigger,
            Button::RightTrigger,
            Button::LeftTrigger2,
            Button::RightTrigger2,
        ];
        let mut reached = std::collections::HashSet::new();
        for button in buttons {
            reached.extend(map_guest_keys(|b| b == button, zero_axis));
        }
        assert_eq!(
            reached,
            [
                GUEST_Q,
                GUEST_E,
                GUEST_OK,
                GUEST_LEFT,
                GUEST_RIGHT,
                GUEST_UP,
                GUEST_DOWN,
                GUEST_N,
                GUEST_M
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn dpad_maps_to_direction_keys() {
        let keys = map_guest_keys(
            |b| matches!(b, Button::DPadUp | Button::DPadRight),
            zero_axis,
        );
        assert_eq!(keys, [GUEST_UP, GUEST_RIGHT]);
    }
}
