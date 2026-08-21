//! Guest framebuffer drawing: LCD services, blits, rects, and text.

use armv4t_emu::{reg, Memory};
use encoding_rs::GBK;
use log::warn;

use super::{
    clip_axis, image_payload, service_trace_enabled, signed_coord, NicaiMachine,
    DREAM_FACTORY_PACKAGE_SLOT, HEAP_SIZE, SCREEN_IMAGE, SCREEN_IMAGE_STRUCT,
};
use crate::image_decoder;

impl NicaiMachine {
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

    pub(crate) fn handle_lcd_service(&mut self, index: u32) {
        match index {
            0 => self.set_result(SCREEN_IMAGE_STRUCT),
            1 => self.set_result(SCREEN_IMAGE),
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
                let x = signed_coord(self.register(2));
                let y = signed_coord(self.register(3));
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
                self.draw_rect(false, true);
                self.set_result(1);
            }
            17 => {
                self.draw_rect(true, true);
                self.set_result(1);
            }
            18 => {
                self.draw_rect(false, false);
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
        let image = if output == 0 {
            self.allocate(12)
        } else {
            output
        };
        self.memory.w32(image, pixels);
        self.memory.w16(image + 4, decoded.width as u16);
        self.memory.w16(image + 6, decoded.height as u16);
        self.memory.w8(image + 8, 1);
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
        let width = self.memory.r16(source + 4) as i32;
        let height = self.memory.r16(source + 6) as i32;
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
        let source_width = self.memory.r16(source + 4) as i32;
        let source_height = self.memory.r16(source + 6) as i32;
        let mut destination_pixels = self.memory.r32(destination);
        let mut destination_width = self.memory.r16(destination + 4) as i32;
        let mut destination_height = self.memory.r16(destination + 6) as i32;
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
        let (mut destination, mut x, mut y, mut width) = if has_destination {
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
        let mut height = signed_coord(self.memory.r32(stack));
        let color = self.memory.r32(stack + 4) as u16;
        let mut pixels = self.memory.r32(destination);
        let mut destination_width = self.memory.r16(destination + 4) as i32;
        let mut destination_height = self.memory.r16(destination + 6) as i32;
        if destination == SCREEN_IMAGE_STRUCT
            || pixels == 0
            || destination_width <= 0
            || destination_height <= 0
            || destination_width > 240
            || destination_height > 400
        {
            destination = SCREEN_IMAGE_STRUCT;
            pixels = SCREEN_IMAGE;
            destination_width = 240;
            destination_height = 400;
        }
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
        let _ = destination;
    }

    fn draw_text(&mut self, address: u32, x: i32, y: i32, color: u16) {
        let bytes = self.read_c_bytes(address, 4096);
        let (text, _, _) = GBK.decode(&bytes);
        let mut pen_x = x;
        for character in text.chars() {
            let Some(glyph) = unifont::get_glyph(character) else {
                pen_x += 16;
                continue;
            };
            for glyph_y in 0..16i32 {
                let screen_y = y + glyph_y;
                if !(0..400).contains(&screen_y) {
                    continue;
                }
                for glyph_x in 0..glyph.get_width() as i32 {
                    let screen_x = pen_x + glyph_x;
                    if (0..240).contains(&screen_x)
                        && glyph.get_pixel(glyph_x as usize, glyph_y as usize)
                    {
                        let offset = (screen_y as u32 * 240 + screen_x as u32) * 2;
                        self.memory.w16(SCREEN_IMAGE + offset, color);
                    }
                }
            }
            pen_x += glyph.get_width() as i32;
        }
    }

    pub(crate) fn handle_game_lcd_service(&mut self, index: u32) {
        match index {
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
            _ => self.set_result(0),
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
