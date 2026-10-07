//! Game, fixed-manager, native-dispatch, and game-util services
//! (groups 3, 10, and the fixed/native ABIs).

use armv4t_emu::{reg, Memory};

use super::super::{
    game_service_string_uses_wide_length, signed_coord, NicaiMachine, DREAM_FACTORY_FORMAT_BUFFER,
    DREAM_FACTORY_FORMAT_BUFFER_SIZE, DREAM_FACTORY_MEMORY_BLOCK_SLOT, DREAM_FACTORY_PACKAGE_SLOT,
    FIXED_GAMEOLD_OBJECT_SERVICE, HEAP_BASE, HEAP_SIZE, MEMORY_BLOCK_PTR, METHOD_KIND_ACTOR,
    METHOD_KIND_AUTO, METHOD_KIND_GAMEOLD, METHOD_KIND_MEMBLOCK, METHOD_KIND_MEMORY,
    METHOD_KIND_PANEL, METHOD_KIND_PICTURE, METHOD_KIND_TEXTBOX, METHOD_STUB_BASE,
    METHOD_STUB_KINDS, METHOD_STUB_STRIDE, NATIVE_DISPATCH_SERVICE, NATIVE_SYSTEM_TIME_SERVICE,
    SCREEN_IS_IN_QUIT, SERVICE_BASE, TABLE_STRIDE,
};

fn read_little_endian_short(memory: &mut impl Memory, address: u32) -> i16 {
    i16::from_le_bytes([memory.r8(address), memory.r8(address.wrapping_add(1))])
}

fn read_little_endian_int(memory: &mut impl Memory, address: u32) -> u32 {
    u32::from_le_bytes([
        memory.r8(address),
        memory.r8(address.wrapping_add(1)),
        memory.r8(address.wrapping_add(2)),
        memory.r8(address.wrapping_add(3)),
    ])
}

fn rect_contains_point(left: i32, top: i32, right: i32, bottom: i32, x: i32, y: i32) -> bool {
    x >= left && x <= right && y >= top && y <= bottom
}

/// Leading-integer parse for the guest `atoi`/`atol` exports.
fn parse_ascii_integer(text: &str) -> u32 {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let negative = index < bytes.len() && bytes[index] == b'-';
    if negative {
        index += 1;
    }
    let mut value: i64 = 0;
    let mut saw_digit = false;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        saw_digit = true;
        value = value
            .saturating_mul(10)
            .saturating_add((bytes[index] - b'0') as i64);
        if value > i32::MAX as i64 {
            value = i32::MAX as i64;
        }
        index += 1;
    }
    if !saw_digit {
        return 0;
    }
    if negative {
        (-(value.min(0x8000_0000))) as i32 as u32
    } else {
        value as u32
    }
}

fn packed_rectangles_overlap(
    first_position: u32,
    first_size: u32,
    second_position: u32,
    second_size: u32,
) -> bool {
    let low = |value: u32| (value as u16 as i16) as i32;
    let high = |value: u32| ((value >> 16) as u16 as i16) as i32;

    low(first_position) + low(first_size) > low(second_position)
        && low(second_position) + low(second_size) > low(first_position)
        && high(first_position) + high(first_size) > high(second_position)
        && high(second_position) + high(second_size) > high(first_position)
}

fn df_sin(degrees: u32) -> i32 {
    let radians = f64::from(degrees as i32) * std::f64::consts::PI / 180.0;
    (radians.sin() * 4096.0) as i32
}

fn df_degree(x: u32, y: u32) -> u32 {
    let x = i64::from(x as i32);
    let y = i64::from(y as i32);
    let length_squared = (i128::from(x) * i128::from(x) + i128::from(y) * i128::from(y)) as u128;
    let mut length = (length_squared as f64).sqrt() as u128;
    while (length + 1) * (length + 1) <= length_squared {
        length += 1;
    }
    while length * length > length_squared {
        length -= 1;
    }

    let mut scaled_y = (y as u32).wrapping_shl(12) as i32;
    if length != 0 {
        scaled_y /= length.min(u32::MAX.into()) as u32 as i32;
    }

    let (begin, end) = if y < 0 {
        if x > 0 {
            (271, 359)
        } else {
            (181, 270)
        }
    } else if x < 0 {
        (91, 180)
    } else {
        (0, 90)
    };
    (begin..=end)
        .find(|&degrees| {
            let sine = df_sin(degrees);
            if end == 90 || end == 359 {
                sine >= scaled_y
            } else {
                sine <= scaled_y
            }
        })
        .unwrap_or(0)
}

