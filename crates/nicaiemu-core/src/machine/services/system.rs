//! System and root manager services (groups 0 and 1).

use armv4t_emu::Memory;

use super::super::{
    manager_initializer_count, NicaiMachine, DL_IMAGE_MANAGER, DL_LOAD_MANAGER, DL_PAY_MANAGER,
    DL_RESOURCE_MANAGER, MANAGER_BASE, SERVICE_BASE, TABLE_STRIDE, VIDEO_MANAGER,
};

impl NicaiMachine {
    pub(crate) fn handle_system_service(&mut self, index: u32) {
        match index {
            3 => self.set_result(3),
            15 => self.set_result(1),
            17 => {
                let destination = self.register(0);
                self.memory.w16(destination, b'.' as u16);
                self.memory.w16(destination + 2, b'/' as u16);
                self.memory.w16(destination + 4, 0);
                self.set_result(4);
            }
            22 => self.set_result(255),
            23 => self.set_result(0),
            25 => {
                let destination = self.register(0);
                let capacity = self.register(1) as usize;
                let value = b"cbe_emu\0";
                let length = value.len().min(capacity);
                self.memory.write_bytes(destination, &value[..length]);
                self.set_result(0);
            }
            30 => self.set_result(46),
            33 => self.set_result(1002),
            37 => self.set_result(1),
            47 => self.set_result(self.instruction_count as u32),
            64 => self.set_result(0x0e),
            65 => self.set_result(5),
            80 | 89 => self.set_result(1),
            90 => self.set_result(0),
            106 => {
                let destination = self.register(1);
                if destination != 0 && self.register(2) != 0 {
                    self.memory.w8(destination, 0);
                }
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    pub(crate) fn handle_root_service(&mut self, index: u32) {
        match index {
            34 => {
                let destination = self.register(0);
                if destination != 0 {
                    self.populate_table(destination, SERVICE_BASE, 52);
                }
                self.set_result(0);
                return;
            }
            39 => {
                let destination = self.register(0);
                if destination != 0 {
                    self.populate_table(destination, SERVICE_BASE + TABLE_STRIDE * 22, 11);
                }
                self.set_result(destination);
                return;
            }
            40 => {
                self.set_result(DL_LOAD_MANAGER);
                return;
            }
            41 => {
                self.set_result(DL_RESOURCE_MANAGER);
                return;
            }
            42 => {
                let destination = self.register(0);
                if destination != 0 {
                    self.populate_table(destination, SERVICE_BASE + TABLE_STRIDE * 25, 20);
                }
                self.set_result(destination);
                return;
            }
            43 => {
                self.set_result(DL_IMAGE_MANAGER);
                return;
            }
            44 => {
                let destination = self.register(0);
                if destination != 0 {
                    self.populate_table(destination, SERVICE_BASE + TABLE_STRIDE * 26, 12);
                }
                self.set_result(destination);
                return;
            }
            49 => {
                let destination = self.register(0);
                if destination != 0 {
                    self.populate_table(destination, SERVICE_BASE + TABLE_STRIDE * 23, 38);
                }
                self.set_result(destination);
                return;
            }
            50 => {
                self.set_result(VIDEO_MANAGER);
                return;
            }
            51 => {
                self.set_result(DL_PAY_MANAGER);
                return;
            }
            _ => {}
        }
        let table_group = match index {
            0 | 1 => Some(5),
            2 | 3 => Some(4),
            4 | 5 => Some(7),
            6 | 7 => Some(8),
            8 | 9 => Some(2),
            10 | 11 => Some(12),
            12 | 13 => Some(14),
            14 | 15 => Some(9),
            16 | 17 => Some(13),
            18 | 19 => Some(1),
            20 | 21 => Some(15),
            22 | 23 => Some(16),
            24 | 25 => Some(10),
            26 | 27 => Some(11),
            28 | 29 => Some(17),
            30 | 31 => Some(18),
            32 | 33 => Some(3),
            35 | 36 => Some(19),
            37 | 38 => Some(6),
            45 | 46 => Some(20),
            _ => None,
        };
        if let Some(table_group) = table_group {
            let table = MANAGER_BASE + TABLE_STRIDE * (table_group + 1);
            let service = SERVICE_BASE + TABLE_STRIDE * table_group;
            let is_initializer = matches!(
                index,
                0 | 2
                    | 4
                    | 6
                    | 8
                    | 10
                    | 12
                    | 14
                    | 16
                    | 18
                    | 20
                    | 22
                    | 24
                    | 26
                    | 28
                    | 30
                    | 32
                    | 35
                    | 37
                    | 45
            );
            if is_initializer {
                let destination = self.register(0);
                if destination != 0 {
                    if index == 26 {
                        self.memory.w32(destination + 8 * 4, service + 8 * 4);
                        self.memory.w32(destination + 10 * 4, service + 10 * 4);
                    } else if index == 28 {
                        self.memory.w32(destination + 60 * 4, service + 60 * 4);
                    } else {
                        let count = manager_initializer_count(index).unwrap_or(0);
                        self.populate_table(destination, service, count);
                    }
                }
                self.set_result(destination);
            } else {
                self.set_result(table);
            }
        } else {
            self.set_result(0);
        }
    }
}
