//! File and stdio services (groups 5 and 6) with C-string helpers.

use armv4t_emu::Memory;

use super::super::NicaiMachine;

impl NicaiMachine {
    pub(crate) fn handle_file_service(&mut self, index: u32) {
        match index {
            3 | 17 => self.set_result(1),
            13 | 14 => self.set_result(u32::MAX),
            16 => self.set_result(64 * 1024 * 1024),
            _ => self.set_result(0),
        }
    }

    pub(crate) fn handle_stdio_service(&mut self, index: u32) {
        match index {
            0 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                let bytes: Vec<u8> = (0..count)
                    .map(|offset| self.memory.r8(source + offset))
                    .collect();
                self.memory.write_bytes(destination, &bytes);
                self.set_result(destination);
            }
            1 => {
                let length = self.read_c_bytes(self.register(0), 0x1_0000).len() as u32;
                self.set_result(length);
            }
            2 => {
                let destination = self.register(0);
                let value = self.register(1) as u8;
                let count = self.register(2);
                for offset in 0..count {
                    self.memory.w8(destination + offset, value);
                }
                self.set_result(destination);
            }
            3 => {
                let destination = self.register(0);
                let format = self.read_c_bytes(self.register(1), 4096);
                let output = self.format_c_string(&format);
                for (offset, byte) in output.iter().enumerate() {
                    self.memory
                        .w8(destination.wrapping_add(offset as u32), *byte);
                }
                self.memory
                    .w8(destination.wrapping_add(output.len() as u32), 0);
                self.set_result(destination);
            }
            4 => self.set_result(0),
            5 => self.set_result(self.instruction_count as u32),
            7 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                for offset in 0..count {
                    let value = self.memory.r8(source + offset);
                    self.memory.w8(destination + offset, value);
                    if value == 0 {
                        for remainder in offset + 1..count {
                            self.memory.w8(destination + remainder, 0);
                        }
                        break;
                    }
                }
                self.set_result(destination);
            }
            8 | 9 => {
                let destination = self.register(0);
                let source = self.register(1);
                let destination_offset = if index == 9 {
                    self.read_c_bytes(destination, 0x1_0000).len() as u32
                } else {
                    0
                };
                let bytes = self.read_c_bytes(source, 0x1_0000);
                self.memory
                    .write_bytes(destination + destination_offset, &bytes);
                self.memory
                    .w8(destination + destination_offset + bytes.len() as u32, 0);
                self.set_result(destination);
            }
            10 | 12 => {
                let text = self.read_c_string(self.register(0), 256);
                let value = text.trim().parse::<i32>().unwrap_or(0);
                self.set_result(value as u32);
            }
            11 => {
                let destination = self.register(0);
                let source = self.register(1);
                let count = self.register(2);
                let bytes: Vec<u8> = (0..count)
                    .map(|offset| self.memory.r8(source + offset))
                    .collect();
                self.memory.write_bytes(destination, &bytes);
                self.set_result(destination);
            }
            13 => {
                let left = f32::from_bits(self.register(0));
                let right = f32::from_bits(self.register(1));
                self.set_result(left.powf(right).to_bits());
            }
            14..=16 => {
                let limit = if index == 14 {
                    0x1_0000
                } else {
                    self.register(2)
                };
                let stop_at_nul = index != 15;
                let result = self.compare_c_bytes(
                    self.register(0),
                    self.register(1),
                    limit,
                    stop_at_nul,
                    false,
                );
                self.set_result(result as u32);
            }
            20 | 21 => {
                let limit = if index == 20 {
                    0x1_0000
                } else {
                    self.register(2)
                };
                let result =
                    self.compare_c_bytes(self.register(0), self.register(1), limit, true, true);
                self.set_result(result as u32);
            }
            _ => self.set_result(0),
        }
    }

    pub(crate) fn compare_c_bytes(
        &mut self,
        left: u32,
        right: u32,
        limit: u32,
        stop_at_nul: bool,
        ignore_ascii_case: bool,
    ) -> i32 {
        for offset in 0..limit {
            let mut a = self.memory.r8(left + offset);
            let mut b = self.memory.r8(right + offset);
            if ignore_ascii_case {
                a = a.to_ascii_lowercase();
                b = b.to_ascii_lowercase();
            }
            if a != b {
                return a as i32 - b as i32;
            }
            if stop_at_nul && (a == 0 || b == 0) {
                break;
            }
        }
        0
    }

    pub(crate) fn format_c_string(&mut self, format: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        let mut position = 0usize;
        let mut argument_index = 0u32;
        while position < format.len() {
            if format[position] != b'%' {
                output.push(format[position]);
                position += 1;
                continue;
            }
            position += 1;
            if format.get(position) == Some(&b'%') {
                output.push(b'%');
                position += 1;
                continue;
            }
            let zero_pad = format.get(position) == Some(&b'0');
            if zero_pad {
                position += 1;
            }
            let mut width = 0usize;
            while let Some(digit @ b'0'..=b'9') = format.get(position) {
                width = width * 10 + (digit - b'0') as usize;
                position += 1;
            }
            while matches!(format.get(position), Some(b'l' | b'h')) {
                position += 1;
            }
            let Some(specifier) = format.get(position).copied() else {
                break;
            };
            position += 1;
            let argument = match argument_index {
                0 => self.register(2),
                1 => self.register(3),
                index => self.memory.r32(
                    self.register(armv4t_emu::reg::SP)
                        .wrapping_add((index - 2) * 4),
                ),
            };
            argument_index += 1;
            let mut formatted = match specifier {
                b's' => self.read_c_bytes(argument, 4096),
                b'c' => vec![argument as u8],
                b'd' | b'i' => (argument as i32).to_string().into_bytes(),
                b'u' => argument.to_string().into_bytes(),
                b'x' => format!("{argument:x}").into_bytes(),
                b'X' => format!("{argument:X}").into_bytes(),
                _ => vec![b'%', specifier],
            };
            if formatted.len() < width {
                let padding = vec![if zero_pad { b'0' } else { b' ' }; width - formatted.len()];
                output.extend_from_slice(&padding);
            }
            output.append(&mut formatted);
        }
        output
    }

    pub(crate) fn read_c_string(&mut self, address: u32, limit: u32) -> String {
        String::from_utf8_lossy(&self.read_c_bytes(address, limit)).into_owned()
    }

    pub(crate) fn read_c_bytes(&mut self, address: u32, limit: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        for offset in 0..limit {
            let byte = self.memory.r8(address.wrapping_add(offset));
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        bytes
    }
}
