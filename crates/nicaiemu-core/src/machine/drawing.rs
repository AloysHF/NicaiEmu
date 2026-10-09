//! Guest framebuffer drawing: LCD services, blits, rects, and text.

use armv4t_emu::{reg, Memory};
use encoding_rs::GBK;
use log::warn;

use super::{
    clip_axis, image_payload, service_trace_enabled, signed_coord, NicaiMachine,
    DREAM_FACTORY_PACKAGE_SLOT, HEAP_SIZE, METHOD_KIND_AUTO, METHOD_KIND_GAMEOLD, SCREEN_IMAGE,
    SCREEN_IMAGE_STRUCT,
};
use crate::image_decoder;

impl NicaiMachine {
    // ---- old-lib drawing API (F_0 0x00..0x3c) ----
    //
    // The native (big-endian, preferred-address) games ask native dispatch
    // sid 82 for a copy of the gameold table, keep it in their own buffer,
    // and call the drawing slots straight out of it.  These slots were
    // previously unimplemented: calls landed on generic stubs that allocated
    // memory instead of drawing, which is why those games stayed on a blank
    // framebuffer while the reference rendered their menus.

    /// Intersect the old-lib clip rectangle with the given rectangle; an
    /// empty intersection collapses the clip like the reference.
    fn oldlib_and_clip(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let [x0, y0, x1, y1] = self.oldlib_clip;
        if x + w >= x0 && y + h >= y0 && x1 >= x && y1 >= y {
            let (mut nx, mut ny, mut nw, mut nh) = (x, y, w, h);
            if x0 > nx {
                nw -= x0 - nx;
                nx = x0;
            }
            if y0 > ny {
                nh -= y0 - ny;
                ny = y0;
            }
            if nx + nw > x1 {
                nw = x1 - nx;
            }
            if ny + nh > y1 {
                nh = y1 - ny;
            }
            let (nw, nh) = (nw.max(0), nh.max(0));
            self.oldlib_clip = [nx, ny, nx + nw, ny + nh];
        } else {
            self.oldlib_clip = [0, 0, 0, 0];
        }
    }

    /// Clipped blit onto the framebuffer through the old-lib clip rectangle.
    #[allow(clippy::too_many_arguments)]
    fn oldlib_blit(
        &mut self,
        source: u32,
        mut source_x: i32,
        mut source_y: i32,
        mut width: i32,
        mut height: i32,
        mut destination_x: i32,
        mut destination_y: i32,
        transparent: bool,
    ) {
        if source == 0 {
            return;
        }
        let [x0, y0, x1, y1] = self.oldlib_clip;
        if !(destination_x + width > x0 && destination_y + height > y0) {
            return;
        }
        if x0 > destination_x {
            width -= x0 - destination_x;
            source_x -= x0 - destination_x;
            destination_x = x0;
        }
        if y0 > destination_y {
            height -= y0 - destination_y;
            source_y -= y0 - destination_y;
            destination_y = y0;
        }
        if destination_x + width > x1 {
            width = x1 - destination_x;
        }
        if width <= 0 {
            return;
        }
        if destination_y + height > y1 {
            height = y1 - destination_y;
        }
        if height <= 0 {
            return;
        }
        self.blit_image(
            SCREEN_IMAGE_STRUCT,
            source,
            source_x,
            source_y,
            width,
            height,
            destination_x,
            destination_y,
            transparent,
        );
    }

    /// DrawImageWithClip / DrawImageClipAndAlpha (F_0 0x04 / 0x08):
    /// (image, sx, sy, w, h, dx, dy) with the halves packed in the low 16
    /// bits of each register.
    pub(crate) fn draw_image_with_clip(&mut self, transparent: bool) {
        let source = self.register(0);
        let source_x = signed_coord(self.register(1));
        let source_y = signed_coord(self.register(2));
        let width = signed_coord(self.register(3));
        let height = signed_coord(self.register(4));
        let destination_x = signed_coord(self.register(5));
        let destination_y = signed_coord(self.register(6));
        self.oldlib_blit(
            source,
            source_x,
            source_y,
            width,
            height,
            destination_x,
            destination_y,
            transparent,
        );
        self.set_result(0);
    }

    /// DrawFullScreen (F_0 0x0c): copy the image straight into the
    /// framebuffer and adopt it as the new clip rectangle.
    pub(crate) fn draw_full_screen(&mut self) {
        let image = self.register(0);
        if image == 0 {
            self.set_result(0);
            return;
        }
        let (width, height) = self.image_dims(image);
        let width = width.clamp(0, 240);
        let height = height.clamp(0, 400);
        self.oldlib_clip = [0, 0, width, height];
        let pixels = self.memory.r32(image);
        if pixels != 0 {
            for y in 0..height {
                for x in 0..width {
                    let value = self.memory.r16(pixels + ((y * width + x) * 2) as u32);
                    self.memory
                        .w16(SCREEN_IMAGE + ((y * 240 + x) * 2) as u32, value);
                }
            }
        }
        self.set_result(0);
    }

    /// DrawNumber (F_0 0x10): draw a signed decimal number from a glyph
    /// strip; each digit is a cw x ch cell starting at the strip origin.
    pub(crate) fn draw_number_service(&mut self) {
        let image = self.register(0);
        let number = self.register(1) as i32;
        let cell_width = signed_coord(self.register(2));
        let cell_height = signed_coord(self.register(3));
        let gap = signed_coord(self.register(4));
        let x = signed_coord(self.register(5));
        let y = signed_coord(self.register(6));
        let align = self.register(7);
        if image == 0 || cell_width <= 0 || cell_height <= 0 {
            self.set_result(0);
            return;
        }
        let value = number.unsigned_abs();
        let mut digits = 1i32;
        let mut probe = value;
        while probe / 10 != 0 {
            probe /= 10;
            digits += 1;
        }
        if number < 0 {
            digits += 1;
        }
        let total = digits * cell_width + (digits - 1) * gap;
        let mut position = match align {
            0 => x + total - cell_width,
            1 => total / 2 + x - cell_width,
            2 => x - cell_width,
            _ => 0,
        };
        if value == 0 {
            self.oldlib_blit(image, 0, 0, cell_width, cell_height, position, y, true);
            self.set_result(0);
            return;
        }
        let mut remaining = value;
        loop {
            let digit = remaining % 10;
            remaining /= 10;
            self.oldlib_blit(
                image,
                (digit as i32) * cell_width,
                0,
                cell_width,
                cell_height,
                position,
                y,
                true,
            );
            position -= cell_width + gap;
            if remaining == 0 {
                break;
            }
        }
        self.set_result(0);
    }

