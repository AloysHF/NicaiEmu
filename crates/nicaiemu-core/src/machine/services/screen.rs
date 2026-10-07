//! Screen stack and DreamFactory engine services (groups 14 and 11).

use armv4t_emu::Memory;

use super::super::{
    NicaiMachine, DREAM_FACTORY_MEMORY_BLOCK_SLOT, DREAM_FACTORY_PACKAGE_SLOT, MEMORY_BLOCK_PTR,
    METHOD_KIND_GAMEOLD, SCREEN_IS_IN_QUIT,
};

fn clip_panel_rect(memory: &mut impl Memory, rect: u32) {
    let mut x = memory.r16(rect) as i16 as i32;
    let mut y = memory.r16(rect.wrapping_add(2)) as i16 as i32;
    let mut width = memory.r16(rect.wrapping_add(4)) as i16 as i32;
    let mut height = memory.r16(rect.wrapping_add(6)) as i16 as i32;

    if x + width < 0 || x > 240 || y + height < 0 || y > 400 {
        (x, y, width, height) = (0, 0, 0, 0);
    }
    if x < 0 {
        width += x;
        x = 0;
    }
    if x + width > 240 {
        width = 240 - x;
    }
    if y + height > 400 {
        height = 400 - y;
    }

    for (offset, value) in [x, y, width, height].into_iter().enumerate() {
        memory.w16(rect.wrapping_add((offset * 2) as u32), value as i16 as u16);
    }
}

fn add_panel_dirty_rect(memory: &mut impl Memory, panel: u32, rect: u32) {
    let count = memory.r32(panel.wrapping_add(4));
    let capacity = memory.r32(panel.wrapping_add(8));
    let table = memory.r32(panel.wrapping_add(12));
    if count >= capacity || table == 0 {
        return;
    }
    let entry = memory.r32(table.wrapping_add(count.wrapping_mul(4)));
    if entry == 0 {
        return;
    }
    let mut bytes = [0; 8];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = memory.r8(rect.wrapping_add(index as u32));
    }
    for (index, byte) in bytes.into_iter().enumerate() {
        memory.w8(entry.wrapping_add(index as u32), byte);
    }
    memory.w32(panel.wrapping_add(4), count.wrapping_add(1));
}

fn invalidate_panel(memory: &mut impl Memory, panel: u32, kind: i16, rect: u32) {
    if panel == 0 || rect == 0 {
        return;
    }
    clip_panel_rect(memory, rect);
    if kind == 2 {
        let mut pending = vec![panel];
        let mut visited = std::collections::BTreeSet::new();
        while let Some(current) = pending.pop() {
            if current == 0 || !visited.insert(current) || visited.len() > 4096 {
                continue;
            }
            add_panel_dirty_rect(memory, current, rect);
            let sibling = memory.r32(current.wrapping_add(32));
            let child = memory.r32(current.wrapping_add(36));
            if sibling != 0 {
                pending.push(sibling);
            }
            if child != 0 {
                pending.push(child);
            }
        }
    } else if kind == 4 {
        add_panel_dirty_rect(memory, panel, rect);
    }
}

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
            14 => {
                let panel = self.register(0);
                let kind = self.register(1) as i16;
                let rect = self.register(2);
                invalidate_panel(&mut self.memory, panel, kind, rect);
                self.set_result(0);
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

#[cfg(test)]
mod tests {
    use super::invalidate_panel;
    use crate::machine::memory::MachineMemory;
    use armv4t_emu::Memory;

    #[test]
    fn panel_invalidation_clips_and_queues_dirty_rectangle() {
        let mut memory = MachineMemory::new(true);
        memory.map(0x1000, 0x1000, false);
        memory.w32(0x1004, 0);
        memory.w32(0x1008, 1);
        memory.w32(0x100c, 0x1100);
        memory.w32(0x1100, 0x1200);
        memory.w16(0x1300, (-8_i16) as u16);
        memory.w16(0x1302, 10);
        memory.w16(0x1304, 20);
        memory.w16(0x1306, 30);

        invalidate_panel(&mut memory, 0x1000, 4, 0x1300);

        assert_eq!(memory.r32(0x1004), 1);
        assert_eq!(memory.r16(0x1300), 0);
        assert_eq!(memory.r16(0x1304), 12);
        assert_eq!(memory.r16(0x1200), 0);
        assert_eq!(memory.r16(0x1204), 12);
        assert_eq!(memory.r16(0x1206), 30);
    }
}
