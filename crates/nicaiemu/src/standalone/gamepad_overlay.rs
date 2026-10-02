// Virtual gamepad overlay for the standalone frontend.
//
// Layout: a translucent control dock along the bottom edge. The top row is
// the numeric keypad spread across the full width; the bottom row follows a
// standard gamepad silhouette: large D-pad on the left, OK in the middle like
// a Start button, and the Q/E/*/# face keys as a diamond on the right. The
// rest of the frame stays clear so game content remains visible while the
// overlay shows which keys are held.

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

// Style palette. Idle buttons are dark chips with a lighter edge; pressed
// buttons fill with a per-group accent and switch the label to a dark ink
// color so it stays readable on the bright fill.
const COLOR_IDLE: u32 = 0x002A3140;
const COLOR_IDLE_EDGE: u32 = 0x004C5A70;
const COLOR_DPAD_PRESSED: u32 = 0x0000DDFF;
const COLOR_OK_PRESSED: u32 = 0x0000EE44;
const COLOR_SOFT_PRESSED: u32 = 0x00FF8800;
const COLOR_NUM_PRESSED: u32 = 0x00FFE040;
const COLOR_LABEL: u32 = 0x00E8ECF2;
const COLOR_LABEL_INK: u32 = 0x00101820;
const COLOR_PANEL: u32 = 0x5A12161C;

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

        let unit = (width.min(height) / 44).clamp(3, 8) as i32;
        let width = width as i32;
        let height = height as i32;
        let margin = 2 * unit;
        let gap = (unit / 2).max(1);

        // Numeric row across the top of the dock, control row below it.
        let digit_h = 3 * unit + 2;
        let control_h = 8 * unit;
        let dock_h = margin + digit_h + gap + control_h + margin;
        let dock_y = height - dock_h;
        fill_chamfered_rect(
            buffer,
            width,
            height,
            margin / 2,
            dock_y,
            width - margin,
            dock_h,
            unit,
            COLOR_PANEL,
        );

        Self::draw_digit_row(
            buffer,
            width,
            height,
            dock_y + margin,
            digit_h,
            margin,
            gap,
            held,
        );
        Self::draw_control_row(
            buffer,
            width,
            height,
            dock_y + margin + digit_h + gap,
            control_h,
            margin,
            held,
        );
    }

    /// Numeric keypad 0-9 spread across the full dock width.
    #[allow(clippy::too_many_arguments)]
    fn draw_digit_row(
        buffer: &mut [u32],
        width: i32,
        height: i32,
        y: i32,
        key_h: i32,
        margin: i32,
        gap: i32,
        held: u32,
    ) {
        const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        let usable = width - 2 * margin - 9 * gap;
        let key_w = (usable / 10).clamp(1, key_h);
        let row_w = 10 * key_w + 9 * gap;
        let start_x = (width - row_w) / 2;
        for (index, digit) in DIGITS.iter().enumerate() {
            let x = start_x + index as i32 * (key_w + gap);
            draw_chip(
                buffer,
                width,
                height,
                x,
                y,
                key_w,
                key_h,
                digit,
                held & (1 << index) != 0,
                COLOR_NUM_PRESSED,
            );
        }
    }

    /// Standard gamepad layout: large D-pad on the left, OK in the middle
    /// (Start position), and Q/E/*/# as a face-button diamond on the right.
    #[allow(clippy::too_many_arguments)]
    fn draw_control_row(
        buffer: &mut [u32],
        width: i32,
        height: i32,
        y: i32,
        row_h: i32,
        margin: i32,
        held: u32,
    ) {
        let center_y = y + row_h / 2;

        // D-pad occupies the full row height on the left.
        let dpad_size = row_h;
        let dpad_cx = margin + dpad_size / 2;
        Self::draw_dpad(buffer, width, height, dpad_cx, center_y, dpad_size, held);

        // Face-key diamond on the right: Q top, E left, * right, # bottom,
        // matching the RetroPad positions (X=Q north, Y=E west).
        let button = (row_h * 2 / 5).max(3);
        let radius = (row_h * 3 / 10).max(1);
        let diamond_cx = width - margin - radius - button / 2;
        let faces = [
            ("Q", KEY_Q, 0, -radius),
            ("E", KEY_E, -radius, 0),
            ("*", KEY_N, radius, 0),
            ("#", KEY_M, 0, radius),
        ];
        for (label, key, dx, dy) in faces {
            draw_chip(
                buffer,
                width,
                height,
                diamond_cx + dx - button / 2,
                center_y + dy - button / 2,
                button,
                button,
                label,
                held & key != 0,
                COLOR_SOFT_PRESSED,
            );
        }

        // OK sits centered between the two clusters like a Start button.
        let ok_w = (row_h * 3 / 5).max(1);
        let ok_h = (row_h * 2 / 5).max(1);
        let diamond_left = diamond_cx - radius - button / 2;
        let ok_x = (dpad_cx + dpad_size / 2 + diamond_left) / 2 - ok_w / 2;
        draw_chip(
            buffer,
            width,
            height,
            ok_x,
            center_y - ok_h / 2,
            ok_w,
            ok_h,
            "OK",
            held & KEY_OK != 0,
            COLOR_OK_PRESSED,
        );
    }

    /// Cross-shaped D-pad with arrow triangles and a small center hub.
    #[allow(clippy::too_many_arguments)]
    fn draw_dpad(
        buffer: &mut [u32],
        width: i32,
        height: i32,
        cx: i32,
        cy: i32,
        size: i32,
        held: u32,
    ) {
        let arm = (size * 2 / 5).max(3);
        let half = arm / 2;
        // The four arms of the cross; the middle cell is only a hub.
        let arms = [
            (cx - half, cy - arm - half, KEY_UP, Direction::Up),
            (cx - half, cy + half, KEY_DOWN, Direction::Down),
            (cx - arm - half, cy - half, KEY_LEFT, Direction::Left),
            (cx + half, cy - half, KEY_RIGHT, Direction::Right),
        ];
        for (x, y, key, direction) in arms {
            let pressed = held & key != 0;
            let fill = if pressed {
                COLOR_DPAD_PRESSED
            } else {
                COLOR_IDLE
            };
            fill_chamfered_rect(buffer, width, height, x, y, arm, arm, 1, fill);
            draw_edge(
                buffer,
                width,
                height,
                x,
                y,
                arm,
                arm,
                1,
                1,
                if pressed {
                    COLOR_LABEL_INK
                } else {
                    COLOR_IDLE_EDGE
                },
            );
            let (arrow_x, arrow_y) = (x + arm / 2, y + arm / 2);
            let arrow_color = if pressed {
                COLOR_LABEL_INK
            } else {
                COLOR_LABEL
            };
            fill_triangle(
                buffer,
                width,
                height,
                arrow_x,
                arrow_y,
                (arm / 3).max(1),
                direction,
                arrow_color,
            );
        }
        // Center hub ties the cross together visually.
        fill_chamfered_rect(
            buffer,
            width,
            height,
            cx - half,
            cy - half,
            arm,
            arm,
            1,
            COLOR_IDLE,
        );
        draw_edge(
            buffer,
            width,
            height,
            cx - half,
            cy - half,
            arm,
            arm,
            1,
            1,
            COLOR_IDLE_EDGE,
        );
        let hub = (arm / 3).max(1);
        fill_rect(
            buffer,
            width,
            height,
            cx - hub / 2,
            cy - hub / 2,
            hub,
            hub,
            COLOR_IDLE_EDGE,
        );
    }
}

enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Rounded-rectangle-style chip: chamfered fill plus a one-pixel edge and a
/// centered label that darkens while the key is held.
#[allow(clippy::too_many_arguments)]
fn draw_chip(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    chip_width: i32,
    chip_height: i32,
    label: &str,
    pressed: bool,
    pressed_color: u32,
) {
    if chip_width <= 0 || chip_height <= 0 {
        return;
    }
    let chamfer = (chip_height / 6).clamp(1, 3);
    fill_chamfered_rect(
        buffer,
        width,
        height,
        x,
        y,
        chip_width,
        chip_height,
        chamfer,
        if pressed { pressed_color } else { COLOR_IDLE },
    );
    draw_edge(
        buffer,
        width,
        height,
        x,
        y,
        chip_width,
        chip_height,
        chamfer,
        1,
        if pressed {
            COLOR_LABEL_INK
        } else {
            COLOR_IDLE_EDGE
        },
    );
    draw_text_centered(
        buffer,
        width,
        height,
        x,
        y,
        chip_width,
        chip_height,
        label,
        if pressed {
            COLOR_LABEL_INK
        } else {
            COLOR_LABEL
        },
    );
}

/// Fill a rectangle with its corners chamfered for a soft pixel-UI look.
#[allow(clippy::too_many_arguments)]
fn fill_chamfered_rect(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_width: i32,
    rect_height: i32,
    chamfer: i32,
    color: u32,
) {
    if rect_width <= 0 || rect_height <= 0 {
        return;
    }
    let chamfer = chamfer.min(rect_width / 2).min(rect_height / 2).max(0);
    for row in 0..rect_height {
        let inset = row_inset(row, rect_height, chamfer);
        fill_rect(
            buffer,
            width,
            height,
            x + inset,
            y + row,
            rect_width - 2 * inset,
            1,
            color,
        );
    }
}