    /// DrawUI (F_0 0x14): nine-slice stretch of a 3x3 (or n x n) UI skin.
    pub(crate) fn draw_ui(&mut self) {
        let image = self.register(0);
        let x = signed_coord(self.register(1));
        let y = signed_coord(self.register(2));
        let width = signed_coord(self.register(3));
        let height = signed_coord(self.argument(4));
        let n = signed_coord(self.argument(5));
        if image == 0 || n <= 0 {
            self.set_result(0);
            return;
        }
        let (image_width, image_height) = self.image_dims(image);
        let cell_width = image_width / n;
        let cell_height = image_height / n;
        if cell_width <= 0 || cell_height <= 0 {
            self.set_result(0);
            return;
        }
        let columns = (width + cell_width - 1) / cell_width;
        let rows = (height + cell_height - 1) / cell_height;
        let saved = self.oldlib_clip;
        self.oldlib_and_clip(x, y, width, height);
        let middle = n - 2;
        let right = x + width - cell_width;
        let last = cell_width * (middle + 1);
        let modulo = |value: i32| -> i32 {
            if middle <= 0 {
                0
            } else {
                value % middle
            }
        };
        for row in 0..=rows {
            let (source_y, destination_y) = if row == 0 {
                (0, y)
            } else if row == rows {
                (cell_height * (middle + 1), y + height - cell_height)
            } else {
                ((modulo(row - 1) + 1) * cell_height, row * cell_height + y)
            };
            self.oldlib_blit(
                image,
                0,
                source_y,
                cell_width,
                cell_height,
                x,
                destination_y,
                false,
            );
            for column in 1..columns {
                self.oldlib_blit(
                    image,
                    (modulo(column - 1) + 1) * cell_width,
                    source_y,
                    cell_width,
                    cell_height,
                    column * cell_width + x,
                    destination_y,
                    false,
                );
            }
            self.oldlib_blit(
                image,
                last,
                source_y,
                cell_width,
                cell_height,
                right,
                destination_y,
                false,
            );
        }
        self.oldlib_clip = saved;
        self.set_result(0);
    }

    /// DrawUIFourXRepeat (F_0 0x18): tile a two-cell (checker) strip.
    pub(crate) fn draw_ui_four_x_repeat(&mut self) {
        let image = self.register(0);
        let x = signed_coord(self.register(1));
        let y = signed_coord(self.register(2));
        let width = signed_coord(self.register(3));
        let height = signed_coord(self.register(4));
        if image == 0 {
            self.set_result(0);
            return;
        }
        let (image_width, image_height) = self.image_dims(image);
        let cell_width = image_width >> 1;
        let cell_height = image_height;
        let saved = self.oldlib_clip;
        self.oldlib_and_clip(x, y, width, height);
        let mut row = 0i32;
        let mut destination_y = y;
        while cell_height > 0 && cell_width > 0 && destination_y < y + height {
            let mut column = 0i32;
            let mut destination_x = x;
            while destination_x < x + width {
                let source_x = if (column + row) & 1 != 0 {
                    cell_width
                } else {
                    0
                };
                self.oldlib_blit(
                    image,
                    source_x,
                    0,
                    cell_width,
                    cell_height,
                    destination_x,
                    destination_y,
                    false,
                );
                destination_x += cell_width;
                column += 1;
            }
            destination_y += cell_height;
            row += 1;
        }
        self.oldlib_clip = saved;
        self.set_result(0);
    }

    /// DrawUISingleRepeat (F_0 0x1c): tile a single cell across the rect.
    pub(crate) fn draw_ui_single_repeat(&mut self) {
        let image = self.register(0);
        let x = signed_coord(self.register(1));
        let y = signed_coord(self.register(2));
        let width = signed_coord(self.register(3));
        let height = signed_coord(self.register(4));
        if image == 0 {
            self.set_result(0);
            return;
        }
        let (cell_width, cell_height) = self.image_dims(image);
        let saved = self.oldlib_clip;
        self.oldlib_and_clip(x, y, width, height);
        let right = x + width;
        let bottom = y + height;
        let mut destination_y = y;
        while cell_height > 0 && cell_width > 0 && bottom >= destination_y {
            let mut destination_x = x;
            while right >= destination_x {
                self.oldlib_blit(
                    image,
                    0,
                    0,
                    cell_width,
                    cell_height,
                    destination_x,
                    destination_y,
                    false,
                );
                destination_x += cell_width;
            }
            destination_y += cell_height;
        }
        self.oldlib_clip = saved;
        self.set_result(0);
    }

    /// DrawUIHorizontal (F_0 0x20): three-slice horizontal stretch.
    pub(crate) fn draw_ui_horizontal(&mut self) {
        let image = self.register(0);
        let x = signed_coord(self.register(1));
        let y = signed_coord(self.register(2));
        let width = signed_coord(self.register(3));
        if image == 0 {
            self.set_result(0);
            return;
        }
        let (image_width, cell_height) = self.image_dims(image);
        let cell_width = image_width / 3;
        if cell_width <= 0 {
            self.set_result(0);
            return;
        }
        let columns = (width + cell_width - 1) / cell_width;
        let saved = self.oldlib_clip;
        self.oldlib_and_clip(x, y, width, cell_height);
        self.oldlib_blit(image, 0, 0, cell_width, cell_height, x, y, false);
        for column in 1..columns {
            self.oldlib_blit(
                image,
                cell_width,
                0,
                cell_width,
                cell_height,
                column * cell_width + x,
                y,
                false,
            );
        }
        self.oldlib_blit(
            image,
            2 * cell_width,
            0,
            cell_width,
            cell_height,
            x + width - cell_width,
            y,
            false,
        );
        self.oldlib_clip = saved;
        self.set_result(0);
    }

    /// IMG_Destory (F_0 0x28): free the image's pixel data (the header may
    /// be a guest-owned static object, so it is only cleared, like the
    /// reference).
    pub(crate) fn release_oldlib_image(&mut self) {
        let image = self.register(0);
        if image != 0 && image != SCREEN_IMAGE_STRUCT {
            let pixels = self.memory.r32(image);
            if pixels != 0 {
                self.deallocate(pixels);
                self.memory.w32(image, 0);
            }
        }
        self.set_result(0);
    }

    /// SetClip (F_0 0x38): (x, y, w, h); returns the bottom edge.
    pub(crate) fn set_oldlib_clip(&mut self) {
        let x = signed_coord(self.register(0));
        let y = signed_coord(self.register(1));
        let width = signed_coord(self.register(2));
        let height = signed_coord(self.register(3));
        self.oldlib_clip = [x, y, x + width, y + height];
        self.set_result(((y + height) as u32) & 0xffff);
    }

    /// IMG_GetHeight (F_0 0x3c).
    pub(crate) fn oldlib_image_height(&mut self) {
        let image = self.register(0);
        let height = if image == 0 {
            0
        } else {
            self.image_dims(image).1 as u32 & 0xffff
        };
        self.set_result(height);
    }

    /// DrawString (F_0 0x24): (string, len16, x, y, color888).
    pub(crate) fn draw_string_oldlib(&mut self) {
        let string = self.register(0);
        let length = signed_coord(self.register(1));
        let x = signed_coord(self.register(2));
        let y = signed_coord(self.register(3));
        let rgb = self.register(4);
        let color =
            ((((rgb >> 19) & 31) << 11) | (((rgb >> 10) & 63) << 5) | ((rgb >> 3) & 31)) as u16;
        let mut bytes = self.read_c_bytes(string, 4096);
        if length >= 0 {
            bytes.truncate(length as usize);
        }
        self.draw_text_bytes(&bytes, x, y, color);
        self.set_result(0);
    }

