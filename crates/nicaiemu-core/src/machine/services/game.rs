//! Game, fixed-manager, native-dispatch, and game-util services
//! (groups 3, 10, and the fixed/native ABIs).

use armv4t_emu::{reg, Memory};

use super::super::{
    game_service_string_uses_wide_length, signed_coord, NicaiMachine, DREAM_FACTORY_FORMAT_BUFFER,
    DREAM_FACTORY_FORMAT_BUFFER_SIZE, DREAM_FACTORY_MEMORY_BLOCK_SLOT, DREAM_FACTORY_PACKAGE_SLOT,
    FIXED_GAMEOLD_OBJECT_SERVICE, HEAP_BASE, HEAP_SIZE, MEMORY_BLOCK_PTR, NATIVE_DISPATCH_SERVICE,
    NATIVE_SYSTEM_TIME_SERVICE, SCREEN_IS_IN_QUIT, SERVICE_BASE, TABLE_STRIDE,
};

impl NicaiMachine {
    pub(crate) fn handle_game_service(&mut self, index: u32) {
        if let Some(wide_length) = game_service_string_uses_wide_length(index) {
            let result =
                self.read_length_prefixed_string(self.register(0), self.register(1), wide_length);
            self.set_result(result);
            return;
        }
        match index {
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
                self.resource_load_screen = self.pending_screen;
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
            75 if self.uses_fixed_manager_abi() => {
                let object = self.register(0);
                let capacity = self.register(1) & 0xffff;
                let scanline = self.allocate(240 * 2);
                let resource_ids = self.allocate(capacity.saturating_mul(2).max(2));
                let pictures = self.allocate(capacity.saturating_mul(4).max(4));
                self.memory.w32(object, scanline);
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
                let value = self.memory.r16(buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(value as u32);
            }
            92 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = self.memory.r32(buffer.wrapping_add(offset));
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
            _ => self.set_result(0),
        }
    }

    pub(crate) fn handle_fixed_gameold_object_service(&mut self, index: u32) {
        if index == 4 {
            let x = signed_coord(self.register(1));
            let y = signed_coord(self.register(2));
            let width = signed_coord(self.register(3));
            let stack = self.register(reg::SP);
            let height = signed_coord(self.memory.r32(stack));
            let color = self.memory.r32(stack + 4) as u16;
            self.fill_screen_rect(x, y, width, height, color);
        }
        self.set_result(0);
    }

    pub(crate) fn handle_fixed_gameold_region_service(&mut self, index: u32) {
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
                self.memory.w32(object + 4, 0);
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
            self.set_result(0);
            return;
        }
        match id {
            0x79e => {
                if argument != 0 {
                    self.native_app_parser = self.memory.r32(argument);
                    self.native_app_init = self.memory.r32(argument + 4);
                    self.memory.w32(argument + 8, NATIVE_DISPATCH_SERVICE | 1);
                }
                self.set_result(NATIVE_DISPATCH_SERVICE | 1);
            }
            0x52 => {
                if argument != 0 {
                    self.memory
                        .w32(self.executable.data_address() + 0x1724, argument);
                }
                self.set_result(0);
            }
            0x8e | 0x8f | 0x97 | 0xac | 0x421 | 0x41a => self.set_result(id),
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
                self.handle_native_interface_request(argument);
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    fn handle_native_interface_request(&mut self, argument: u32) {
        if argument == 0 {
            return;
        }
        let output = self.memory.r32(argument);
        let handle = self.memory.r32(argument + 4);
        let size = self.memory.r32(argument + 8);
        if output == 0 || size < 4 {
            return;
        }
        self.memory.w32(output, 0);
        match handle {
            0x8f => {
                if self.native_system_info == 0 {
                    self.native_system_info = self.allocate(0x400);
                    let info = self.native_system_info;
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
                    self.memory
                        .w32(info + 0x20c, SERVICE_BASE + TABLE_STRIDE * 6);
                    self.memory
                        .w32(info + 0x210, SERVICE_BASE + TABLE_STRIDE * 6 + 4);
                    self.memory
                        .w32(info + 0x214, SERVICE_BASE + TABLE_STRIDE * 6 + 8);
                    self.memory
                        .w32(info + 0x22c, SERVICE_BASE + TABLE_STRIDE * 6 + 8 * 4);
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
                    self.memory.w32(info + 0xf0, NATIVE_DISPATCH_SERVICE | 1);
                }
                self.memory.w32(output, self.native_system_info);
            }
            0x8e => {
                if self.native_property_info == 0 {
                    self.native_property_info = self.allocate(0x100);
                    self.memory.w32(
                        self.native_property_info + 0x14,
                        NATIVE_DISPATCH_SERVICE | 1,
                    );
                }
                self.memory.w32(output, self.native_property_info);
            }
            0x41a => self.memory.w32(output, u32::MAX),
            _ => {}
        }
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
                let value = self.memory.r16(buffer.wrapping_add(offset));
                self.memory.w32(cursor, offset.wrapping_add(2));
                self.set_result(value as u32);
            }
            20 => {
                let buffer = self.register(0);
                let cursor = self.register(1);
                let offset = self.memory.r32(cursor);
                let value = self.memory.r32(buffer.wrapping_add(offset));
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
            _ => self.set_result(0),
        }
    }
}
