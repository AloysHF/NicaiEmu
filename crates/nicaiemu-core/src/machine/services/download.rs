//! Download, payment, and download-resource services (groups 22-26).

use armv4t_emu::Memory;

use super::super::{NicaiMachine, DREAM_FACTORY_PACKAGE_SLOT, HEAP_SIZE};

impl NicaiMachine {
    pub(crate) fn handle_download_service(&mut self, index: u32) {
        if index == 4 {
            self.set_result(u32::MAX);
        } else {
            self.set_result(0);
        }
    }

    pub(crate) fn handle_payment_service(&mut self, index: u32) {
        if index == 7 {
            let destination = self.register(0);
            let capacity = self.register(1);
            let identifier = b"111111111111111\0";
            let count = capacity.min(identifier.len() as u32);
            if destination != 0 && count != 0 {
                self.memory
                    .write_bytes(destination, &identifier[..count as usize]);
                self.memory.w8(destination + count - 1, 0);
            }
        }
        self.set_result(0);
    }

    pub(crate) fn handle_download_resource_service(&mut self, index: u32) {
        match index {
            0 => {
                let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
                self.set_result(package);
            }
            10 => {
                let result = self.create_image_from_stream(self.register(0), self.register(1));
                self.set_result(result);
            }
            _ => self.set_result(0),
        }
    }

    pub(crate) fn handle_download_image_service(&mut self, index: u32) {
        if index == 4 {
            let pointer = self.allocate(self.register(0));
            self.set_result(pointer);
        } else {
            self.set_result(0);
        }
    }

    pub(crate) fn read_length_prefixed_string(
        &mut self,
        buffer: u32,
        cursor: u32,
        wide_length: bool,
    ) -> u32 {
        let mut offset = self.memory.r32(cursor);
        let length = if wide_length {
            let length = self.memory.r32(buffer.wrapping_add(offset));
            offset = offset.wrapping_add(4);
            length
        } else {
            let length = self.memory.r8(buffer.wrapping_add(offset)) as u32;
            offset = offset.wrapping_add(1);
            length
        };
        if length > HEAP_SIZE as u32 {
            return 0;
        }
        let result = self.allocate(length.saturating_add(1).max(1));
        for index in 0..length {
            let byte = self.memory.r8(buffer.wrapping_add(offset + index));
            self.memory.w8(result + index, byte);
        }
        self.memory.w8(result + length, 0);
        self.memory.w32(cursor, offset.wrapping_add(length));
        result
    }
}