/// One-pixel highlight along the chamfered outline of a shape.
#[allow(clippy::too_many_arguments)]
fn draw_edge(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_width: i32,
    rect_height: i32,
    chamfer: i32,
    edge: i32,
    edge_color: u32,
) {
    if rect_width <= 0 || rect_height <= 0 {
        return;
    }
    let chamfer = chamfer.min(rect_width / 2).min(rect_height / 2).max(0);
    for row in 0..rect_height {
        let inset = row_inset(row, rect_height, chamfer);
        let inner = (edge - 1).max(0);
        let border_row = row < edge || row >= rect_height - edge;
        for column in inset..rect_width - inset {
            let border_column = column < inset + edge || column >= rect_width - inset - edge;
            if border_row || border_column {
                // Skip pixels that sit deeper than the chamfer cut.
                if row < chamfer && column < chamfer - row + inner {
                    continue;
                }
                if row < chamfer && column >= rect_width - (chamfer - row) - inner {
                    continue;
                }
                if row >= rect_height - chamfer
                    && column < chamfer - (rect_height - 1 - row) + inner
                {
                    continue;
                }
                if row >= rect_height - chamfer
                    && column >= rect_width - (chamfer - (rect_height - 1 - row)) - inner
                {
                    continue;
                }
                fill_rect(buffer, width, height, x + column, y + row, 1, 1, edge_color);
            }
        }
    }
}

/// Horizontal inset applied to scanline `row` of a chamfered rectangle.
fn row_inset(row: i32, rect_height: i32, chamfer: i32) -> i32 {
    if row < chamfer {
        chamfer - row
    } else if row >= rect_height - chamfer {
        row - (rect_height - chamfer - 1)
    } else {
        0
    }
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

/// Solid isoceles triangle used for the D-pad arrows.
#[allow(clippy::too_many_arguments)]
fn fill_triangle(
    buffer: &mut [u32],
    width: i32,
    height: i32,
    center_x: i32,
    center_y: i32,
    radius: i32,
    direction: Direction,
    color: u32,
) {
    if radius <= 0 {
        return;
    }
    // Scan from tip (-radius) to base (+radius); each line is twice as wide
    // as the previous one, giving a solid isoceles triangle.
    for offset in -radius..=radius {
        let half_span = ((offset + radius) / 2).max(0);
        match direction {
            Direction::Up => fill_rect(
                buffer,
                width,
                height,
                center_x - half_span,
                center_y + offset,
                half_span * 2 + 1,
                1,
                color,
            ),
            Direction::Down => fill_rect(
                buffer,
                width,
                height,
                center_x - half_span,
                center_y - offset,
                half_span * 2 + 1,
                1,
                color,
            ),
            Direction::Left => fill_rect(
                buffer,
                width,
                height,
                center_x + offset,
                center_y - half_span,
                1,
                half_span * 2 + 1,
                color,
            ),
            Direction::Right => fill_rect(
                buffer,
                width,
                height,
                center_x - offset,
                center_y - half_span,
                1,
                half_span * 2 + 1,
                color,
            ),
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
    let scale = (cell_height / 8).max(1);
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
        '#' => Some([0b101, 0b111, 0b101, 0b111, 0b101, 0b000]),
        '*' => Some([0b000, 0b101, 0b010, 0b101, 0b000, 0b000]),
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

    /// The dock must not eat the game view: it only covers the bottom band
    /// of the frame and never touches the top half.
    #[test]
    fn overlay_is_confined_to_the_bottom_dock() {
        let mut frame = vec![0x00101010; 240 * 400];
        GamepadOverlay::draw(&mut frame, 240, 400, 0);
        let top_half = &frame[..200 * 240];
        assert!(top_half.iter().all(|&pixel| pixel == 0x00101010));
    }
}