    pub(crate) fn handle_textbox_method(&mut self, offset: u32) {
        let textbox = self.register(0);
        if textbox == 0 {
            self.set_result(0);
            return;
        }
        match offset {
            0x1c => {
                for i in 0..4u32 {
                    let value = if i < 3 {
                        self.register(i as u8 + 1)
                    } else {
                        self.memory.r32(self.register(reg::SP))
                    };
                    self.memory.w16(textbox + 20 + i * 2, value as u16);
                }
            }
            0x20 | 0x34 => {
                self.memory.w16(
                    textbox + if offset == 0x20 { 6 } else { 4 },
                    self.register(1) as u16,
                );
            }
            0x24 => {
                let text = self.register(1);
                let width = self.memory.r16(textbox + 24) as i16 as i32;
                let height = self.memory.r16(textbox + 26) as i16 as i32;
                let line_height = self.memory.r16(textbox + 6).max(1) as i32;
                let bytes = self.read_c_bytes(text, 65535);
                let mut lines = Vec::new();
                let (mut start, mut position, mut pixels) = (0usize, 0usize, 0i32);
                while position < bytes.len() && lines.len() < 127 {
                    if bytes[position] == b'\n' {
                        lines.push((start, position - start));
                        position += 1;
                        start = position;
                        pixels = 0;
                        continue;
                    }
                    let count = if bytes[position] & 0x80 != 0 && position + 1 < bytes.len() {
                        2
                    } else {
                        1
                    };
                    let glyph_width = if count == 2 { 16 } else { 8 };
                    if position > start
                        && (pixels + glyph_width > width || position + count - start > 255)
                    {
                        lines.push((start, position - start));
                        start = position;
                        pixels = 0;
                        if lines.len() == 127 {
                            break;
                        }
                    }
                    pixels += glyph_width;
                    position += count;
                }
                if position > start && lines.len() < 127 {
                    lines.push((start, position - start));
                }
                if width <= 0 || height <= 0 || text == 0 {
                    lines.clear();
                }
                for field in [8, 12] {
                    let allocation = self.memory.r32(textbox + field);
                    self.deallocate(allocation);
                    self.memory.w32(textbox + field, 0);
                }
                let starts = self.allocate((lines.len() as u32 * 2).max(2));
                let lengths = self.allocate((lines.len() as u32).max(1));
                self.memory.w32(textbox, text);
                self.memory.w32(textbox + 8, starts);
                self.memory.w32(textbox + 12, lengths);
                for (i, (start, length)) in lines.iter().enumerate() {
                    self.memory.w16(starts + i as u32 * 2, *start as u16);
                    self.memory.w8(lengths + i as u32, *length as u8);
                }
                let per_page = (height / line_height).clamp(1, 127) as usize;
                self.memory.w8(textbox + 16, lines.len() as u8);
                self.memory.w8(textbox + 17, per_page as u8);
                self.memory
                    .w8(textbox + 18, lines.len().div_ceil(per_page) as u8);
                self.memory.w8(textbox + 19, 0);
            }
            0x28 | 0x2c => {
                let image = if offset == 0x2c {
                    self.register(1)
                } else {
                    SCREEN_IMAGE_STRUCT
                };
                let rgb = self.register(if offset == 0x2c { 2 } else { 1 });
                let color = ((((rgb >> 19) & 31) << 11)
                    | (((rgb >> 10) & 63) << 5)
                    | ((rgb >> 3) & 31)) as u16;
                let text = self.memory.r32(textbox);
                let starts = self.memory.r32(textbox + 8);
                let lengths = self.memory.r32(textbox + 12);
                let lines = self.memory.r8(textbox + 16) as u32;
                let per_page = self.memory.r8(textbox + 17) as u32;
                let first = self.memory.r8(textbox + 19) as u32 * per_page;
                let style = self.memory.r16(textbox + 4);
                let width = self.memory.r16(textbox + 24) as i16 as i32;
                let height = self.memory.r16(textbox + 26) as i16 as i32;
                let x = self.memory.r16(textbox + 20) as i16 as i32;
                let mut y = self.memory.r16(textbox + 22) as i16 as i32;
                let step = self.memory.r16(textbox + 6) as i32;
                let count = lines.saturating_sub(first).min(per_page);
                if style & 4 != 0 {
                    y += (height - step * count as i32).max(0) / 2;
                }
                for line in first..first + count {
                    let start = self.memory.r16(starts + line * 2) as u32;
                    let length = self.memory.r8(lengths + line) as usize;
                    let mut bytes = self.read_c_bytes(text + start, length as u32);
                    bytes.truncate(length);
                    let (decoded, _, _) = GBK.decode(&bytes);
                    let text_width: i32 = decoded
                        .chars()
                        .map(|c| unifont::get_glyph(c).map_or(16, |g| g.get_width() as i32))
                        .sum();
                    let dx = if style & 2 != 0 {
                        (width - text_width).max(0) / 2
                    } else {
                        0
                    };
                    self.draw_text_bytes_with_height(
                        image,
                        &bytes,
                        x + dx,
                        y,
                        color,
                        (step - 2).clamp(1, 16),
                    );
                    y += step;
                }
            }
            0x30 => {
                for field in [8, 12] {
                    let pointer = self.memory.r32(textbox + field);
                    self.deallocate(pointer);
                    self.memory.w32(textbox + field, 0);
                }
                self.memory.w32(textbox, 0);
                self.memory.w32(textbox + 16, 0);
            }
            _ => {}
        }
        self.set_result(0);
    }

