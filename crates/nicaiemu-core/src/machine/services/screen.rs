//! Screen stack and DreamFactory engine services (groups 14 and 11).

use armv4t_emu::Memory;

use super::super::{
    NicaiMachine, DREAM_FACTORY_MEMORY_BLOCK_SLOT, DREAM_FACTORY_PACKAGE_SLOT, MEMORY_BLOCK_PTR,
    SCREEN_IS_IN_QUIT,
};

impl NicaiMachine {
    pub(crate) fn handle_screen_service(&mut self, index: u32) {
        match index {
            0 | 2 | 3 => {
                let screen = self.register(0);
                if screen != 0 {
                    if let Some(current) = self.screen_stack.last_mut() {
                        *current = screen;
                    } else {
                        self.screen_stack.push(screen);
                    }
                    self.pending_screen = screen;
                    self.memory.w32(SCREEN_IS_IN_QUIT, 0);
                }
                self.set_result(SCREEN_IS_IN_QUIT);
            }
            1 | 7 | 8 => {
                let requested = self.register(0);
                self.resource_load_screen = if requested != 0 {
                    requested
                } else {
                    self.pending_screen
                };
                self.resource_load_pending = true;
                self.set_result(0);
            }
            4 | 5 => {
                let screen = self.register(0);
                if screen != 0 {
                    self.screen_stack.push(screen);
                    self.pending_screen = screen;
                    self.memory.w32(SCREEN_IS_IN_QUIT, 0);
                }
                self.set_result(0);
            }
            6 => {
                let screen = self.register(0);
                let removed = self
                    .screen_stack
                    .iter()
                    .rposition(|candidate| *candidate == screen)
                    .map(|position| {
                        self.screen_stack.remove(position);
                        true
                    })
                    .unwrap_or(false);
                if removed && (self.active_screen == screen || self.pending_screen == screen) {
                    self.pending_screen = self.screen_stack.last().copied().unwrap_or(0);
                    self.active_screen = self.pending_screen;
                    self.screen_initialized = false;
                }
                self.set_result(u32::from(removed));
            }
            9 => self.set_result(u32::from(
                self.screen_stack.last().copied() == Some(self.register(0)),
            )),
            10 => self.set_result(u32::from(
                self.screen_stack.first().copied() == Some(self.register(0)),
            )),
            _ => self.set_result(0),
        }
    }

    pub(crate) fn handle_df_engine_service(&mut self, index: u32) {
        match index {
            8 => {
                self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, 0);
                self.memory
                    .w32(DREAM_FACTORY_MEMORY_BLOCK_SLOT, MEMORY_BLOCK_PTR);
                self.set_result(0);
            }
            10 => {
                let package = self.register(0);
                let capacity = self.register(1);
                self.initialize_data_package(package, capacity);
            }
            _ => self.set_result(0),
        }
    }
}
