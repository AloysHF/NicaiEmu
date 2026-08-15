//! File and stdio services (groups 5 and 6) with C-string helpers.

use armv4t_emu::Memory;
use encoding_rs::GBK;

use super::super::{variadic_argument_location, NicaiMachine, VariadicArgument};

impl NicaiMachine {
    pub(crate) fn handle_file_service(&mut self, index: u32) {
        if super::super::service_trace_enabled(5, index) {
            let path = match index {
                0 | 2 | 3 | 9..=12 => self.read_file_path(self.register(1)),
                _ => String::new(),
            };
            let extra = match index {
                0 => self.read_c_string(self.register(2), 16),
                10 => self.read_file_path(self.register(2)),
                _ => String::new(),
            };
            eprintln!(
                "file service index={index} r0={:08X} r1={:08X} r2={:08X} path={path:?} extra={extra:?}",
                self.register(0),
                self.register(1),
                self.register(2),
            );
        }
        match index {
            0 => {
                let path = self.read_file_path(self.register(1));
                let mode = self.read_c_string(self.register(2), 16);
                let result = self.virtual_fs.open(&path, &mode, self.register(0));
                self.set_result(result as u32);
            }
            1 => {
                let result = self.virtual_fs.close(self.register(0));
                self.set_result(result as u32);
            }
            2 => {
                let path = self.read_file_path(self.register(1));
                self.set_result(u32::from(self.virtual_fs.file_exists(&path)));
            }
            3 => {
                let path = self.read_file_path(self.register(1));
                self.set_result(u32::from(self.virtual_fs.directory_exists(&path)));
            }
            4 => {
                let destination = self.register(0);
                let size = self.register(1) as usize;
                let handle = self.register(2);
                let result = if size > 16 * 1024 * 1024 {
                    None
                } else {
                    self.virtual_fs.read(handle, size)
                };
                if let Some(data) = result {
                    self.memory.write_bytes(destination, &data);
                    self.set_result(data.len() as u32);
                } else {
                    self.set_result(u32::MAX);
                }
            }
            5 => {
                let source = self.register(0);
                let size = self.register(1) as usize;
                let handle = self.register(2);
                if size > 16 * 1024 * 1024 {
                    self.set_result(u32::MAX);
                } else {
                    let data: Vec<u8> = (0..size)
                        .map(|offset| self.memory.r8(source + offset as u32))
                        .collect();
                    let result = self.virtual_fs.write(handle, &data);
                    self.set_result(result.map_or(u32::MAX, |written| written as u32));
                }
            }
            6 => {
                let result = self.virtual_fs.seek(
                    self.register(0),
                    self.register(1) as i32,
                    self.register(2),
                );
                self.set_result(result.map_or(u32::MAX, |position| position as u32));
            }
            7 => {
                let result = self.virtual_fs.tell(self.register(0));
                self.set_result(result.map_or(u32::MAX, |position| position as u32));
            }
            8 => {
                let result = self.virtual_fs.size(self.register(0));
                self.set_result(result.map_or(u32::MAX, |size| size as u32));
            }
            9 => {
                let path = self.read_file_path(self.register(1));
                let removed = self.virtual_fs.remove_file(&path);
                self.set_result(u32::from(removed));
            }
            10 => {
                let old_path = self.read_file_path(self.register(1));
                let new_path = self.read_file_path(self.register(2));
                let renamed = self.virtual_fs.rename(&old_path, &new_path);
                self.set_result(u32::from(renamed));
            }
            11 => {
                let path = self.read_file_path(self.register(1));
                let result = self.virtual_fs.create_directory(&path);
                self.set_result(if result { 0 } else { u32::MAX });
            }
            12 => self.set_result(0),
            13 | 14 => self.set_result(u32::MAX),
            16 => self.set_result(64 * 1024 * 1024),
            17 => self.set_result(1),
            _ => self.set_result(0),
        }
    }

    fn read_file_path(&mut self, address: u32) -> String {
        if address == 0 {
            return String::new();
        }
        if self.memory.r8(address + 1) != 0 {
            return self.read_gbk_string(address, 512);
        }
        let mut units = Vec::new();
        for offset in 0..256u32 {
            let unit = self.memory.r16(address + offset * 2);
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        String::from_utf16_lossy(&units)
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
        self.format_c_string_from(format, 2)
    }

    pub(crate) fn format_c_string_from(&mut self, format: &[u8], first_register: u32) -> Vec<u8> {
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
            let argument = match variadic_argument_location(first_register, argument_index) {
                VariadicArgument::Register(register) => self.register(register),
                VariadicArgument::Stack(offset) => self
                    .memory
                    .r32(self.register(armv4t_emu::reg::SP).wrapping_add(offset)),
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

    pub(crate) fn read_gbk_string(&mut self, address: u32, limit: u32) -> String {
        GBK.decode(&self.read_c_bytes(address, limit))
            .0
            .into_owned()
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