    pub(crate) fn handle_picture_library_method(&mut self, offset: u32) {
        let library = self.register(0);
        let argument = self.register(1);
        if library == 0 {
            self.set_result(u32::MAX);
            return;
        }
        let count = self.memory.r16(library + 20) as u32;
        let capacity = self.memory.r16(library + 8) as u32;
        let images = self.memory.r32(library + 16);
        let ids = self.memory.r32(library + 12);
        let mut target = self.memory.r32(library + 4);
        if target == 0 {
            target = SCREEN_IMAGE_STRUCT;
        }
        match offset {
            0x18 | 0x1c => {
                let resource_id = if offset == 0x1c {
                    let Some(id) = self.resource_id_by_name(argument) else {
                        self.set_result(u32::MAX);
                        return;
                    };
                    for index in 0..count {
                        if self.memory.r16(ids + index * 2) as u32 == id {
                            self.set_result(index);
                            return;
                        }
                    }
                    id
                } else {
                    u32::MAX
                };
                if count >= capacity || ids == 0 || images == 0 {
                    self.set_result(u32::MAX);
                    return;
                }
                let image = if offset == 0x1c {
                    let source = self.resource_by_id(resource_id);
                    self.create_image_from_stream(source, 0)
                } else {
                    let width = argument & 0xffff;
                    let height = self.register(2) & 0xffff;
                    let size = width
                        .next_multiple_of(4)
                        .saturating_mul(height)
                        .saturating_mul(2);
                    // A zero height is legitimate: the reference still
                    // registers the header-only image (data pointer null),
                    // and rejecting it leaves the guest's canvas slot unset
                    // so later scene draws silently target nothing.
                    if width == 0 || size > HEAP_SIZE as u32 {
                        self.set_result(u32::MAX);
                        return;
                    }
                    let header = self.image_header_len();
                    let image = self.allocate(header);
                    let pixels = if size > 0 { self.allocate(size) } else { 0 };
                    if pixels == 0 && size > 0 {
                        self.deallocate(image);
                        self.set_result(u32::MAX);
                        return;
                    }
                    if size > 0 {
                        self.memory.write_bytes(pixels, &vec![0; size as usize]);
                    }
                    self.memory.w32(image, pixels);
                    self.write_image_dims(image, width, height);
                    let kind = self.image_kind_offset();
                    self.memory.w8(image + kind, 1);
                    image
                };
                if image == 0 {
                    self.set_result(u32::MAX);
                    return;
                }
                self.memory.w32(images + count * 4, image);
                self.memory.w16(ids + count * 2, resource_id as u16);
                self.memory.w16(library + 20, (count + 1) as u16);
                self.set_result(count);
            }
            0x20 | 0x24 => {
                let index = argument & 0xffff;
                let image = if index < count {
                    self.memory.r32(images + index * 4)
                } else {
                    0
                };
                let size = if image == 0 {
                    0
                } else {
                    let (width, height) = self.image_dims(image);
                    if offset == 0x20 {
                        width as u32
                    } else {
                        height as u32
                    }
                };
                self.set_result(size);
            }
            0x28 => {
                let x = signed_coord(argument);
                let y = signed_coord(self.register(2));
                let width = signed_coord(self.register(3));
                let stack = self.register(reg::SP);
                let height = signed_coord(self.memory.r32(stack));
                let color = self.memory.r32(stack + 4) as u16;
                let pixels = self.memory.r32(target);
                let (target_width, target_height) = self.image_dims(target);
                self.paint_rect(
                    x,
                    y,
                    width,
                    height,
                    color,
                    false,
                    target_width,
                    target_height,
                    pixels,
                );
                self.set_result(0);
            }
            0x30 | 0x34 | 0x38 | 0x3c | 0x40 => {
                let index = argument & 0xffff;
                if index < count {
                    let image = self.memory.r32(images + index * 4);
                    let (image_width, image_height) = self.image_dims(image);
                    let (dx, dy, sx, sy, width, height) = if offset == 0x30 {
                        (0, 0, 0, 0, image_width, image_height)
                    } else if offset == 0x34 || offset == 0x38 {
                        (
                            signed_coord(self.register(2)),
                            signed_coord(self.register(3)),
                            0,
                            0,
                            image_width,
                            image_height,
                        )
                    } else {
                        let stack = self.register(reg::SP);
                        (
                            signed_coord(self.register(2)),
                            signed_coord(self.register(3)),
                            signed_coord(self.memory.r32(stack)),
                            signed_coord(self.memory.r32(stack + 4)),
                            signed_coord(self.memory.r32(stack + 8)),
                            signed_coord(self.memory.r32(stack + 12)),
                        )
                    };
                    self.blit_image(
                        target,
                        image,
                        sx,
                        sy,
                        width,
                        height,
                        dx,
                        dy,
                        offset == 0x38 || offset == 0x40,
                    );
                }
                self.set_result(0);
            }
            0x4c => {
                self.memory.w32(library + 4, argument);
                let width = if argument == 0 {
                    240
                } else {
                    self.image_dims(argument).0 as u32
                };
                self.set_result(width);
            }
            0x50 => {
                if self.memory.r8(library + 22) == 1 {
                    for index in 0..count {
                        let image = self.memory.r32(images + index * 4);
                        if image != 0 {
                            let pixels = self.memory.r32(image);
                            self.deallocate(pixels);
                            self.deallocate(image);
                        }
                    }
                    self.deallocate(ids);
                    self.deallocate(images);
                    let line = self.memory.r32(library);
                    self.deallocate(line);
                    self.memory.w32(library, 0);
                    self.memory.w32(library + 4, 0);
                    self.memory.w32(library + 12, 0);
                    self.memory.w32(library + 16, 0);
                    self.memory.w16(library + 20, 0);
                    self.memory.w8(library + 22, 0);
                }
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    pub(crate) fn fill_screen_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u16) {
        let left = x.clamp(0, 240);
        let top = y.clamp(0, 400);
        let right = x.saturating_add(width).clamp(0, 240);
        let bottom = y.saturating_add(height).clamp(0, 400);
        for screen_y in top..bottom {
            for screen_x in left..right {
                self.memory.w16(
                    SCREEN_IMAGE + (screen_y as u32 * 240 + screen_x as u32) * 2,
                    color,
                );
            }
        }
    }

    // Text-origin ABI, recovered from guest behavior (issue #45): games draw
    // progress/status text through a helper whose position never reaches
    // DrawText (index 10) directly. Instead the helper submits GetScreenImage
    // (index 1) immediately before each DrawText with the pen origin in r1 (x)
    // / r2 (y) and issues DrawText with zero coordinates. The measured origins
    // match the guest's intended layout exactly (僵尸先生 centers its installer
    // text on the 400-wide display and advances by the measured glyph width),
    // so index 1 latches the origin and index 10 falls back to it when its own
    // coordinates are zero.
    pub(crate) fn handle_lcd_service(&mut self, index: u32) {
        match index {
            0 => self.set_result(SCREEN_IMAGE_STRUCT),
            1 => {
                // GetScreenImage also latches the pen origin for the DrawText
                // call that follows; see the module notes above handle_lcd_service.
                self.latched_text_origin = (
                    signed_coord(self.register(1)),
                    signed_coord(self.register(2)),
                );
                self.set_result(SCREEN_IMAGE);
            }
            5 => self.set_result(if self.register(0) == 0 { 8 } else { 16 }),
            6 => self.set_result(16),
            7 => {
                let bytes = self.read_c_bytes(self.register(0), 4096);
                let (text, _, _) = GBK.decode(&bytes);
                let width = text
                    .chars()
                    .map(|character| {
                        unifont::get_glyph(character)
                            .map(|glyph| glyph.get_width() as u32)
                            .unwrap_or(16)
                    })
                    .sum();
                self.set_result(width);
            }
            9 => {
                let string = self.register(0);
                let x = signed_coord(self.register(1));
                let y = signed_coord(self.register(2));
                let color = self.register(3) as u16;
                self.draw_text(string, x, y, color);
                self.set_result(1);
            }
            10 => {
                let string = self.register(1);
                let raw_x = signed_coord(self.register(2));
                let raw_y = signed_coord(self.register(3));
                // Games that position text through the GetScreenImage latch
                // submit DrawText with zero coordinates; only fall back to the
                // latched origin in that case so explicit coordinates keep
                // their previous meaning.
                let (x, y) = if raw_x == 0 && raw_y == 0 {
                    self.latched_text_origin
                } else {
                    (raw_x, raw_y)
                };
                let color = self.memory.r16(self.register(reg::SP));
                if service_trace_enabled(4, 10) {
                    let bytes = self.read_c_bytes(string, 256);
                    let (text, _, _) = GBK.decode(&bytes);
                    eprintln!("draw text x={x} y={y} color={color:04X} text={text:?}");
                }
                self.draw_text(string, x, y, color);
                self.set_result(1);
            }
            11..=13 => {
                let r0 = self.register(0);
                let r0_is_string = self
                    .memory
                    .region(r0, 1)
                    .is_some_and(|region| region.data[(r0 - region.base) as usize] != 0);
                let (string, x, y, color) = if r0_is_string {
                    (
                        r0,
                        signed_coord(self.register(1)),
                        signed_coord(self.register(2)),
                        self.memory.r32(self.register(reg::SP) + 4) as u16,
                    )
                } else {
                    (
                        self.register(1),
                        signed_coord(self.register(2)),
                        signed_coord(self.register(3)),
                        self.memory.r16(self.register(reg::SP) + 16),
                    )
                };
                self.draw_text(string, x, y, color);
                self.set_result(1);
            }
            16 => {
                if !self.draw_packed_screen_rect(true) {
                    self.draw_rect(false, true);
                }
                self.set_result(1);
            }
            17 => {
                self.draw_rect(true, true);
                self.set_result(1);
            }
            18 => {
                if !self.draw_packed_screen_rect(false) {
                    self.draw_rect(false, false);
                }
                self.set_result(1);
            }
            19 => {
                let x = signed_coord(self.register(0));
                let y = signed_coord(self.register(1));
                let width = signed_coord(self.register(2));
                let height = signed_coord(self.register(3));
                if self.register(0) <= u16::MAX as u32
                    && (-239..240).contains(&x)
                    && (-399..400).contains(&y)
                    && (-239..=240).contains(&width)
                    && (-399..=400).contains(&height)
                {
                    let color = self.memory.r32(self.register(reg::SP)) as u16;
                    self.fill_screen_rect(x, y, width, height, color);
                } else {
                    self.draw_rect(true, false);
                }
                self.set_result(1);
            }
            22 => {
                let image_id = self.register(0);
                let output = self.register(1);
                let local_id = if image_id >= 0xfff {
                    image_id - 0xfff
                } else {
                    image_id
                };
                let result = self.create_image_from_resource_index(local_id as usize, output);
                self.set_result(result);
            }
            23 => self.set_result(0),
            24 => {
                self.draw_image_clip(false);
                self.set_result(1);
            }
            25 => {
                self.draw_image_clip(true);
                self.set_result(1);
            }
            26 | 28 => {
                let packed = self.register(1);
                self.draw_image_at(
                    self.register(0),
                    signed_coord(packed),
                    signed_coord(packed >> 16),
                    index == 28,
                );
                self.set_result(1);
            }
            27 => {
                self.draw_image_at(
                    self.register(0),
                    signed_coord(self.register(1)),
                    signed_coord(self.register(2)),
                    false,
                );
                self.set_result(1);
            }
            29 | 31 => {
                self.draw_image_packed(index == 31);
                self.set_result(1);
            }
            30 | 32 => {
                self.draw_image_full_clip(index == 32);
                self.set_result(1);
            }
            33 | 34 => {
                let image = self.register(0);
                let offset = if index == 33 { 4 } else { 6 };
                let result = self.memory.r16(image + offset) as u32;
                self.set_result(result);
            }
            35 => {
                let image = self.register(0);
                if image != 0 {
                    self.memory.w32(image, 0);
                    self.set_result(1);
                } else {
                    self.set_result(0);
                }
            }
            36 => self.set_result(1),
            38 => {
                let source = self.register(0);
                let destination = self.register(1);
                let capacity = self.register(2) as usize;
                if source == 0 || destination == 0 || capacity == 0 {
                    self.set_result(0);
                } else {
                    let bytes = self.read_c_bytes(source, 4096);
                    let (text, _, _) = GBK.decode(&bytes);
                    let units: Vec<u16> = text.encode_utf16().take(capacity - 1).collect();
                    for (index, unit) in units.iter().enumerate() {
                        self.memory.w16(destination + index as u32 * 2, *unit);
                    }
                    self.memory.w16(destination + units.len() as u32 * 2, 0);
                    self.set_result(units.len() as u32);
                }
            }
            44 => {
                let result = self.initialize_image_data_page(false);
                self.set_result(result);
            }
            45 => {
                let result = self.initialize_image_data_page(true);
                self.set_result(result);
            }
            46 => {
                self.app_image_package = 0;
                self.inner_image_package = 0;
                self.current_image_package = 0;
                self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, 0);
                self.set_result(0);
            }
            47 => {
                let package = self.register(2);
                if package == 0 {
                    self.set_result(0);
                } else {
                    self.current_image_package = package;
                    self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, package);
                    if self.memory.r16(package + 8) == 0 {
                        self.initialize_data_package(package, 5);
                        self.load_main_resource_package(package);
                    }
                    let count = self.memory.r16(package + 8) as u32;
                    self.set_result(count);
                }
            }
            48 => {
                let result = self.create_image_from_data_package(
                    self.register(0),
                    self.register(1),
                    self.register(2),
                );
                self.set_result(result);
            }
            49 => {
                let result = self.create_image_from_stream(self.register(0), self.register(1));
                self.set_result(result);
            }
            54..=56 => self.set_result(1),
            57 => self.set_result(0),
            62 => self.set_result(16),
            // Touch-menu hit test: r0 packs the point and r1/r2 pack the
            // top-left and bottom-right corners as x | (y << 16). Nonzero
            // means the point is inside the inclusive rectangle.
            40 => {
                let x = self.register(0) & 0xffff;
                let y = self.register(0) >> 16;
                let left = self.register(1) & 0xffff;
                let top = self.register(1) >> 16;
                let right = self.register(2) & 0xffff;
                let bottom = self.register(2) >> 16;
                let inside = x >= left && x <= right && y >= top && y <= bottom;
                self.set_result(u32::from(inside));
            }
            90..=92 => self.set_result(0),
            _ => self.set_result(0),
        }
    }

