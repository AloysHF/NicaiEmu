// Virtual gamepad overlay for the standalone frontend.

// Guest key codes: 0-9 digits, 12 Q, 13 E, 14 OK, 15-18 dpad, 19 N, 20 M.
const KEY_Q: u32 = 1 << 12;
const KEY_E: u32 = 1 << 13;
const KEY_OK: u32 = 1 << 14;
const KEY_UP: u32 = 1 << 17;
const KEY_DOWN: u32 = 1 << 18;
const KEY_LEFT: u32 = 1 << 15;
const KEY_RIGHT: u32 = 1 << 16;
const KEY_N: u32 = 1 << 19;
const KEY_M: u32 = 1 << 20;

const COLOR_IDLE: u32 = 0x00505050;
const COLOR_DPAD_PRESSED: u32 = 0x0000DDFF;
const COLOR_OK_PRESSED: u32 = 0x0000EE44;
const COLOR_SOFT_PRESSED: u32 = 0x00FF8800;
const COLOR_NUM_PRESSED: u32 = 0x00FFE040;
const COLOR_LABEL: u32 = 0x00E8E8E8;
const COLOR_BACKGROUND: u32 = 0xA01A1A1A;

pub struct GamepadOverlay;

impl GamepadOverlay {
    /// Draw the effective physical key state into a native-resolution frame.
    pub fn draw(buffer: &mut [u32], width: u32, height: u32, held: u32) {
        let Some(expected_len) = (width as usize).checked_mul(height as usize) else {
            return;
        };
        if width == 0 || height == 0 || buffer.len() < expected_len {
            return;
        }

        let unit = (width.min(height) / 48).clamp(1, 6) as i32;
        let margin = 2 * unit;
        let width = width as i32;
        let height = height as i32;

        Self::draw_keypad(buffer, width, height, unit, held);

        let dpad_x = margin + 4 * unit;
        let dpad_y = height - margin - 4 * unit;
        fill_rect_alpha(
            buffer,
            width,
            height,
            dpad_x - 4 * unit,
            dpad_y - 4 * unit,
            9 * unit,
            9 * unit,
            COLOR_BACKGROUND,
        );
        draw_dpad_button(
            buffer,
            width,
            height,
            dpad_x - unit,
            dpad_y - 4 * unit,
            unit,
            "U",
            held & KEY_UP != 0,
        );
        draw_dpad_button(
            buffer,
            width,
            height,
            dpad_x - unit,
            dpad_y + 2 * unit,
            unit,
            "D",
            held & KEY_DOWN != 0,
        );
        draw_dpad_button(
            buffer,
            width,
            height,
            dpad_x - 4 * unit,
            dpad_y - unit,
            unit,
            "L",
            held & KEY_LEFT != 0,
        );
        draw_dpad_button(
            buffer,
            width,
            height,
            dpad_x + 2 * unit,
            dpad_y - unit,
            unit,
            "R",
            held & KEY_RIGHT != 0,
        );
        fill_rect(
            buffer,
            width,
            height,
            dpad_x - unit,
            dpad_y - unit,
            3 * unit,
            3 * unit,
            COLOR_IDLE,
        );

        // Q/E soft keys sit above the dpad.
        draw_soft_key(
            buffer,
            width,
            height,
            dpad_x - 2 * unit,
            dpad_y - 8 * unit,
            4 * unit,
            2 * unit,
            "Q",
            held & KEY_Q != 0,
            COLOR_SOFT_PRESSED,
        );
        draw_soft_key(
            buffer,
            width,
            height,
            dpad_x - 2 * unit,
            dpad_y - 5 * unit,
            4 * unit,
            2 * unit,
            "E",
            held & KEY_E != 0,
            COLOR_SOFT_PRESSED,
        );

        // N/M sit to the right of the dpad.
        draw_soft_key(
            buffer,
            width,
            height,
            dpad_x + 5 * unit,
            dpad_y - 4 * unit,
            4 * unit,
            2 * unit,
            "N",
            held & KEY_N != 0,
            COLOR_SOFT_PRESSED,
        );
        draw_soft_key(
            buffer,
            width,
            height,
            dpad_x + 5 * unit,
            dpad_y - unit,
            4 * unit,
            2 * unit,
            "M",
            held & KEY_M != 0,
            COLOR_SOFT_PRESSED,
        );

        // OK confirm button in the bottom-right corner.
        let ok_x = width - margin - 3 * unit;
        let ok_y = height - margin - 5 * unit;
        draw_action_button(
            buffer,
            width,
            height,
            ok_x,
            ok_y,
            2 * unit,
            "OK",
            if held & KEY_OK != 0 {
                COLOR_OK_PRESSED
            } else {
                COLOR_IDLE
            },
        );
    }

