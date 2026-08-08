//! UCS2 string services (group 13).

use armv4t_emu::Memory;
use encoding_rs::GBK;

use super::super::{ascii_uppercase, NicaiMachine};

impl NicaiMachine {
    pub(crate) fn handle_ucs2_service(&mut self, index: u32) {
        match index {
            0 => {
                let length = self.ucs2_len(self.register(0), 0x8000);
                self.set_result(length);
            }
            1 => {
                let destination = self.register(0);
                let source = self.register(1);
                let length = self.ucs2_len(source, 0x8000);
                self.copy_ucs2(destination, source, length + 1);
                self.set_result(destination);
            }
            2 => {
                let destination = self.register(0);
                let source = self.register(1);
                let destination_length = self.ucs2_len(destination, 512);
                let source_length = self.ucs2_len(source, 0x8000);
                self.copy_ucs2(
                    destination + destination_length * 2,
                    source,
                    source_length + 1,
                );
                self.set_result(destination);
            }
            3 => {
                let destination = self.register(0);
                let bytes = self.read_c_bytes(self.register(1), 0x8000);
                let (text, _, _) = GBK.decode(&bytes);
                for (index, unit) in text.encode_utf16().enumerate() {
                    self.memory.w16(destination + index as u32 * 2, unit);
                }
                let length = text.encode_utf16().count() as u32;
                self.memory.w16(destination + length * 2, 0);
                self.set_result(destination);
            }
            4 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                self.copy_ucs2(destination, source, count);
                self.set_result(destination);
            }
            5 | 6 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                for offset in 0..count {
                    let byte = self.memory.r8(source + offset);
                    self.memory.w16(destination + offset * 2, byte as u16);
                    if index == 5 && byte == 0 {
                        for remainder in offset + 1..count {
                            self.memory.w16(destination + remainder * 2, 0);
                        }
                        break;
                    }
                }
                self.set_result(destination);
            }
            7 => {
                let source = self.register(0);
                let character = self.register(1) as u16;
                let mut result = 0;
                for offset in 0..0x8000 {
                    let value = self.memory.r16(source + offset * 2);
                    if value == character {
                        result = source + offset * 2;
                        break;
                    }
                    if value == 0 {
                        break;
                    }
                }
                self.set_result(result);
            }
            8..=10 => {
                let left = self.register(0);
                let right = self.register(1);
                let limit = if index == 10 {
                    self.register(2).min(0x8000)
                } else {
                    0x8000
                };
                let mut result = 0i32;
                for offset in 0..limit {
                    let mut a = self.memory.r16(left + offset * 2);
                    let mut b = self.memory.r16(right + offset * 2);
                    if index == 9 {
                        a = ascii_uppercase(a);
                        b = ascii_uppercase(b);
                    }
                    result = a as i32 - b as i32;
                    if result != 0 || a == 0 || b == 0 {
                        break;
                    }
                }
                self.set_result(result as u32);
            }
            _ => self.set_result(0),
        }
    }

    fn ucs2_len(&mut self, address: u32, limit: u32) -> u32 {
        for offset in 0..limit {
            if self.memory.r16(address + offset * 2) == 0 {
                return offset;
            }
        }
        limit
    }

    fn copy_ucs2(&mut self, destination: u32, source: u32, count: u32) {
        for offset in 0..count {
            let value = self.memory.r16(source + offset * 2);
            self.memory.w16(destination + offset * 2, value);
            if value == 0 {
                for remainder in offset + 1..count {
                    self.memory.w16(destination + remainder * 2, 0);
                }
                break;
            }
        }
    }
}