    pub(crate) fn create_image_from_stream(&mut self, source: u32, output: u32) -> u32 {
        if source == 0 {
            return 0;
        }
        let Some(resource_index) = self
            .resource_data
            .iter()
            .position(|pointer| *pointer == source)
        else {
            let Some(size) = self.allocation_size(source) else {
                warn!("image stream at 0x{source:08X} has no allocation boundary");
                return 0;
            };
            let Some(region) = self.memory.region(source, size as usize) else {
                return 0;
            };
            let offset = (source - region.base) as usize;
            let resource = region.data[offset..offset + size as usize].to_vec();
            return self.create_image_from_bytes(&resource, "guest stream", output);
        };
        self.create_image_from_resource_index(resource_index, output)
    }

    pub(crate) fn create_image_from_resource_index(
        &mut self,
        resource_index: usize,
        output: u32,
    ) -> u32 {
        let Some(host_resource) = self.resources.get(resource_index) else {
            return 0;
        };
        let resource = host_resource.data.clone();
        let name = host_resource.name.clone();
        self.create_image_from_bytes(&resource, &name, output)
    }

    fn create_image_from_bytes(&mut self, resource: &[u8], name: &str, output: u32) -> u32 {
        let encoded = image_payload(resource);
        let decoded = match image_decoder::decode_image(encoded) {
            Ok(decoded) => decoded,
            Err(error) => {
                if service_trace_enabled(4, 49) {
                    eprintln!(
                        "image resource {} decode failed (head={:02X?}): {error:#}",
                        name,
                        &resource[..resource.len().min(12)]
                    );
                }
                warn!("failed to decode CBE image resource: {error:#}");
                return 0;
            }
        };
        if decoded.width == 0
            || decoded.height == 0
            || decoded.width > u16::MAX as u32
            || decoded.height > u16::MAX as u32
        {
            return 0;
        }
        if service_trace_enabled(4, 49) {
            let opaque = decoded
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[3] >= 128)
                .count();
            eprintln!(
                "image resource {} decoded={}x{} opaque={opaque}",
                name, decoded.width, decoded.height
            );
        }
        let pitch = decoded.width.next_multiple_of(4);
        let pixels = self.allocate(pitch.saturating_mul(decoded.height).saturating_mul(2));
        if pixels == 0 {
            return 0;
        }
        for y in 0..decoded.height {
            for x in 0..decoded.width {
                let offset = ((y * decoded.width + x) * 4) as usize;
                let red = decoded.data[offset] as u16;
                let green = decoded.data[offset + 1] as u16;
                let blue = decoded.data[offset + 2] as u16;
                let alpha = decoded.data[offset + 3];
                let color = if alpha < 128 {
                    0
                } else {
                    ((red & 0xf8) << 8) | ((green & 0xfc) << 3) | (blue >> 3)
                };
                self.memory.w16(pixels + (y * pitch + x) * 2, color);
            }
        }
        let header = self.image_header_len();
        let image = if output == 0 {
            self.allocate(header)
        } else {
            output
        };
        self.memory.w32(image, pixels);
        let (width, height) = (decoded.width, decoded.height);
        self.write_image_dims(image, width, height);
        let kind = self.image_kind_offset();
        self.memory.w8(image + kind, 1);
        image
    }

    fn draw_image_clip(&mut self, transparent: bool) {
        let destination = self.register(0);
        let source = self.register(1);
        let source_x = signed_coord(self.register(2));
        let source_y = signed_coord(self.register(3));
        let stack = self.register(reg::SP);
        let width = signed_coord(self.memory.r32(stack));
        let height = signed_coord(self.memory.r32(stack + 4));
        let destination_x = signed_coord(self.memory.r32(stack + 8));
        let destination_y = signed_coord(self.memory.r32(stack + 12));
        self.blit_image(
            destination,
            source,
            source_x,
            source_y,
            width,
            height,
            destination_x,
            destination_y,
            transparent,
        );
    }

    fn draw_image_at(&mut self, source: u32, x: i32, y: i32, transparent: bool) {
        let (width, height) = self.image_dims(source);
        self.blit_image(
            SCREEN_IMAGE_STRUCT,
            source,
            0,
            0,
            width,
            height,
            x,
            y,
            transparent,
        );
    }

    fn draw_image_packed(&mut self, transparent: bool) {
        let source = self.register(0);
        let source_start = self.register(1);
        let destination_start = self.register(2);
        let destination_end = self.register(3);
        let source_x = signed_coord(source_start);
        let source_y = signed_coord(source_start >> 16);
        let destination_x = signed_coord(destination_start);
        let destination_y = signed_coord(destination_start >> 16);
        let width = signed_coord(destination_end) - destination_x + 1;
        let height = signed_coord(destination_end >> 16) - destination_y + 1;
        self.blit_image(
            SCREEN_IMAGE_STRUCT,
            source,
            source_x,
            source_y,
            width,
            height,
            destination_x,
            destination_y,
            transparent,
        );
    }

    fn draw_image_full_clip(&mut self, transparent: bool) {
        let source = self.register(0);
        let source_x = signed_coord(self.register(1));
        let source_y = signed_coord(self.register(2));
        let width = signed_coord(self.register(3));
        let stack = self.register(reg::SP);
        let height = signed_coord(self.memory.r32(stack));
        let destination_x = signed_coord(self.memory.r32(stack + 4));
        let destination_y = signed_coord(self.memory.r32(stack + 8));
        self.blit_image(
            SCREEN_IMAGE_STRUCT,
            source,
            source_x,
            source_y,
            width,
            height,
            destination_x,
            destination_y,
            transparent,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn blit_image(
        &mut self,
        mut destination: u32,
        source: u32,
        mut source_x: i32,
        mut source_y: i32,
        mut width: i32,
        mut height: i32,
        mut destination_x: i32,
        mut destination_y: i32,
        transparent: bool,
    ) {
        let source_pixels = self.memory.r32(source);
        let (source_width, source_height) = self.image_dims(source);
        let mut destination_pixels = self.memory.r32(destination);
        let (mut destination_width, mut destination_height) = self.image_dims(destination);
        if service_trace_enabled(4, 24)
            || service_trace_enabled(4, if transparent { 26 } else { 25 })
        {
            eprintln!(
                "draw image dst={destination:08X} src={source:08X} pixels={source_pixels:08X} size={source_width}x{source_height} clip={source_x},{source_y} {width}x{height} at={destination_x},{destination_y}"
            );
        }
        if destination == SCREEN_IMAGE_STRUCT
            || destination_pixels == 0
            || destination_width <= 0
            || destination_height <= 0
            || destination_width > 240
            || destination_height > 400
        {
            destination = SCREEN_IMAGE_STRUCT;
            destination_pixels = SCREEN_IMAGE;
            destination_width = 240;
            destination_height = 400;
        }
        if source_pixels == 0
            || source_width <= 0
            || source_height <= 0
            || width <= 0
            || height <= 0
        {
            return;
        }

        clip_axis(
            &mut source_x,
            &mut width,
            &mut destination_x,
            source_width,
            destination_width,
        );
        clip_axis(
            &mut source_y,
            &mut height,
            &mut destination_y,
            source_height,
            destination_height,
        );
        if width <= 0 || height <= 0 {
            return;
        }
        let source_pitch = ((source_width + 3) & !3) as u32;
        let destination_pitch = ((destination_width + 3) & !3) as u32;
        for row in 0..height as u32 {
            for column in 0..width as u32 {
                let source_offset =
                    ((source_y as u32 + row) * source_pitch + source_x as u32 + column) * 2;
                let color = self.memory.r16(source_pixels + source_offset);
                if !transparent || color != 0 {
                    let destination_offset = ((destination_y as u32 + row) * destination_pitch
                        + destination_x as u32
                        + column)
                        * 2;
                    self.memory
                        .w16(destination_pixels + destination_offset, color);
                }
            }
        }
        let _ = destination;
    }

    fn draw_rect(&mut self, has_destination: bool, outline: bool) {
        let (destination, x, y, width) = if has_destination {
            (
                self.register(0),
                signed_coord(self.register(1)),
                signed_coord(self.register(2)),
                signed_coord(self.register(3)),
            )
        } else {
            (
                SCREEN_IMAGE_STRUCT,
                signed_coord(self.register(0)),
                signed_coord(self.register(1)),
                signed_coord(self.register(2)),
            )
        };
        let stack = self.register(reg::SP);
        let height = signed_coord(self.memory.r32(stack));
        let color = self.memory.r32(stack + 4) as u16;
        let mut pixels = self.memory.r32(destination);
        let (mut destination_width, mut destination_height) = self.image_dims(destination);
        if destination == SCREEN_IMAGE_STRUCT
            || pixels == 0
            || destination_width <= 0
            || destination_height <= 0
            || destination_width > 240
            || destination_height > 400
        {
            pixels = SCREEN_IMAGE;
            destination_width = 240;
            destination_height = 400;
        }
        self.paint_rect(
            x,
            y,
            width,
            height,
            color,
            outline,
            destination_width,
            destination_height,
            pixels,
        );
    }

    /// Draw a screen rectangle from the firmware's packed form: r0 and r1
    /// hold the inclusive corner coordinates (y<<16|x) and r2 the color.
    /// Returns false when the registers do not carry packed coordinates so
    /// the caller falls back to the stack-based form.
    fn draw_packed_screen_rect(&mut self, outline: bool) -> bool {
        let first = self.register(0);
        let second = self.register(1);
        if (first | second) & 0xffff_0000 == 0 {
            return false;
        }
        let x0 = signed_coord(first);
        let y0 = signed_coord(first >> 16);
        let x1 = signed_coord(second);
        let y1 = signed_coord(second >> 16);
        // Data pointers share these registers; reject coordinate halves far
        // outside the screen so such calls keep their stack-form meaning.
        let horizontally_plausible = (-240..=480).contains(&x0) && (-240..=480).contains(&x1);
        let vertically_plausible = (-400..=800).contains(&y0) && (-400..=800).contains(&y1);
        if !horizontally_plausible || !vertically_plausible {
            return false;
        }
        let (x0, x1) = (x0.min(x1), x0.max(x1));
        let (y0, y1) = (y0.min(y1), y0.max(y1));
        let color = self.register(2) as u16;
        self.paint_rect(
            x0,
            y0,
            x1 - x0 + 1,
            y1 - y0 + 1,
            color,
            outline,
            240,
            400,
            SCREEN_IMAGE,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_rect(
        &mut self,
        mut x: i32,
        mut y: i32,
        mut width: i32,
        mut height: i32,
        color: u16,
        outline: bool,
        destination_width: i32,
        destination_height: i32,
        pixels: u32,
    ) {
        let mut source_x = 0;
        let mut source_y = 0;
        clip_axis(
            &mut source_x,
            &mut width,
            &mut x,
            i32::MAX,
            destination_width,
        );
        clip_axis(
            &mut source_y,
            &mut height,
            &mut y,
            i32::MAX,
            destination_height,
        );
        if width <= 0 || height <= 0 {
            return;
        }
        let pitch = ((destination_width + 3) & !3) as u32;
        for row in 0..height {
            for column in 0..width {
                if outline && row != 0 && row != height - 1 && column != 0 && column != width - 1 {
                    continue;
                }
                let offset = ((y + row) as u32 * pitch + (x + column) as u32) * 2;
                self.memory.w16(pixels + offset, color);
            }
        }
    }

    fn draw_text(&mut self, address: u32, x: i32, y: i32, color: u16) {
        let bytes = self.read_c_bytes(address, 4096);
        self.draw_text_bytes(&bytes, x, y, color);
    }

    fn draw_text_bytes(&mut self, bytes: &[u8], x: i32, y: i32, color: u16) {
        self.draw_text_bytes_on_image(SCREEN_IMAGE_STRUCT, bytes, x, y, color);
    }

    fn draw_text_bytes_on_image(&mut self, image: u32, bytes: &[u8], x: i32, y: i32, color: u16) {
        self.draw_text_bytes_with_height(image, bytes, x, y, color, 16);
    }

    fn draw_text_bytes_with_height(
        &mut self,
        image: u32,
        bytes: &[u8],
        x: i32,
        y: i32,
        color: u16,
        glyph_height: i32,
    ) {
        let (text, _, _) = GBK.decode(bytes);
        // Text coordinates live in the presented display space: the firmware
        // renders the glyphs itself, so a landscape-packaged game issues them
        // with 400x240 coordinates that have to be mapped back into the
        // 240x400 framebuffer pixel by pixel (identity for portrait games).
        let screen = image == 0 || image == SCREEN_IMAGE_STRUCT;
        let swaps = screen && self.effective_orientation.swaps_dimensions();
        let (pixels, width, height) = if screen {
            (SCREEN_IMAGE, 240, 400)
        } else {
            let (width, height) = self.image_dims(image);
            (self.memory.r32(image), width, height)
        };
        let (display_width, display_height) = if swaps {
            (height, width)
        } else {
            (width, height)
        };
        if pixels == 0 {
            return;
        }
        let orientation = self.effective_orientation;
        let mut pen_x = x;
        for character in text.chars() {
            let Some(glyph) = unifont::get_glyph(character) else {
                pen_x += 16;
                continue;
            };
            for glyph_y in 0..glyph_height {
                let display_y = y + glyph_y;
                if !(0..display_height).contains(&display_y) {
                    continue;
                }
                for glyph_x in 0..glyph.get_width() as i32 {
                    let display_x = pen_x + glyph_x;
                    if (0..display_width).contains(&display_x)
                        && glyph.get_pixel(glyph_x as usize, (glyph_y * 16 / glyph_height) as usize)
                    {
                        let (screen_x, screen_y) = if swaps {
                            orientation.unrotate(display_x, display_y)
                        } else {
                            (display_x, display_y)
                        };
                        let offset = (screen_y as u32 * width as u32 + screen_x as u32) * 2;
                        self.memory.w16(pixels + offset, color);
                    }
                }
            }
            pen_x += glyph.get_width() as i32;
        }
    }

    pub(crate) fn handle_game_lcd_service(&mut self, index: u32) {
        match index {
            0 => {
                let image = self.create_image_from_stream(self.register(0), 0);
                self.set_result(image);
            }
            1 | 2 => {
                let stack = self.register(reg::SP);
                let height = signed_coord(self.memory.r32(stack));
                let x = signed_coord(self.memory.r32(stack + 4));
                let y = signed_coord(self.memory.r32(stack + 8));
                self.legacy_clipped_blit(
                    SCREEN_IMAGE_STRUCT,
                    self.register(0),
                    signed_coord(self.register(1)),
                    signed_coord(self.register(2)),
                    signed_coord(self.register(3)),
                    height,
                    x,
                    y,
                    index == 2,
                );
                self.set_result(0);
            }
            3 => {
                self.draw_image_at(self.register(0), 0, 0, false);
                self.set_result(0);
            }
            9 => {
                let rgb = self.memory.r32(self.register(reg::SP));
                let color =
                    (((rgb >> 19) & 31) << 11) | (((rgb >> 10) & 63) << 5) | ((rgb >> 3) & 31);
                let length = signed_coord(self.register(1));
                let mut bytes = self.read_c_bytes(self.register(0), 4096);
                if length >= 0 {
                    bytes.truncate(length as usize);
                }
                self.draw_text_bytes(
                    &bytes,
                    signed_coord(self.register(2)),
                    signed_coord(self.register(3)),
                    color as u16,
                );
                self.set_result(0);
            }
            13 | 14 => {
                let offset = if index == 13 { 6 } else { 4 };
                let size = self.memory.r16(self.register(0) + offset) as u32;
                self.set_result(size);
            }
            12 => {
                for offset in 0..4 {
                    self.memory.w16(
                        SCREEN_IMAGE_STRUCT + 12 + offset * 2,
                        self.register(offset as u8) as u16,
                    );
                }
                self.set_result(0);
            }
            21 => {
                let output = self.register(0);
                if output != 0 {
                    for offset in 0..4 {
                        let value = self.memory.r16(SCREEN_IMAGE_STRUCT + 12 + offset * 2);
                        self.memory.w16(output + offset * 2, value);
                    }
                }
                self.set_result(output);
            }
            23..=26 => self.set_result(if index <= 24 { 8 } else { 16 }),
            27 | 28 => {
                let stack = self.register(reg::SP);
                let width = signed_coord(self.memory.r32(stack));
                let height = signed_coord(self.memory.r32(stack + 4));
                let x = signed_coord(self.memory.r32(stack + 8));
                let y = signed_coord(self.memory.r32(stack + 12));
                self.legacy_clipped_blit(
                    self.register(0),
                    self.register(1),
                    signed_coord(self.register(2)),
                    signed_coord(self.register(3)),
                    width,
                    height,
                    x,
                    y,
                    index == 28,
                );
                self.set_result(0);
            }
            32 => self.set_result(SCREEN_IMAGE),
            11 => {
                let image = self.register(0);
                if image != 0 {
                    for offset in (0..12).step_by(4) {
                        self.memory.w32(image + offset, 0);
                    }
                }
                self.set_result(0);
            }
            20 => {
                let result = self.decode_resource_stream(self.register(0));
                self.set_result(result);
            }
            // F_12 InitTextBox is the same builder as the gameold method
            // slot (F_0 0x11c).  Returning zero left callers holding a
            // NULL text box whose next method load jumped through NULL.
            33 => {
                let stub = Self::method_stub_address(METHOD_KIND_GAMEOLD, 0x011c) & !1;
                self.handle_method_stub(stub);
            }
            39 => {
                // Menu-label builder: the caller formats the label into the
                // object's text buffer (+8) and hands us the header, whose two
                // word slots are the object's method table.  They were left
                // null, so the redraw loop's `ldr r7, [obj, #4]; bx r7` jumped
                // to 0 on the first item.
                let object = self.register(0);
                if object != 0 {
                    for offset in (0..8u32).step_by(4) {
                        self.memory.w32(
                            object + offset,
                            Self::method_stub_address(METHOD_KIND_AUTO, offset),
                        );
                    }
                }
                self.set_result(object);
            }
            _ => self.set_result(0),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn legacy_clipped_blit(
        &mut self,
        destination: u32,
        source: u32,
        sx: i32,
        sy: i32,
        width: i32,
        height: i32,
        x: i32,
        y: i32,
        alpha: bool,
    ) {
        // Store the clip in the reserved screen descriptor bytes so memory snapshots preserve it.
        let cx = self.memory.r16(SCREEN_IMAGE_STRUCT + 12) as i16 as i32;
        let cy = self.memory.r16(SCREEN_IMAGE_STRUCT + 14) as i16 as i32;
        let cw = self.memory.r16(SCREEN_IMAGE_STRUCT + 16) as i16 as i32;
        let ch = self.memory.r16(SCREEN_IMAGE_STRUCT + 18) as i16 as i32;
        let left = x.max(cx);
        let top = y.max(cy);
        let right = (x + width).min(cx + cw);
        let bottom = (y + height).min(cy + ch);
        if right > left && bottom > top {
            self.blit_image(
                destination,
                source,
                sx + left - x,
                sy + top - y,
                right - left,
                bottom - top,
                left,
                top,
                alpha,
            );
        }
    }

    pub(crate) fn decode_resource_stream(&mut self, source: u32) -> u32 {
        if source == 0 {
            return 0;
        }
        let compressed_size = u32::from_be_bytes([
            self.memory.r8(source + 1),
            self.memory.r8(source + 2),
            self.memory.r8(source + 3),
            self.memory.r8(source + 4),
        ]);
        let output_size = u32::from_be_bytes([
            self.memory.r8(source + 5),
            self.memory.r8(source + 6),
            self.memory.r8(source + 7),
            self.memory.r8(source + 8),
        ]) & 0x7fff_ffff;
        if service_trace_enabled(16, 20) {
            let name = self
                .resource_data
                .iter()
                .position(|pointer| *pointer == source)
                .map(|index| self.resources[index].name.as_str())
                .unwrap_or("<unknown>");
            eprintln!(
                "decode stream source={source:08X} name={name:?} compressed={compressed_size} output={output_size}"
            );
        }
        if compressed_size == 0 || output_size == 0 || output_size > HEAP_SIZE as u32 {
            return 0;
        }
        let output = self.allocate(output_size);
        if output == 0 {
            return 0;
        }
        let mut source_offset = 0u32;
        let mut output_offset = 0u32;
        while source_offset < compressed_size && output_offset < output_size {
            let command = self.memory.r8(source + 9 + source_offset);
            if command & 0x80 != 0 {
                let count = (command & 0x7f) as u32;
                if count == 0 || source_offset + 1 + count > compressed_size {
                    break;
                }
                let count = count.min(output_size - output_offset);
                for index in 0..count {
                    let byte = self.memory.r8(source + 10 + source_offset + index);
                    self.memory.w8(output + output_offset + index, byte);
                }
                source_offset += count + 1;
                output_offset += count;
            } else {
                if source_offset + 1 >= compressed_size {
                    break;
                }
                let count = (command >> 1) as u32;
                let distance = (((command as u32) << 8) & 0x1ff)
                    | self.memory.r8(source + 10 + source_offset) as u32;
                if count == 0 || distance == 0 || distance > output_offset {
                    break;
                }
                let count = count.min(output_size - output_offset);
                for index in 0..count {
                    let byte = self.memory.r8(output + output_offset - distance + index);
                    self.memory.w8(output + output_offset + index, byte);
                }
                source_offset += 2;
                output_offset += count;
            }
        }
        if service_trace_enabled(16, 20) {
            eprintln!("decode stream wrote={output_offset}");
        }
        if output_offset == 0 {
            0
        } else {
            output
        }
    }
}
