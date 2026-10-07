//! Bounded DF record files with little-endian section headers and values.

use armv4t_emu::Memory;

use super::super::{NicaiMachine, SERVICE_BASE, TABLE_STRIDE};

impl NicaiMachine {
    pub(crate) fn initialize_record(&mut self) {
        let object = self.register(0);
        let capacity = self.register(2);
        let sections = self.register(3);
        if object == 0 || !(2..=0xffff).contains(&capacity) || sections > 0xffff {
            self.set_result(0);
            return;
        }
        let buffer = self.allocate(capacity);
        self.memory.w32(object, self.register(1));
        self.memory.w32(object + 4, buffer);
        self.memory.w32(object + 8, capacity);
        self.memory.w16(object + 12, 0);
        self.memory.w16(object + 14, sections as u16);
        self.memory.w32(object + 16, 2);
        self.memory.w32(object + 20, 0);
        for index in 0..10 {
            self.memory.w32(
                object + 0x18 + index * 4,
                SERVICE_BASE + TABLE_STRIDE * 23 + index * 4,
            );
        }
        self.set_result(object);
    }

    pub(crate) fn handle_record_service(&mut self, index: u32) {
        let object = self.register(0);
        if object == 0 {
            self.set_result(0);
            return;
        }
        let buffer = self.memory.r32(object + 4);
        let capacity = self.memory.r32(object + 8);
        if buffer == 0 || !(2..=0xffff).contains(&capacity) {
            self.set_result(0);
            return;
        }
        let result = match index {
            0 => self.load_record(object, buffer, capacity),
            1 => {
                let name = self.memory.r32(object);
                let path = self.read_c_string(name, 1024);
                let bytes = (0..capacity).map(|i| self.memory.r8(buffer + i)).collect();
                u32::from(self.virtual_fs.write_file(&path, bytes))
            }
            2 => {
                self.memory.write_bytes(buffer, &vec![0; capacity as usize]);
                self.memory.w16(object + 12, 0);
                self.memory.w32(object + 16, 2);
                self.memory.w32(object + 20, 0);
                0
            }
            3 => self.append_record_section(object, buffer, capacity),
            4..=9 => {
                let width = [1, 2, 4][((index - 4) % 3) as usize];
                let section = self.register(1);
                let cursor = self.memory.r32(object + 20);
                if let Some((start, size)) = self.record_section(object, buffer, capacity, section)
                {
                    if cursor <= size && width <= size - cursor {
                        let address = start + cursor;
                        let value = if index >= 7 {
                            let value = self.register(2);
                            self.memory
                                .write_bytes(address, &value.to_le_bytes()[..width as usize]);
                            value
                        } else {
                            let mut bytes = [0; 4];
                            for i in 0..width {
                                bytes[i as usize] = self.memory.r8(address + i);
                            }
                            u32::from_le_bytes(bytes)
                        };
                        self.memory.w32(object + 20, cursor + width);
                        value
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            _ => 0,
        };
        self.set_result(result);
    }

    fn load_record(&mut self, object: u32, buffer: u32, capacity: u32) -> u32 {
        let name = self.memory.r32(object);
        let path = self.read_c_string(name, 1024);
        let handle = self.virtual_fs.open(&path, "rb", 2);
        let bytes = if handle >= 0 {
            let bytes = self.virtual_fs.read(handle as u32, capacity as usize + 1);
            self.virtual_fs.close(handle as u32);
            bytes.unwrap_or_default()
        } else {
            Vec::new()
        };
        let limit = self.memory.r16(object + 14) as usize;
        let parsed = (|| {
            let count = u16::from_le_bytes(bytes.get(..2)?.try_into().ok()?) as usize;
            if count > limit || bytes.len() > capacity as usize {
                return None;
            }
            let mut used = 2usize;
            for _ in 0..count {
                let length =
                    u16::from_le_bytes(bytes.get(used..used + 2)?.try_into().ok()?) as usize;
                used = used.checked_add(2 + length)?;
                if used > bytes.len() {
                    return None;
                }
            }
            Some((count as u16, used as u32))
        })();
        self.memory.write_bytes(buffer, &vec![0; capacity as usize]);
        self.memory.w16(object + 12, 0);
        self.memory.w32(object + 16, 2);
        self.memory.w32(object + 20, 0);
        if let Some((count, used)) = parsed {
            self.memory.write_bytes(buffer, &bytes);
            self.memory.w16(object + 12, count);
            self.memory.w32(object + 16, used);
            1
        } else {
            0
        }
    }

    fn append_record_section(&mut self, object: u32, buffer: u32, capacity: u32) -> u32 {
        let count = self.memory.r16(object + 12);
        let limit = self.memory.r16(object + 14);
        let used = self.memory.r32(object + 16);
        let size = self.register(1);
        if count >= limit || used > capacity || size > 0xffff || size + 2 > capacity - used {
            return 0;
        }
        self.memory
            .write_bytes(buffer + used, &(size as u16).to_le_bytes());
        self.memory
            .write_bytes(buffer + used + 2, &vec![0; size as usize]);
        self.memory.write_bytes(buffer, &(count + 1).to_le_bytes());
        self.memory.w16(object + 12, count + 1);
        self.memory.w32(object + 16, used + size + 2);
        self.memory.w32(object + 20, 0);
        1
    }

    fn record_section(
        &mut self,
        object: u32,
        buffer: u32,
        capacity: u32,
        section: u32,
    ) -> Option<(u32, u32)> {
        let count = self.memory.r16(object + 12) as u32;
        if section >= count {
            return None;
        }
        let mut offset = 2;
        for current in 0..=section {
            if offset + 2 > capacity {
                return None;
            }
            let size = u16::from_le_bytes([
                self.memory.r8(buffer + offset),
                self.memory.r8(buffer + offset + 1),
            ]) as u32;
            if size > capacity - offset - 2 {
                return None;
            }
            offset += 2;
            if current == section {
                return Some((buffer + offset, size));
            }
            offset += size;
        }
        None
    }
}