    /// Draw a two-row, five-column numeric keypad along the top edge.
    fn draw_keypad(buffer: &mut [u32], width: i32, height: i32, unit: i32, held: u32) {
        const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        let cell_width = 3 * unit;
        let cell_height = 2 * unit;
        let pitch = 4 * unit;
        let keypad_width = 5 * cell_width + 4 * unit;
        let start_x = (width - keypad_width) / 2;
        let start_y = 2 * unit;
        fill_rect_alpha(
            buffer,
            width,
            height,
            start_x - unit,
            start_y - unit,
            keypad_width + 2 * unit,
            2 * cell_height + 3 * unit,
            COLOR_BACKGROUND,
        );
        for (index, digit) in DIGITS.iter().enumerate() {
            let column = (index % 5) as i32;
            let row = (index / 5) as i32;
            let x = start_x + column * pitch;
            let y = start_y + row * (cell_height + unit);
            let mask = 1u32 << index;
            draw_soft_key(
                buffer,
                width,
                height,
                x,
                y,
                cell_width,
                cell_height,
                digit,
                held & mask != 0,
                COLOR_NUM_PRESSED,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_dpad_button(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    unit: i32,
    label: &str,
    pressed: bool,
) {
    fill_rect(
        buffer,
        width,
        height,
        x,
        y,
        3 * unit,
        3 * unit,
        if pressed {
            COLOR_DPAD_PRESSED
        } else {
            COLOR_IDLE
        },
    );
    draw_text_centered(
        buffer,
        width,
        height,
        x,
        y,
        3 * unit,
        3 * unit,
        label,
        COLOR_LABEL,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_action_button(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    center_x: i32,
    center_y: i32,
    radius: i32,
    label: &str,
    color: u32,
) {
    fill_rect_alpha(
        buffer,
        width,
        height,
        center_x - radius - 3,
        center_y - radius - 3,
        radius * 2 + 7,
        radius * 2 + 7,
        COLOR_BACKGROUND,
    );
    fill_circle(buffer, width, height, center_x, center_y, radius, color);
    draw_text_centered(
        buffer,
        width,
        height,
        center_x - radius,
        center_y - radius,
        radius * 2 + 1,
        radius * 2 + 1,
        label,
        COLOR_LABEL,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_soft_key(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    button_width: i32,
    button_height: i32,
    label: &str,
    pressed: bool,
    pressed_color: u32,
) {
    fill_rect_alpha(
        buffer,
        width,
        height,
        x - 2,
        y - 2,
        button_width + 4,
        button_height + 4,
        COLOR_BACKGROUND,
    );
    fill_rect(
        buffer,
        width,
        height,
        x,
        y,
        button_width,
        button_height,
        if pressed { pressed_color } else { COLOR_IDLE },
    );
    draw_text_centered(
        buffer,
        width,
        height,
        x,
        y,
        button_width,
        button_height,
        label,
        COLOR_LABEL,
    );
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_width: i32,
    rect_height: i32,
    color: u32,
) {
    for pixel_y in y.max(0)..(y + rect_height).min(height) {
        for pixel_x in x.max(0)..(x + rect_width).min(width) {
            buffer[pixel_y as usize * width as usize + pixel_x as usize] = color;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect_alpha(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_width: i32,
    rect_height: i32,
    color: u32,
) {
    let alpha = (color >> 24) & 0xFF;
    let inverse_alpha = 255 - alpha;
    let source_r = (color >> 16) & 0xFF;
    let source_g = (color >> 8) & 0xFF;
    let source_b = color & 0xFF;
    for pixel_y in y.max(0)..(y + rect_height).min(height) {
        for pixel_x in x.max(0)..(x + rect_width).min(width) {
            let index = pixel_y as usize * width as usize + pixel_x as usize;
            let destination = buffer[index];
            let destination_r = (destination >> 16) & 0xFF;
            let destination_g = (destination >> 8) & 0xFF;
            let destination_b = destination & 0xFF;
            let r = (source_r * alpha + destination_r * inverse_alpha) / 255;
            let g = (source_g * alpha + destination_g * inverse_alpha) / 255;
            let b = (source_b * alpha + destination_b * inverse_alpha) / 255;
            buffer[index] = (r << 16) | (g << 8) | b;
        }
    }
}

fn fill_circle(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    center_x: i32,
    center_y: i32,
    radius: i32,
    color: u32,
) {
    let radius_squared = radius * radius;
    for offset_y in -radius..=radius {
        let pixel_y = center_y + offset_y;
        if pixel_y < 0 || pixel_y >= height {
            continue;
        }
        for offset_x in -radius..=radius {
            let pixel_x = center_x + offset_x;
            if pixel_x >= 0
                && pixel_x < width
                && offset_x * offset_x + offset_y * offset_y <= radius_squared
            {
                buffer[pixel_y as usize * width as usize + pixel_x as usize] = color;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text_centered(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    cell_width: i32,
    cell_height: i32,
    text: &str,
    color: u32,
) {
    let scale = (cell_height / 7).max(1);
    let text_width = text.chars().count() as i32 * 4 * scale - scale;
    let origin_x = x + (cell_width - text_width) / 2;
    let origin_y = y + (cell_height - 6 * scale) / 2;
    for (index, character) in text.chars().enumerate() {
        let Some(glyph) = glyph(character) else {
            continue;
        };
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..3 {
                if bits & (1 << (2 - column)) != 0 {
                    fill_rect(
                        buffer,
                        width,
                        height,
                        origin_x + (index as i32 * 4 + column) * scale,
                        origin_y + row as i32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }
}

fn glyph(character: char) -> Option<[u8; 6]> {
    match character {
        '0' => Some([0b011, 0b101, 0b101, 0b101, 0b101, 0b011]),
        '1' => Some([0b010, 0b110, 0b010, 0b010, 0b010, 0b111]),
        '2' => Some([0b111, 0b001, 0b011, 0b100, 0b100, 0b111]),
        '3' => Some([0b111, 0b001, 0b011, 0b001, 0b001, 0b111]),
        '4' => Some([0b101, 0b101, 0b111, 0b001, 0b001, 0b001]),
        '5' => Some([0b111, 0b100, 0b111, 0b001, 0b001, 0b111]),
        '6' => Some([0b011, 0b100, 0b111, 0b101, 0b101, 0b011]),
        '7' => Some([0b111, 0b001, 0b010, 0b010, 0b010, 0b010]),
        '8' => Some([0b011, 0b101, 0b011, 0b101, 0b101, 0b011]),
        '9' => Some([0b011, 0b101, 0b111, 0b001, 0b001, 0b011]),
        'D' => Some([0b110, 0b101, 0b101, 0b101, 0b101, 0b110]),
        'E' => Some([0b111, 0b100, 0b110, 0b100, 0b100, 0b111]),
        'K' => Some([0b101, 0b101, 0b110, 0b101, 0b101, 0b101]),
        'L' => Some([0b100, 0b100, 0b100, 0b100, 0b100, 0b111]),
        'M' => Some([0b101, 0b111, 0b111, 0b101, 0b101, 0b101]),
        'N' => Some([0b101, 0b111, 0b111, 0b101, 0b101, 0b101]),
        'O' => Some([0b011, 0b101, 0b101, 0b101, 0b101, 0b011]),
        'Q' => Some([0b011, 0b101, 0b101, 0b101, 0b011, 0b001]),
        'R' => Some([0b110, 0b101, 0b110, 0b101, 0b101, 0b101]),
        'U' => Some([0b101, 0b101, 0b101, 0b101, 0b101, 0b111]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_KEYS: u32 = (1 << 21) - 1;

    #[test]
    fn every_held_key_group_has_a_distinct_highlight() {
        let mut frame = vec![0x00101010; 240 * 400];
        GamepadOverlay::draw(&mut frame, 240, 400, ALL_KEYS);

        for color in [
            COLOR_DPAD_PRESSED,
            COLOR_OK_PRESSED,
            COLOR_SOFT_PRESSED,
            COLOR_NUM_PRESSED,
        ] {
            assert!(frame.contains(&color), "missing highlight {color:08X}");
        }
    }

    #[test]
    fn idle_overlay_does_not_use_pressed_colors() {
        let mut frame = vec![0x00101010; 240 * 400];
        GamepadOverlay::draw(&mut frame, 240, 400, 0);
        assert!(frame.contains(&COLOR_IDLE));
        assert!(!frame.contains(&COLOR_DPAD_PRESSED));
        assert!(!frame.contains(&COLOR_OK_PRESSED));
        assert!(!frame.contains(&COLOR_SOFT_PRESSED));
        assert!(!frame.contains(&COLOR_NUM_PRESSED));
    }

    #[test]
    fn varied_and_tiny_frame_sizes_are_clipped_safely() {
        for (width, height) in [(1, 1), (13, 9), (160, 120), (240, 400), (480, 800)] {
            let mut frame = vec![0; width * height];
            GamepadOverlay::draw(&mut frame, width as u32, height as u32, ALL_KEYS);
            assert_eq!(frame.len(), width * height);
        }
    }

    #[test]
    fn short_or_zero_sized_buffers_are_ignored() {
        let mut short = vec![0x00123456; 3];
        GamepadOverlay::draw(&mut short, 2, 2, ALL_KEYS);
        assert_eq!(short, [0x00123456; 3]);

        let mut empty = Vec::new();
        GamepadOverlay::draw(&mut empty, 0, 0, ALL_KEYS);
        assert!(empty.is_empty());
    }
}