impl NicaiMachine {
    pub(crate) fn handle_game_service(&mut self, index: u32) {
        if let Some(wide_length) = game_service_string_uses_wide_length(index) {
            let result =
                self.read_length_prefixed_string(self.register(0), self.register(1), wide_length);
            self.set_result(result);
            return;
        }
        match index {
            9 if self.uses_fixed_manager_abi() => self.handle_game_lcd_service(9),
            28..=31 if self.uses_fixed_manager_abi() => self.handle_game_lcd_service(index - 5),
            0 => {
                let source = self.resource_by_id(self.register(0));
                let result = self.create_image_from_stream(source, 0);
                self.set_result(result);
            }
            1 | 2 if self.uses_fixed_manager_abi() => {
                let source = self.register(0);
                let source_x = signed_coord(self.register(1));
                let source_y = signed_coord(self.register(2));
                let width = signed_coord(self.register(3));
                let stack = self.register(reg::SP);
                let height = signed_coord(self.memory.r32(stack));
                let destination_x = signed_coord(self.memory.r32(stack + 4));
                let destination_y = signed_coord(self.memory.r32(stack + 8));
                self.blit_image(
                    super::super::SCREEN_IMAGE_STRUCT,
                    source,
                    source_x,
                    source_y,
                    width,
                    height,
                    destination_x,
                    destination_y,
                    index == 2,
                );
                self.set_result(0);
            }
            23 => {
                let result = self.decode_resource_stream(self.register(0));
                self.set_result(result);
            }
            50 => {
                let stack = self.register(reg::SP);
                let result = rect_contains_point(
                    signed_coord(self.register(0)),
                    signed_coord(self.register(1)),
                    signed_coord(self.register(2)),
                    signed_coord(self.register(3)),
                    signed_coord(self.memory.r32(stack)),
                    signed_coord(self.memory.r32(stack + 4)),
                );
                self.set_result(u32::from(result));
            }
            58 => {
                let block = self.register(0);
                let size = self.register(1);
                self.initialize_memory_block(block, size);
                self.set_result(block);
            }
            11 => {
                let mask = self.register(0);
                self.set_result(u32::from(self.key_down & mask != 0));
            }
            12 => {
                let mask = self.register(0);
                self.set_result(u32::from(self.key_held & mask != 0));
            }
            14 if self.uses_fixed_manager_abi() => {
                self.set_result(self.register(1).wrapping_add(self.register(3)));
            }
            15 if self.uses_fixed_manager_abi() => {
                let image = self.register(0);
                let height = if image == 0 {
                    0
                } else {
                    self.memory.r16(image + 6) as u32
                };
                self.set_result(height);
            }
            16 if self.uses_fixed_manager_abi() => {
                let image = self.register(0);
                let width = if image == 0 {
                    0
                } else {
                    self.memory.r16(image + 4) as u32
                };
                self.set_result(width);
            }
            17 if self.uses_fixed_manager_abi() => {
                let red = self.register(0) as u16;
                let green = self.register(1) as u16;
                let blue = self.register(2) as u16;
                self.set_result(
                    (((red & 0xf8) << 8) | ((green & 0xfc) << 3) | ((blue & 0xf8) >> 3)) as u32,
                );
            }
            60 => {
                self.pending_screen = self.register(0);
                self.memory.w32(SCREEN_IS_IN_QUIT, 0);
                self.set_result(SCREEN_IS_IN_QUIT);
            }
            61 => {
                self.resource_load_screen = 0;
                self.resource_load_pending = true;
                self.set_result(0);
            }
            62 => self.set_result(u32::from(self.pointer.held)),
            63 => self.set_result(u32::from(self.pointer.down)),
            64 => self.set_result(u32::from(self.pointer.up)),
            65 => self.set_result(u32::from(self.pointer.dragging())),
            66 => self.set_result(self.pointer.x as u32),
            67 => self.set_result(self.pointer.y as u32),
            68 => self.set_result(self.key_down),
            71 => {
                self.handle_method_stub(Self::method_stub_address(METHOD_KIND_GAMEOLD, 0x11c) & !1);
            }
            75 if self.uses_fixed_manager_abi() => {
                let object = self.register(0);
                let capacity = self.register(1) & 0xffff;
                let scanline = self.allocate(240 * 2);
                let resource_ids = self.allocate(capacity.saturating_mul(2).max(2));
                let pictures = self.allocate(capacity.saturating_mul(4).max(4));
                self.memory.w32(object, scanline);
                self.memory.w32(object + 4, 0);
                self.memory.w8(object + 22, 1);
                self.memory.w16(object + 8, capacity as u16);
                self.memory.w32(object + 12, resource_ids);
                self.memory.w32(object + 16, pictures);
                self.memory.w16(object + 20, 0);
                for method in 0..15 {
                    self.memory.w32(
                        object + 0x18 + method * 4,
                        FIXED_GAMEOLD_OBJECT_SERVICE + method * 4,
                    );
                }
                self.set_result(u32::from(
                    scanline != 0 && resource_ids != 0 && pictures != 0,
                ));
            }
            79 if self.uses_fixed_manager_abi() => {
                self.initialize_fixed_gameold_region();
            }
            80 => {
                self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, 0);
                self.memory
                    .w32(DREAM_FACTORY_MEMORY_BLOCK_SLOT, MEMORY_BLOCK_PTR);
                self.set_result(0);
            }
            81 => {
                self.memory
                    .w32(DREAM_FACTORY_PACKAGE_SLOT, self.register(0));
                self.set_result(0);
            }
            82 => {
                let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
                self.set_result(package);
            }
            83 => {
                let result = self.resource_by_id(self.register(0));
                self.set_result(result);
            }
            84 => {
                let result = self.resource_by_name(self.register(0));
                self.set_result(result);
            }
            85 => {
                let result = self.resource_name_by_id(self.register(0));
                self.set_result(result);
            }
            86 => {
                let result = self.resource_id_by_name(self.register(0));
                self.set_result(result.unwrap_or(u32::MAX));
            }
            87 | 88 => {
                let result = self.resource_by_name(self.register(0));
                self.set_result(result);
            }
            90 => {
                let left = self.read_c_bytes(self.register(0), 4096);
                let right = self.read_c_bytes(self.register(1), 4096);
                self.set_result(u32::from(left == right));
            }
            91 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = read_little_endian_short(&mut self.memory, buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(value as i32 as u32);
            }
            92 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = read_little_endian_int(&mut self.memory, buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(4));
                self.set_result(value);
            }
            95 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                self.memory
                    .w16(buffer.wrapping_add(offset), self.register(2) as u16);
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(offset.wrapping_add(2));
            }
            96 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                self.memory
                    .w32(buffer.wrapping_add(offset), self.register(2));
                self.memory.w32(cursor, offset.wrapping_add(4));
                self.set_result(offset.wrapping_add(4));
            }
            102 => self.set_result(MEMORY_BLOCK_PTR),
            103 => self.set_result(df_sin(self.register(0)) as u32),
            104 => self.set_result(df_sin(self.register(0).wrapping_add(90)) as u32),
            105 => self.set_result(df_degree(self.register(0), self.register(1))),
            106 => self.set_result(u32::from(packed_rectangles_overlap(
                self.register(0),
                self.register(1),
                self.register(2),
                self.register(3),
            ))),
            108 => {
                let format = self.read_c_bytes(self.register(0), 4096);
                let output = self.format_c_string_from(&format, 1);
                let length = output
                    .len()
                    .min(DREAM_FACTORY_FORMAT_BUFFER_SIZE.saturating_sub(1));
                self.memory
                    .write_bytes(DREAM_FACTORY_FORMAT_BUFFER, &output[..length]);
                self.memory
                    .w8(DREAM_FACTORY_FORMAT_BUFFER + length as u32, 0);
                self.set_result(DREAM_FACTORY_FORMAT_BUFFER);
            }
            110 => {
                let package = self.register(0);
                let capacity = self.register(1);
                self.initialize_data_package(package, capacity);
            }
            136 => {
                // sprintf(dst, fmt, ...)
                let destination = self.register(0);
                let format = self.read_c_bytes(self.register(1), 4096);
                let output = self.format_c_string_from(&format, 2);
                if destination != 0 {
                    self.memory.write_bytes(destination, &output);
                    self.memory.w8(destination + output.len() as u32, 0);
                }
                self.set_result(output.len() as u32);
            }
            // Firmware C-library routines exposed through the gameold
            // manager. Games call these via directory thunks; returning a
            // null or doing nothing leaves structure fields unfilled and the
            // next indirect call jumps to zero.
            133 => {
                // memcpy(dst, src, n)
                let dst = self.register(0);
                let src = self.register(1);
                let count = self.register(2) as usize;
                if dst != 0 && src != 0 && count != 0 {
                    let mut buffer = vec![0u8; count];
                    for (offset, byte) in buffer.iter_mut().enumerate() {
                        *byte = self.memory.r8(src + offset as u32);
                    }
                    self.memory.write_bytes(dst, &buffer);
                }
                self.set_result(dst);
            }
            134 => {
                // strlen(s)
                let pointer = self.register(0);
                let mut length = 0u32;
                while length < 0x1_0000 && self.memory.r8(pointer + length) != 0 {
                    length += 1;
                }
                self.set_result(length);
            }
            135 => {
                // memset(dst, value, n)
                let dst = self.register(0);
                let value = self.register(1) as u8;
                let count = self.register(2) as usize;
                if dst != 0 && count != 0 {
                    let bytes = vec![value; count];
                    self.memory.write_bytes(dst, &bytes);
                }
                self.set_result(dst);
            }
            140 => {
                // strncpy(dst, src, n) — copy up to n bytes, zero-pad the rest
                let dst = self.register(0);
                let src = self.register(1);
                let count = self.register(2) as usize;
                if dst != 0 && count != 0 {
                    let mut bytes = vec![0u8; count];
                    if src != 0 {
                        for (offset, byte) in bytes.iter_mut().enumerate() {
                            let ch = self.memory.r8(src + offset as u32);
                            *byte = ch;
                            if ch == 0 {
                                break;
                            }
                        }
                    }
                    self.memory.write_bytes(dst, &bytes);
                }
                self.set_result(dst);
            }
            141 => {
                // strcpy(dst, src)
                let dst = self.register(0);
                let src = self.register(1);
                if dst != 0 && src != 0 {
                    let mut bytes = Vec::new();
                    loop {
                        let ch = self.memory.r8(src + bytes.len() as u32);
                        bytes.push(ch);
                        if ch == 0 || bytes.len() >= 0x1_0000 {
                            break;
                        }
                    }
                    self.memory.write_bytes(dst, &bytes);
                }
                self.set_result(dst);
            }
            138 => {
                // rand() — deterministic LCG so guest code that seeds and
                // samples the generator gets a stable stream.
                self.rand_state = self
                    .rand_state
                    .wrapping_mul(1_103_515_245)
                    .wrapping_add(12_345);
                self.set_result((self.rand_state >> 16) & 0x7fff);
            }
            // Firmware exports at F_0 offsets 0x224..0x274 (indices 137..157).
            // Falling through to the object-constructor default here corrupts
            // memory (it treats scalar arguments like 0x3EB as object
            // pointers) and returns the wrong value — 极品飞车 formats the
            // GetPayNum result with an in-place sprintf, so a 4-digit return
            // overruns the format string and walks the pointer table.
            137 => self.set_result(0), // vm_log_trace
            142 => {
                // strcat(dst, src)
                let destination = self.register(0);
                let source = self.register(1);
                if destination != 0 && source != 0 {
                    let mut end = destination;
                    while end.wrapping_sub(destination) < 0x1_0000 && self.memory.r8(end) != 0 {
                        end = end.wrapping_add(1);
                    }
                    let mut offset = 0u32;
                    loop {
                        let byte = self.memory.r8(source.wrapping_add(offset));
                        self.memory.w8(end.wrapping_add(offset), byte);
                        if byte == 0 || offset >= 0x1_0000 {
                            break;
                        }
                        offset += 1;
                    }
                }
                self.set_result(destination);
            }
            143 | 145 => {
                // atol / atoi
                let text = self.read_c_string(self.register(0), 64);
                self.set_result(parse_ascii_integer(&text));
            }
            144 => {
                // memmove(dst, src, n) — copy through a temp buffer so
                // overlapping ranges behave like memmove, not memcpy.
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2).min(0x1_0000);
                if destination != 0 && source != 0 && count != 0 {
                    let bytes: Vec<u8> = (0..count)
                        .map(|offset| self.memory.r8(source.wrapping_add(offset)))
                        .collect();
                    self.memory.write_bytes(destination, &bytes);
                }
                self.set_result(destination);
            }
            146..=156 => {
                // BILLING_* family — same semantics as the dedicated billing
                // group; offline answers keep in-place %d formatting short.
                self.handle_billing_service(index - 146);
            }
            157 => {
                // vMstricmp(a, b) — case-insensitive compare, 0 when equal.
                let left = self.read_c_string(self.register(0), 256);
                let right = self.read_c_string(self.register(1), 256);
                let result = if left.eq_ignore_ascii_case(&right) {
                    0
                } else {
                    let l = left.to_ascii_lowercase();
                    let r = right.to_ascii_lowercase();
                    match l.cmp(&r) {
                        std::cmp::Ordering::Less => u32::MAX,
                        std::cmp::Ordering::Greater => 1,
                        std::cmp::Ordering::Equal => 0,
                    }
                };
                self.set_result(result);
            }
            _ => {
                // Unrecognised gameold ids act as object constructors: the
                // guest passes a struct in r0 and later reads method pointers
                // out of it.  The firmware fills every slot with a callable
                // stub; leaving them null makes the next indirect call jump
                // to zero.  Only zero slots are touched, so scalar-returning
                // services and already-initialised objects are unaffected.
                let obj = self.register(0);
                self.fill_zero_method_slots(obj, 0x100);
                self.set_result(obj);
            }
        }
    }

    pub(crate) fn handle_fixed_gameold_object_service(&mut self, index: u32) {
        self.handle_picture_library_method(0x18 + index * 4);
    }

    pub(crate) fn handle_fixed_gameold_region_service(&mut self, index: u32) -> anyhow::Result<()> {
        let object = self.register(0);
        match index {
            0 => {
                let slot = self.register(2);
                if slot <= 1 {
                    self.memory.w32(object + 0x20 + slot * 4, self.register(1));
                }
                self.set_result(object);
            }
            4 => {
                self.repaint_fixed_gameold_windows(object)?;
                self.set_result(0);
            }
            5 => {
                let rectangle = self.register(2);
                let used = self.memory.r32(object + 4);
                let capacity = self.memory.r32(object + 8);
                let entries = self.memory.r32(object + 12);
                if rectangle != 0 && entries != 0 && used < capacity {
                    let entry = self.memory.r32(entries + used * 4);
                    if entry != 0 {
                        for offset in (0..8).step_by(2) {
                            let value = self.memory.r16(rectangle + offset);
                            self.memory.w16(entry + offset, value);
                        }
                        self.memory.w32(object + 4, used + 1);
                    }
                }
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
        Ok(())
    }

    fn repaint_fixed_gameold_windows(&mut self, root: u32) -> anyhow::Result<()> {
        let mut pending = vec![root];
        let mut visited = std::collections::BTreeSet::new();
        while let Some(object) = pending.pop() {
            if object == 0 || !visited.insert(object) {
                continue;
            }
            let count = self.memory.r32(object + 4).min(self.memory.r32(object + 8));
            let entries = self.memory.r32(object + 12);
            let callback = self.memory.r32(object + 48);
            let context = self.memory.r32(object + 20);
            for i in 0..count {
                let rectangle = self.memory.r32(entries + i * 4);
                for j in 0..4 {
                    let value = self.memory.r16(rectangle + j * 2);
                    self.memory
                        .w16(super::super::SCREEN_IMAGE_STRUCT + 12 + j * 2, value);
                }
                // Guest painters run synchronously and must preserve the calling CPU context.
                let cpu = self.cpu;
                let result =
                    self.invoke_callback(callback, context, 0, 0, crate::DEFAULT_INSTRUCTION_LIMIT);
                self.cpu = cpu;
                result?;
                if self.state == super::super::MachineState::Halted {
                    return Ok(());
                }
            }
            self.memory.w32(object + 4, 0);
            pending.push(self.memory.r32(object + 32));
            pending.push(self.memory.r32(object + 36));
        }
        Ok(())
    }

    pub(crate) fn initialize_fixed_gameold_region(&mut self) {
        let object = self.register(0);
        let first_bounds = self.register(1);
        let second_bounds = self.register(2);
        let owner_a = self.register(3);
        let stack = self.register(reg::SP);
        let owner_b = self.memory.r32(stack);
        let capacity = self.memory.r32(stack + 4);
        let entries = self.allocate(capacity.saturating_mul(4).max(4));
        for index in 0..capacity {
            let rectangle = self.allocate(8);
            self.memory.w32(entries + index * 4, rectangle);
        }
        self.memory.w32(object + 4, 0);
        self.memory.w32(object + 8, capacity);
        self.memory.w32(object + 12, entries);
        self.memory.w32(object + 16, owner_a);
        self.memory.w32(object + 20, owner_b);
        self.memory.w32(object + 24, first_bounds);
        self.memory.w32(object + 28, second_bounds);
        self.memory.w32(object + 32, 0);
        self.memory.w32(object + 36, 0);
        for method in 0..8 {
            self.memory.w32(
                object + 0x28 + method * 4,
                super::super::FIXED_GAMEOLD_REGION_SERVICE + method * 4,
            );
        }
        if capacity != 0 {
            let first = self.memory.r32(entries);
            self.memory.w32(first, first_bounds);
            self.memory.w32(first + 4, second_bounds);
            self.memory.w32(object + 4, 1);
        }
        self.set_result(u32::from(entries != 0));
    }

    pub(crate) fn handle_native_dispatch_service(&mut self) {
        let id = self.register(0);
        let argument = self.register(1);
        let code_start = self.executable.code_address();
        let code_end = code_start.saturating_add(self.executable.code_image_size);
        let data_start = self.executable.data_address();
        let data_end = data_start.saturating_add(self.executable.data_image_size);
        if (code_start..code_end).contains(&id)
            || (data_start..data_end).contains(&id)
            || (HEAP_BASE..HEAP_BASE + HEAP_SIZE as u32).contains(&id)
        {
            // When called as an object constructor (r1 = block size), allocate
            // the block and hand the pointer back through the caller's stack
            // slot the shared template uses for the result. Other calls with
            // a pointer id simply echo zero.
            let size = self.register(1);
            if (16..=0x1000).contains(&size) {
                let block = self.allocate(size);
                let sp = self.register(reg::SP);
                if block != 0 && sp != 0 {
                    self.memory.w32(sp + 68, block);
                }
                self.set_result(block);
            } else {
                self.set_result(0);
            }
            return;
        }
        match id {
            0x79e => {
                if std::env::var_os("CBE_TRACE").is_some() {
                    eprintln!(
                        "[dispatch] id=0x79e arg=0x{argument:08X} r0=0x{:08X} lr=0x{:08X}",
                        self.register(0),
                        self.register(reg::LR)
                    );
                }
                if argument != 0 {
                    self.native_app_parser = self.memory.r32(argument);
                    self.native_app_init = self.memory.r32(argument + 4);
                    self.memory.w32(argument + 8, NATIVE_DISPATCH_SERVICE | 1);
                }
                self.set_result(NATIVE_DISPATCH_SERVICE | 1);
            }
            0x52 => {
                // Native app-object registration: install the standard
                // system API into the registered object.  Games copy these
                // slots (alloc at +0x9c, free at +0xa0) into their own
                // manager objects during boot; leaving them null makes the
                // later mallocBigMen call jump to 0x0.  Zero slots become
                // callable MEMORY stubs so any other read stays safe.
                if argument != 0 {
                    for offset in (0..0x100u32).step_by(4) {
                        if self.memory.r32(argument + offset) == 0 {
                            self.memory.w32(
                                argument + offset,
                                Self::method_stub_address(METHOD_KIND_MEMORY, offset),
                            );
                        }
                    }
                }
                self.set_result(0);
            }
            0x8e | 0x8f | 0x97 | 0xac | 0x421 | 0x41a => {
                if std::env::var_os("CBE_TRACE").is_some() {
                    eprintln!(
                        "[dispatch] id=0x{id:x} r1=0x{:08X} lr=0x{:08X}",
                        self.register(1),
                        self.register(reg::LR)
                    );
                }
                self.set_result(id)
            }
            0x3ed => {
                if argument != 0 {
                    self.memory.w8(argument, 0);
                }
                self.set_result(0);
            }
            0x3ec | 0x3ee => {
                if argument != 0 {
                    for offset in 0..4 {
                        self.memory.w8(argument + offset, 0);
                    }
                }
                self.set_result(0);
            }
            0x7d1 => {
                if std::env::var_os("CBE_TRACE").is_some() {
                    eprintln!(
                        "[dispatch] id=0x7d1 arg=0x{argument:08X} lr=0x{:08X}",
                        self.register(reg::LR)
                    );
                }
                self.handle_native_interface_request(argument);
                self.set_result(0);
            }
            0xb7 | 0xb8 | 0x67 | 0x6b | 0x6e => self.set_result(0),
            _ => {
                if std::env::var_os("CBE_TRACE").is_some() {
                    eprintln!(
                        "[dispatch] id=0x{id:x} r1=0x{:08X} r2=0x{:08X} lr=0x{:08X}",
                        self.register(1),
                        self.register(2),
                        self.register(reg::LR)
                    );
                }
                // The shared template calls a method slot that the firmware
                // leaves sparse. When invoked as a method (r1 = 1), hand back
                // a callable stub address so the caller's `ptr - 52` arithmetic
                // still lands on executable code instead of zero-filled heap.
                if self.register(1) == 1 {
                    let sp = self.register(reg::SP);
                    // The caller subtracts 52 from the +68 slot to recover a
                    // method pointer, so seed it 52 bytes past the stub.  The
                    // +36 slot is used directly as a callable target.
                    let stub = NATIVE_DISPATCH_SERVICE | 1;
                    if sp != 0 {
                        self.memory.w32(sp + 68, stub.wrapping_add(52));
                        self.memory.w32(sp + 36, stub);
                    }
                    self.set_result(stub);
                } else {
                    self.set_result(id)
                }
            }
        }
    }

    /// Address of the per-slot method stub for `offset` bytes into a guest
    /// object of the given table kind.  The low bit is set so a `bx` stays
    /// in Thumb mode.
    pub(crate) fn method_stub_address(kind: u32, offset: u32) -> u32 {
        (METHOD_STUB_BASE
            + (kind % METHOD_STUB_KINDS) * METHOD_STUB_STRIDE
            + (offset % METHOD_STUB_STRIDE))
            | 1
    }

    /// Firmware-style auto result object for an unimplemented manager
    /// method: a fresh block whose every slot is a callable stub that
    /// returns zero.  The guest chains calls through the result, so a bare
    /// zero return would turn the next indirect call into a jump to NULL.
    /// One object is cached per (kind, offset) so repeated calls keep the
    /// same identity, matching the firmware's `auto_result` cache.
    fn auto_result_object(&mut self, kind: u32, offset: u32) -> u32 {
        if let Some(&obj) = self.auto_objects.get(&(kind, offset)) {
            return obj;
        }
        const AUTO_SLOTS: u32 = 24;
        let obj = self.allocate(AUTO_SLOTS * 4);
        if obj == 0 {
            return 0;
        }
        for slot in (0..AUTO_SLOTS * 4).step_by(4) {
            self.memory.w32(
                obj + slot,
                Self::method_stub_address(METHOD_KIND_AUTO, slot),
            );
        }
        self.auto_objects.insert((kind, offset), obj);
        obj
    }

    /// Fill the null word-slots of a guest object with callable per-slot
    /// method stubs so later indirect calls through it land on executable
    /// code with known semantics instead of zero.  Already-populated slots
    /// are left alone.
    fn fill_zero_method_slots(&mut self, obj: u32, size: u32) {
        if obj == 0 {
            return;
        }
        for offset in (0..size).step_by(4) {
            if self.memory.r32(obj + offset) == 0 {
                self.memory.w32(
                    obj + offset,
                    Self::method_stub_address(METHOD_KIND_GAMEOLD, offset),
                );
            }
        }
    }

    /// Dispatch a per-slot object method.  `stub` is the aligned stub address;
    /// the table kind and the byte offset of the slot are encoded in it.  The
    /// memory-manager table (id 143) documents alloc/free/memset at
    /// 0x9c/0xa0/0x214; everything else behaves like the reference's
    /// `h_unimpl` and returns zero.
    pub(crate) fn handle_method_stub(&mut self, stub: u32) {
        let rel = stub.wrapping_sub(METHOD_STUB_BASE);
        let kind = rel / METHOD_STUB_STRIDE;
        let offset = rel % METHOD_STUB_STRIDE;
        let r0 = self.register(0);
        let r1 = self.register(1);
        let r2 = self.register(2);
        let r3 = self.register(3);
        let r4 = self.register(4);
        let r5 = self.register(5);
        if std::env::var_os("CBE_TRACE").is_some() {
            eprintln!(
                "[mstub] kind={kind} off=0x{offset:X} r0=0x{r0:08X} r1=0x{r1:08X} r2=0x{r2:08X}"
            );
        }
        match (kind, offset) {
            (METHOD_KIND_PICTURE, _) => self.handle_picture_library_method(offset),
            (METHOD_KIND_TEXTBOX, _) => self.handle_textbox_method(offset),
            // memset(ptr, val, len) — mirrors h_old_memset, including its
            // 4 MiB length clamp.  Only the memory-manager table owns this
            // slot; a generic table with the same offset is left alone.
            (METHOD_KIND_MEMORY, 0x214) => {
                if r0 != 0 && (1..=0x40_0000).contains(&r2) {
                    let bytes = vec![r1 as u8; r2 as usize];
                    self.memory.write_bytes(r0, &bytes);
                }
                self.set_result(r0);
            }
            // alloc(size)
            (METHOD_KIND_MEMORY, 0x9c) => {
                let block = if r0 == 0 { 0 } else { self.allocate(r0) };
                self.set_result(block);
            }
            // free(ptr)
            (METHOD_KIND_MEMORY, 0xa0) => {
                self.set_result(0);
            }
            // MEMORY_BLOCK bump allocator installed by initMemoryBlock.
            // MB_Malloc(blk, n): 4-byte-aligned carve-out from the backing
            // store, zero-filled like the firmware.
            (METHOD_KIND_MEMBLOCK, 0x0c) => {
                let blk = r0;
                if blk == 0 {
                    self.set_result(0);
                    return;
                }
                let base = self.memory.r32(blk);
                let cursor = self.memory.r32(blk + 4);
                let total = self.memory.r32(blk + 8);
                let size = r1.wrapping_add(3) & !3;
                if cursor.saturating_add(size) > total {
                    self.set_result(0);
                    return;
                }
                self.memory.w32(blk + 4, cursor.saturating_add(size));
                let start = base.saturating_add(cursor);
                if size != 0 {
                    let zeros = vec![0u8; size as usize];
                    self.memory.write_bytes(start, &zeros);
                }
                self.set_result(start);
            }
            // MB_Reset(blk): rewind the bump cursor.
            (METHOD_KIND_MEMBLOCK, 0x10) => {
                if r0 != 0 {
                    self.memory.w32(r0 + 4, 0);
                }
                self.set_result(0);
            }
            // MB_Release(blk): the firmware keeps the backing store; no-op.
            (METHOD_KIND_MEMBLOCK, 0x14) => {
                self.set_result(0);
            }
            // GameManagerOld C-library slots (vmspec F_0).  These mirror the
            // gameold func-list implementations so the table and the dispatch
            // path agree.
            (METHOD_KIND_GAMEOLD, 0x214) => {
                // memcpy(dst, src, n)
                let dst = r0;
                let src = r1;
                let count = r2 as usize;
                if dst != 0 && src != 0 && count != 0 {
                    let mut buf = vec![0u8; count];
                    for (i, b) in buf.iter_mut().enumerate() {
                        *b = self.memory.r8(src + i as u32);
                    }
                    self.memory.write_bytes(dst, &buf);
                }
                self.set_result(dst);
            }
            // The shared template calls slot 0x218 as
            // `fn(dst, src, n)` — a bounded string copy, matching the
            // reference's strncpy(d, s, n) rather than the vmspec name.
            (METHOD_KIND_GAMEOLD, 0x218) => {
                let dst = r0;
                let src = r1;
                let n = r2 as usize;
                if dst != 0 && n != 0 {
                    let mut bytes = vec![0u8; n.min(0x1000)];
                    if src != 0 {
                        for (i, b) in bytes.iter_mut().enumerate() {
                            let ch = self.memory.r8(src + i as u32);
                            *b = ch;
                            if ch == 0 {
                                break;
                            }
                        }
                    }
                    self.memory.write_bytes(dst, &bytes);
                }
                self.set_result(dst);
            }
            (METHOD_KIND_GAMEOLD, 0x21c) => {
                // memset(dst, val, n)
                if r0 != 0 && (1..=0x40_0000).contains(&r2) {
                    let bytes = vec![r1 as u8; r2 as usize];
                    self.memory.write_bytes(r0, &bytes);
                }
                self.set_result(r0);
            }
            // XS_GetParamAsString — XSE script-VM parameter getter.  The
            // firmware exposes no standalone implementation, but the guest
            // passes an output buffer in r0 with a capacity in r2; writing an
            // empty string there and returning the buffer keeps the caller
            // from treating a null as a string pointer.
            // initDFPictureLibrary(lib, n) — builds the DF_PictureLibrary
            // object: an id array, an image-pointer array, and the method
            // table at 0x18..0x50 the guest calls through.  Mirrors the
            // reference's init_picture_library.
            // --- math (vmspec F_0) ---
            (METHOD_KIND_GAMEOLD, 0x00d4) => self.set_result((r0 as i32).unsigned_abs()),
            (METHOD_KIND_GAMEOLD, 0x00d8) => self.set_result(r0.max(r1)),
            (METHOD_KIND_GAMEOLD, 0x00dc) => self.set_result(r0.min(r1)),
            (METHOD_KIND_GAMEOLD, 0x00e0) => {
                self.rand_state = self
                    .rand_state
                    .wrapping_mul(1_103_515_245)
                    .wrapping_add(12_345);
                self.set_result((self.rand_state >> 16) & 0x7fff);
            }
            (METHOD_KIND_GAMEOLD, 0x00e4) => self.set_result((r0 as f64).sqrt() as u32),
            (METHOD_KIND_GAMEOLD, 0x019c) => self.set_result(df_sin(r0) as u32),
            (METHOD_KIND_GAMEOLD, 0x01a0) => self.set_result(df_sin(r0.wrapping_add(90)) as u32),
            (METHOD_KIND_GAMEOLD, 0x01a4) => self.set_result(df_degree(r0, r1)),
            (METHOD_KIND_GAMEOLD, 0x01a8) => {
                self.set_result(u32::from(packed_rectangles_overlap(r0, r1, r2, r3)))
            }
            // --- input ---
            (METHOD_KIND_GAMEOLD, 0x002c) | (METHOD_KIND_GAMEOLD, 0x0110) => {
                self.set_result(self.key_down)
            }
            (METHOD_KIND_GAMEOLD, 0x0030) => self.set_result(u32::from(self.key_held & 1 != 0)),
            (METHOD_KIND_GAMEOLD, 0x00f8) => self.set_result(u32::from(self.pointer.held)),
            (METHOD_KIND_GAMEOLD, 0x00fc) => self.set_result(u32::from(self.pointer.down)),
            (METHOD_KIND_GAMEOLD, 0x0100) => self.set_result(u32::from(self.pointer.up)),
            (METHOD_KIND_GAMEOLD, 0x0104) => self.set_result(u32::from(self.pointer.dragging())),
            (METHOD_KIND_GAMEOLD, 0x0108) => self.set_result(self.pointer.x as u32),
            (METHOD_KIND_GAMEOLD, 0x010c) => self.set_result(self.pointer.y as u32),
            // --- drawing (vmspec F_0) ---
            // FillRect(x, y, w, h, color) — args in r1..r5.
            (METHOD_KIND_GAMEOLD, 0x0058) => {
                let x = r1 as i16 as i32;
                let y = r2 as i16 as i32;
                let w = r3 as i16 as i32;
                let h = r4 as i16 as i32;
                self.fill_screen_rect(x, y, w, h, r5 as u16);
                self.set_result(0);
            }
            // DrawRect(x, y, w, h, color) — outline only.
            (METHOD_KIND_GAMEOLD, 0x0054) => {
                let x = r1 as i16 as i32;
                let y = r2 as i16 as i32;
                let w = r3 as i16 as i32;
                let h = r4 as i16 as i32;
                let c = r5 as u16;
                if w > 0 && h > 0 {
                    self.fill_screen_rect(x, y, w, 1, c);
                    self.fill_screen_rect(x, y + h - 1, w, 1, c);
                    self.fill_screen_rect(x, y, 1, h, c);
                    self.fill_screen_rect(x + w - 1, y, 1, h, c);
                }
                self.set_result(0);
            }
            // --- DF resource accessors ---
            (METHOD_KIND_GAMEOLD, 0x014c) => {
                let v = self.resource_by_id(r0);
                self.set_result(v);
            }
            (METHOD_KIND_GAMEOLD, 0x0150) | (METHOD_KIND_GAMEOLD, 0x015c) => {
                let v = self.resource_by_name(r0);
                self.set_result(v);
            }
            (METHOD_KIND_GAMEOLD, 0x0154) => {
                let v = self.resource_name_by_id(r0);
                self.set_result(v);
            }
            (METHOD_KIND_GAMEOLD, 0x0158) => {
                let v = self.resource_id_by_name(r0).unwrap_or(u32::MAX);
                self.set_result(v);
            }
            (METHOD_KIND_GAMEOLD, 0x0198) => self.set_result(MEMORY_BLOCK_PTR),
            // --- DreamFactory object-graph initialisers ---
            // initDFActor(a, x, y) — actor object with method table at
            // 0x10..0x28 and per-slot state.
            (METHOD_KIND_GAMEOLD, 0x01b4) => {
                let a = r0;
                if a != 0 {
                    self.memory.w16(a, r1 as u16);
                    self.memory.w16(a + 2, r2 as u16);
                    for off in (0x10u32..=0x28).step_by(4) {
                        self.memory
                            .w32(a + off, Self::method_stub_address(METHOD_KIND_ACTOR, off));
                    }
                    for off in [4u32, 6, 8, 10] {
                        self.memory.w32(a + off, 0);
                    }
                }
                self.set_result(a);
            }
            // initDFWindows (init_repaint_panel) — panel with dirty-rect
            // table and method table at 0x28..0x44.
            (METHOD_KIND_GAMEOLD, 0x013c) => {
                let p = r0;
                let n = r5;
                if p != 0 {
                    let table = if n != 0 { self.allocate(4 * n) } else { 0 };
                    self.memory.w32(p + 12, table);
                    if table != 0 {
                        for i in 0..n {
                            let e = self.allocate(8);
                            self.memory.w32(table + 4 * i, e);
                        }
                    }
                    self.memory.w32(p + 4, 0);
                    self.memory.w32(p + 8, n);
                    self.memory.w32(p + 24, r1);
                    self.memory.w32(p + 28, r2);
                    self.memory.w32(p + 16, r3);
                    self.memory.w32(p + 20, r4);
                    for off in (0x28u32..=0x44).step_by(4) {
                        self.memory
                            .w32(p + off, Self::method_stub_address(METHOD_KIND_PANEL, off));
                    }
                    self.memory.w32(p + 32, 0);
                    self.memory.w32(p + 36, 0);
                }
                self.set_result(0);
            }
            // initMemoryBlock(blk, size) — firmware MEMORY_BLOCK descriptor:
            //   +0x00 base, +0x04 cursor, +0x08 total, +0x0c MB_Malloc,
            //   +0x10 MB_Reset, +0x14 MB_Release.  The three trailing slots
            //   must be callable stubs; the guest invokes them directly and
            //   a zero there becomes a jump to NULL.
            (METHOD_KIND_GAMEOLD, 0x00e8) => {
                let mut blk = r0;
                if blk == 0 {
                    blk = self.allocate(0x18);
                    if blk == 0 {
                        self.set_result(0);
                        return;
                    }
                }
                let base = if r1 != 0 { self.allocate(r1) } else { 0 };
                if base != 0 {
                    let zeros = vec![0u8; r1 as usize];
                    self.memory.write_bytes(base, &zeros);
                }
                self.memory.w32(blk, base);
                self.memory.w32(blk + 4, 0);
                self.memory.w32(blk + 8, r1);
                self.memory.w32(
                    blk + 0x0c,
                    Self::method_stub_address(METHOD_KIND_MEMBLOCK, 0x0c),
                );
                self.memory.w32(
                    blk + 0x10,
                    Self::method_stub_address(METHOD_KIND_MEMBLOCK, 0x10),
                );
                self.memory.w32(
                    blk + 0x14,
                    Self::method_stub_address(METHOD_KIND_MEMBLOCK, 0x14),
                );
                self.set_result(blk);
            }
            // InitTextBox(tb, ...) — text box with method table at 0x1c..0x34.
            (METHOD_KIND_GAMEOLD, 0x011c) => {
                let tb = r0;
                if tb != 0 {
                    for (i, off) in [20u32, 22, 24, 26].into_iter().enumerate() {
                        let v = if i < 2 {
                            self.register(2 + i as u8) as u16
                        } else {
                            self.memory.r32(self.register(reg::SP) + (i as u32 - 2) * 4) as u16
                        };
                        self.memory.w16(tb + off, v);
                    }
                    self.memory.w32(tb, 0);
                    for off in (0x1cu32..=0x34).step_by(4) {
                        self.memory.w32(
                            tb + off,
                            Self::method_stub_address(METHOD_KIND_TEXTBOX, off),
                        );
                    }
                    self.memory.w16(tb + 4, 0);
                    self.memory.w16(tb + 6, 14);
                    self.memory.w32(tb + 8, 0);
                    self.memory.w32(tb + 12, 0);
                }
                self.set_result(14);
            }
            (METHOD_KIND_GAMEOLD, 0x012c) => {
                let lib = r0;
                let n = r1 & 0xffff;
                if lib != 0 {
                    let ids = self.allocate((2 * n).max(2));
                    self.memory.w32(lib + 12, ids);
                    let imgs = self.allocate((4 * n).max(4));
                    self.memory.w32(lib + 16, imgs);
                    self.memory.w16(lib + 20, 0);
                    self.memory.w16(lib + 8, n as u16);
                    for method in (0x18..=0x50u32).step_by(4) {
                        self.memory.w32(
                            lib + method,
                            Self::method_stub_address(METHOD_KIND_PICTURE, method),
                        );
                    }
                    let line = self.allocate(480);
                    self.memory.w32(lib, line);
                    self.memory.w32(lib + 4, 0);
                    self.memory.w8(lib + 22, 1);
                }
                self.set_result(lib);
            }
            (METHOD_KIND_GAMEOLD, 0x210) => {
                if r0 != 0 {
                    self.memory.w8(r0, 0);
                    self.set_result(r0);
                } else {
                    self.set_result(0);
                }
            }
            (METHOD_KIND_GAMEOLD, 0x228) => {
                // VmGetRand()
                self.rand_state = self
                    .rand_state
                    .wrapping_mul(1_103_515_245)
                    .wrapping_add(12_345);
                self.set_result((self.rand_state >> 16) & 0x7fff);
            }
            // Slots of an auto-created result object always return zero:
            // the point of the object is that the call lands on a stub
            // instead of NULL, not that it invents another object.
            (METHOD_KIND_AUTO, _) => {
                self.set_result(0);
            }
            _ => {
                // Unlisted slots keep the calling convention the shared stub
                // used to infer: a pointer-shaped first argument is an object
                // method and returns zero (the reference's `h_unimpl`), while
                // a plain id echoes itself and an r1 block size marks an
                // object constructor whose block comes back through the
                // caller's stack slots.
                let code_start = self.executable.code_address();
                let code_end = code_start.saturating_add(self.executable.code_image_size);
                let data_start = self.executable.data_address();
                let data_end = data_start.saturating_add(self.executable.data_image_size);
                let pointer_shaped = (code_start..code_end).contains(&r0)
                    || (data_start..data_end).contains(&r0)
                    || (HEAP_BASE..HEAP_BASE + HEAP_SIZE as u32).contains(&r0);
                if (16..=0x1000).contains(&r1) {
                    let block = self.allocate(r1);
                    let sp = self.register(reg::SP);
                    if block != 0 && sp != 0 {
                        self.memory.w32(sp + 68, block);
                    }
                    self.set_result(block);
                } else if pointer_shaped {
                    // An unimplemented object method still has to hand back
                    // something the guest can chain through: a fresh block of
                    // callable zero-returning stubs.  A bare zero would turn
                    // the next `ldr r1, [obj, #off]; bx r1` into a jump to
                    // NULL.  This mirrors the firmware's `auto_result`.
                    let obj = self.auto_result_object(kind, offset);
                    self.set_result(obj);
                } else if r1 == 1 {
                    // Constructor-shaped call through an id: hand back a
                    // callable stub via the caller's `ptr - 52` slot.
                    let stub = NATIVE_DISPATCH_SERVICE | 1;
                    let sp = self.register(reg::SP);
                    if sp != 0 {
                        self.memory.w32(sp + 68, stub.wrapping_add(52));
                        self.memory.w32(sp + 36, stub);
                    }
                    self.set_result(stub);
                } else {
                    self.set_result(r0);
                }
            }
        }
    }

    fn handle_native_interface_request(&mut self, argument: u32) {
        if argument == 0 {
            return;
        }
        let output = self.memory.r32(argument);
        let handle = self.memory.r32(argument + 4);
        let size = self.memory.r32(argument + 8);
        // Results are written as u32 for size >= 4 and u16 for size 2..3: the
        // shared template's measure-call marshals a 2-byte result slot and
        // reads it back after the request returns.
        if output == 0 || size < 2 {
            return;
        }
        let value = match handle {
            0x8f => {
                if self.native_system_info == 0 {
                    let info = self.build_native_system_info();
                    self.native_system_info = info;
                }
                Some(self.native_system_info)
            }
            0x8e => {
                if self.native_property_info == 0 {
                    let info = self.allocate(0x100);
                    self.memory.w32(info + 0x14, NATIVE_DISPATCH_SERVICE | 1);
                    self.native_property_info = info;
                }
                Some(self.native_property_info)
            }
            0x41a => Some(u32::MAX),
            // Shared-template measurement request (id computed as 0x7f << 3):
            // its result feeds the render loop's terminate check.  A stable
            // zero ends the loop instead of the stack being eaten by the
            // stale stack-slot value; unknown handles keep the firmware's
            // zero result.
            _ => Some(0),
        };
        let Some(value) = value else {
            return;
        };
        if size >= 4 {
            self.memory.w32(output, value);
        } else {
            self.memory.w16(output, value as u16);
        }
    }

    /// One-time native system-info object: every slot is a callable stub so
    /// sparse guest indexing stays off NULL, with the known firmware slots
    /// rebound to their real services.
    fn build_native_system_info(&mut self) -> u32 {
        let info = self.allocate(0x400);
        // Fill every slot with a per-slot callable stub so guest
        // code that indexes unlisted offsets still gets a valid
        // function pointer with known semantics.
        for offset in (0..0x400u32).step_by(4) {
            self.memory.w32(
                info + offset,
                Self::method_stub_address(METHOD_KIND_MEMORY, offset),
            );
        }
        self.memory
            .w32(info + 0x9c, SERVICE_BASE + TABLE_STRIDE * 2 + 13 * 4);
        self.memory
            .w32(info + 0xa0, SERVICE_BASE + TABLE_STRIDE * 2 + 14 * 4);
        self.memory
            .w32(info + 0x24, SERVICE_BASE + TABLE_STRIDE * 4 + 9 * 4);
        self.memory
            .w32(info + 0x58, SERVICE_BASE + TABLE_STRIDE * 4 + 19 * 4);
        self.memory
            .w32(info + 0x70, SERVICE_BASE + TABLE_STRIDE * 4 + 5 * 4);
        self.memory
            .w32(info + 0x74, SERVICE_BASE + TABLE_STRIDE * 4 + 5 * 4);
        self.memory
            .w32(info + 0x78, SERVICE_BASE + TABLE_STRIDE * 4 + 6 * 4);
        self.populate_table(info + 0x20c, SERVICE_BASE + TABLE_STRIDE * 6, 22);
        for (offset, index) in [
            (0xa4, 2),
            (0xa8, 1),
            (0xac, 0),
            (0xb0, 3),
            (0xb4, 4),
            (0xb8, 5),
        ] {
            self.memory
                .w32(info + offset, NATIVE_SYSTEM_TIME_SERVICE + index * 4);
        }
        // 0xf0 is the native-dispatch entry the guest routes
        // arbitrary ids through; it must stay the shared dispatch
        // stub rather than a per-slot method.
        self.memory.w32(info + 0xf0, NATIVE_DISPATCH_SERVICE | 1);
        // 0xC0 is read by the shared big-endian template as a
        // dispatch-style entry point.
        self.memory.w32(info + 0xC0, NATIVE_DISPATCH_SERVICE | 1);
        // The shared template indexes a BSS method table through
        // a pointer at 0x043F98DC; fill its null slots too.
        let table = self.memory.r32(0x043F98DC);
        if table != 0 {
            self.fill_zero_method_slots(table, 0x400);
        }
        info
    }

    pub(crate) fn handle_game_util_service(&mut self, index: u32) {
        match index {
            9 => {
                self.memory
                    .w32(DREAM_FACTORY_PACKAGE_SLOT, self.register(0));
                self.set_result(0);
            }
            10 => {
                let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
                self.set_result(package);
            }
            11 => {
                let result = self.resource_by_id(self.register(0));
                self.set_result(result);
            }
            12 | 15 | 16 => {
                let result = self.resource_by_name(self.register(0));
                self.set_result(result);
            }
            13 => {
                let result = self.resource_name_by_id(self.register(0));
                self.set_result(result);
            }
            14 => {
                let result = self.resource_id_by_name(self.register(0));
                self.set_result(result.unwrap_or(u32::MAX));
            }
            18 => {
                let left = self.read_c_string(self.register(0), 4096);
                let right = self.read_c_string(self.register(1), 4096);
                self.set_result(u32::from(left == right));
            }
            19 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = read_little_endian_short(&mut self.memory, buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(value as i32 as u32);
            }
            20 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = read_little_endian_int(&mut self.memory, buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(4));
                self.set_result(value);
            }
            23 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                self.memory
                    .w16(buffer.wrapping_add(offset), self.register(2) as u16);
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(offset.wrapping_add(2));
            }
            24 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                self.memory
                    .w32(buffer.wrapping_add(offset), self.register(2));
                self.memory.w32(cursor, offset.wrapping_add(4));
                self.set_result(offset.wrapping_add(4));
            }
            25 => {
                let result =
                    self.read_length_prefixed_string(self.register(0), self.register(1), false);
                self.set_result(result);
            }
            29 => {
                let result =
                    self.read_length_prefixed_string(self.register(0), self.register(1), true);
                self.set_result(result);
            }
            30 => self.set_result(MEMORY_BLOCK_PTR),
            31 => self.set_result(df_sin(self.register(0)) as u32),
            32 => self.set_result(df_sin(self.register(0).wrapping_add(90)) as u32),
            33 => self.set_result(df_degree(self.register(0), self.register(1))),
            34 => self.set_result(u32::from(packed_rectangles_overlap(
                self.register(0),
                self.register(1),
                self.register(2),
                self.register(3),
            ))),
            _ => self.set_result(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        df_degree, df_sin, packed_rectangles_overlap, read_little_endian_int,
        read_little_endian_short, rect_contains_point,
    };
    use crate::machine::memory::MachineMemory;

    fn pack(high: i16, low: i16) -> u32 {
        u32::from(low as u16) | (u32::from(high as u16) << 16)
    }

    #[test]
    fn game_short_reads_are_little_endian_and_signed() {
        for big_endian in [false, true] {
            let mut memory = MachineMemory::new(big_endian);
            memory.map(0x1000, 2, false);
            memory.load(0x1000, &[0x19, 0x00]).unwrap();
            assert_eq!(read_little_endian_short(&mut memory, 0x1000), 0x19);
            memory.load(0x1000, &[0x00, 0x80]).unwrap();
            assert_eq!(read_little_endian_short(&mut memory, 0x1000), i16::MIN);
        }
    }

    #[test]
    fn game_int_reads_are_little_endian_independent_of_guest_endianness() {
        for big_endian in [false, true] {
            let mut memory = MachineMemory::new(big_endian);
            memory.map(0x1000, 4, false);
            memory.load(0x1000, &[0x78, 0x56, 0x34, 0x12]).unwrap();
            assert_eq!(read_little_endian_int(&mut memory, 0x1000), 0x1234_5678);
        }
    }

    #[test]
    fn rectangle_point_test_includes_edges() {
        assert!(rect_contains_point(78, 178, 161, 196, 78, 178));
        assert!(rect_contains_point(78, 178, 161, 196, 120, 187));
        assert!(rect_contains_point(78, 178, 161, 196, 161, 196));
    }

    #[test]
    fn rectangle_point_test_rejects_outside_coordinates() {
        assert!(!rect_contains_point(78, 178, 161, 196, 77, 187));
        assert!(!rect_contains_point(78, 178, 161, 196, 120, 197));
    }

    #[test]
    fn packed_rectangle_test_detects_overlap() {
        assert!(packed_rectangles_overlap(
            pack(20, 10),
            pack(8, 12),
            pack(25, 18),
            pack(10, 6),
        ));
        assert!(packed_rectangles_overlap(
            pack(-8, -10),
            pack(10, 12),
            pack(-4, -3),
            pack(8, 6),
        ));
    }

    #[test]
    fn packed_rectangle_test_excludes_touching_edges() {
        assert!(!packed_rectangles_overlap(
            pack(20, 10),
            pack(8, 12),
            pack(28, 18),
            pack(10, 6),
        ));
        assert!(!packed_rectangles_overlap(
            pack(20, 10),
            pack(8, 12),
            pack(25, 22),
            pack(10, 6),
        ));
    }

    #[test]
    fn fixed_point_trigonometry_matches_cardinal_directions() {
        assert_eq!(df_sin(0), 0);
        assert_eq!(df_sin(90), 4096);
        assert_eq!(df_sin(180), 0);
        assert_eq!(df_sin(270), -4096);

        assert_eq!(df_degree(1, 0), 0);
        assert_eq!(df_degree(0, 1), 90);
        assert_eq!(df_degree((-1_i32) as u32, 0), 180);
        assert_eq!(df_degree(0, (-1_i32) as u32), 270);
    }
}
