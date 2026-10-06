//! Screen stack and DreamFactory engine services (groups 14 and 11).

use armv4t_emu::Memory;

use super::super::{
    NicaiMachine, DREAM_FACTORY_MEMORY_BLOCK_SLOT, DREAM_FACTORY_PACKAGE_SLOT, MEMORY_BLOCK_PTR,
    METHOD_KIND_GAMEOLD, SCREEN_IS_IN_QUIT,
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
            // F_4 map-buffer plumbing: the inits create the engine buffers
            // and the getters hand them out on demand.  Returning zero left
            // callers with a NULL buffer whose next method load jumped
            // through a bx thunk to 0x00000000.
            1 => {
                self.ensure_df_render_buffer();
                self.set_result(0);
            }
            2 => {
                self.ensure_df_vscroll_buffer();
                self.set_result(0);
            }
            11 => {
                let buffer = self.ensure_df_vscroll_buffer();
                self.set_result(buffer);
            }
            12 => {
                let buffer = self.ensure_df_render_buffer();
                self.set_result(buffer);
            }
            // F_4 exposes the same initDF* builders as the gameold method
            // table (F_0 slots 0x12c / 0x13c / 0x1b4).  The service path
            // used to return zero without building the object, so callers
            // stored a NULL handle and crashed on the first method call
            // through their bx thunk.  Route to the shared implementations.
            3 | 7 | 9 => {
                let slot = match index {
                    3 => 0x012c, // initDFPictureLibrary
                    7 => 0x013c, // initDFWindows
                    _ => 0x01b4, // initDFActor
                };
                let stub = Self::method_stub_address(METHOD_KIND_GAMEOLD, slot) & !1;
                self.handle_method_stub(stub);
            }
            _ => self.set_result(0),
        }
    }

    /// Create-on-demand: the DF map render buffer the F_4 getter returns.
    /// Sized for a full 240x400 RGB565 frame with headroom; zero-filled so
    /// guest field reads see a cleared structure.
    fn ensure_df_render_buffer(&mut self) -> u32 {
        if self.df_render_buffer == 0 {
            let buffer = self.allocate(0x30000);
            if buffer != 0 {
                let zeros = vec![0u8; 0x30000];
                self.memory.write_bytes(buffer, &zeros);
            }
            self.df_render_buffer = buffer;
        }
        self.df_render_buffer
    }

    /// Create-on-demand: the scene scroll buffer (smaller working set).
    fn ensure_df_vscroll_buffer(&mut self) -> u32 {
        if self.df_vscroll_buffer == 0 {
            let buffer = self.allocate(0x10000);
            if buffer != 0 {
                let zeros = vec![0u8; 0x10000];
                self.memory.write_bytes(buffer, &zeros);
            }
            self.df_vscroll_buffer = buffer;
        }
        self.df_vscroll_buffer
    }
}
