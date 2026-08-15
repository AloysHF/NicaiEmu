//! Memory manager services (groups 2 and 27).

use armv4t_emu::Memory;

use super::super::{NicaiMachine, MEMORY_BLOCK_POOL, MEMORY_BLOCK_PTR, MEMORY_BLOCK_SERVICE};

impl NicaiMachine {
    pub(crate) fn handle_memory_service(&mut self, index: u32) {
        match index {
            2 => {
                let destination = self.register(0);
                let size = self.register(1).max(2);
                let pointer = self.allocate(size);
                self.memory.w32(destination, pointer);
                self.set_result(1);
            }
            3 => {
                let destination = self.register(0);
                if destination != 0 {
                    let pointer = self.memory.r32(destination);
                    self.deallocate(pointer);
                    self.memory.w32(destination, 0);
                }
                self.set_result(0);
            }
            5 => {
                let block = self.register(0);
                let size = self.register(1);
                self.initialize_memory_block(block, size);
                self.set_result(block);
            }
            6 => {
                let block = self.register(0);
                let size = self.register(1);
                let pointer = self.allocate_from_memory_block(block, size);
                self.set_result(pointer);
            }
            7 => {
                let block = self.register(0);
                self.memory.w32(block + 4, 0);
                self.set_result(block);
            }
            8 => self.set_result(MEMORY_BLOCK_PTR),
            9 => {
                self.initialize_memory_block(MEMORY_BLOCK_PTR, 0x40_0000);
                self.set_result(MEMORY_BLOCK_PTR);
            }
            10 | 11 => {
                if self.memory.r32(MEMORY_BLOCK_PTR) == 0 {
                    self.initialize_memory_block(MEMORY_BLOCK_PTR, 0x40_0000);
                } else {
                    self.memory.w32(MEMORY_BLOCK_PTR + 4, 0);
                }
                self.set_result(MEMORY_BLOCK_PTR);
            }
            12 => {
                let size = self.register(0);
                if self.memory.r32(MEMORY_BLOCK_PTR) == 0 {
                    self.initialize_memory_block(MEMORY_BLOCK_PTR, 0x40_0000);
                }
                let pointer = self.allocate_from_memory_block(MEMORY_BLOCK_PTR, size);
                self.set_result(pointer);
            }
            13 => {
                let size = self.register(0).max(1);
                let pointer = self.allocate(size);
                self.set_result(pointer);
            }
            14 => {
                self.deallocate(self.register(0));
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    pub(crate) fn initialize_memory_block(&mut self, block: u32, size: u32) {
        let base = if block == MEMORY_BLOCK_PTR {
            MEMORY_BLOCK_POOL
        } else {
            self.allocate(size)
        };
        self.memory.w32(block, base);
        self.memory.w32(block + 4, 0);
        self.memory.w32(block + 8, size);
        self.memory.w32(block + 12, MEMORY_BLOCK_SERVICE);
        self.memory.w32(block + 16, MEMORY_BLOCK_SERVICE + 4);
        self.memory.w32(block + 20, MEMORY_BLOCK_SERVICE + 8);
    }

    pub(crate) fn handle_memory_block_service(&mut self, index: u32) {
        let block = self.register(0);
        match index {
            0 => {
                let requested = self.register(1);
                let pointer = self.allocate_from_memory_block(block, requested);
                self.set_result(pointer);
            }
            1 => {
                self.memory.w32(block + 4, 0);
                self.set_result(block);
            }
            2 => {
                let base = self.memory.r32(block);
                if block != MEMORY_BLOCK_PTR {
                    self.deallocate(base);
                }
                self.memory.w32(block, 0);
                self.memory.w32(block + 4, 0);
                self.memory.w32(block + 8, 0);
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    fn allocate_from_memory_block(&mut self, block: u32, requested: u32) -> u32 {
        let aligned = requested.saturating_add(3) & !3;
        let base = self.memory.r32(block);
        let offset = self.memory.r32(block + 4);
        let size = self.memory.r32(block + 8);
        if base != 0 && offset.saturating_add(aligned) <= size {
            self.memory.w32(block + 4, offset + aligned);
            base + offset
        } else {
            0
        }
    }
}
