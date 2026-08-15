//! CPU execution loop, interworking branches, and service dispatch entry.

use anyhow::{bail, Result};
use armv4t_emu::{reg, Memory, Mode};

use super::{
    arm_blx_immediate_target, fixed_manager_specs, service_trace_enabled, thumb_add_pc_target,
    NicaiMachine, APP_STORE_MANAGER, EXIT_ADDRESS, FIXED_GAMEOLD_OBJECT_SERVICE,
    FIXED_GAMEOLD_REGION_SERVICE, FIXED_MANAGER_INIT, LOG_NOOP_SERVICE, MEMORY_BLOCK_SERVICE,
    NATIVE_DISPATCH_SERVICE, NATIVE_SYSTEM_TIME_SERVICE, SERVICE_BASE, SERVICE_SIZE, TABLE_STRIDE,
};

impl NicaiMachine {
    pub(super) fn run_until_return(&mut self, instruction_limit: u64) -> Result<()> {
        for _ in 0..instruction_limit {
            let mut pc = self.cpu.reg_get(Mode::User, reg::PC);
            let aligned_pc = if self.cpu.thumb_mode() {
                pc & !1
            } else {
                pc & !3
            };
            if aligned_pc != pc {
                pc = aligned_pc;
                self.cpu.reg_set(Mode::User, reg::PC, pc);
            }
            self.last_pc = pc;
            if self.recent_pcs.len() == 32 {
                self.recent_pcs.pop_front();
            }
            self.recent_pcs.push_back(pc);
            if pc & !1 == EXIT_ADDRESS {
                return Ok(());
            }
            let terminal_self_branch = if self.cpu.thumb_mode() {
                self.memory.r16(pc) == 0xe7fe
            } else {
                self.memory.r32(pc) == 0xeaff_fffe
            };
            if terminal_self_branch {
                self.state = super::MachineState::Halted;
                return Ok(());
            }
            if (SERVICE_BASE..SERVICE_BASE + SERVICE_SIZE).contains(&pc) {
                self.handle_service(pc)?;
            } else if self.cpu.thumb_mode() && self.memory.r16(pc) == 0xdfab {
                self.handle_semihosting(pc)?;
            } else if self.handle_thumb_add_pc(pc) || self.handle_interworking_branch(pc) {
            } else if self
                .memory
                .region(pc, if self.cpu.thumb_mode() { 2 } else { 4 })
                .is_none()
            {
                self.state = super::MachineState::Faulted;
                bail!("instruction fetch from unmapped address 0x{pc:08X}");
            } else if !self.cpu.step(&mut self.memory) {
                self.state = super::MachineState::Faulted;
                bail!("unsupported ARM instruction at 0x{pc:08X}");
            }
            self.instruction_count += 1;
        }
        self.state = super::MachineState::Faulted;
        bail!(
            "CBE execution exceeded {instruction_limit} instructions at 0x{:08X}",
            self.last_pc
        )
    }

    fn handle_thumb_blx(&mut self, pc: u32) -> bool {
        self.handle_thumb_blx_register(pc) || self.handle_thumb_blx_immediate(pc)
    }

    fn handle_thumb_add_pc(&mut self, pc: u32) -> bool {
        if !self.cpu.thumb_mode() {
            return false;
        }
        let instruction = self.memory.r16(pc);
        let source = ((instruction >> 3) & 0x0f) as u8;
        let Some(target) = thumb_add_pc_target(pc, instruction, self.register(source)) else {
            return false;
        };
        self.cpu.reg_set(Mode::User, reg::PC, target);
        true
    }

    fn handle_interworking_branch(&mut self, pc: u32) -> bool {
        if self.cpu.thumb_mode() {
            self.handle_thumb_blx(pc)
        } else {
            self.handle_arm_blx_immediate(pc)
        }
    }

    fn handle_arm_blx_immediate(&mut self, pc: u32) -> bool {
        let instruction = self.memory.r32(pc);
        let Some(target) = arm_blx_immediate_target(pc, instruction) else {
            return false;
        };
        self.cpu.reg_set(Mode::User, reg::LR, pc.wrapping_add(4));
        self.cpu.reg_set(Mode::User, reg::PC, target & !1);
        let cpsr = self.register(reg::CPSR) | (1 << 5);
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
        true
    }

    fn handle_thumb_blx_register(&mut self, pc: u32) -> bool {
        let instruction = self.memory.r16(pc);
        if instruction & 0xff87 != 0x4780 {
            return false;
        }
        let source = ((instruction >> 3) & 0x0f) as u8;
        let target = self.register(source);
        self.cpu.reg_set(Mode::User, reg::LR, pc + 3);
        self.cpu.reg_set(Mode::User, reg::PC, target & !1);
        let mut cpsr = self.register(reg::CPSR);
        if target & 1 != 0 {
            cpsr |= 1 << 5
        } else {
            cpsr &= !(1 << 5)
        }
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
        true
    }

    fn handle_thumb_blx_immediate(&mut self, pc: u32) -> bool {
        let prefix = self.memory.r16(pc);
        let suffix = self.memory.r16(pc + 2);
        if prefix & 0xf800 != 0xf000 || suffix & 0xf800 != 0xe800 {
            return false;
        }
        let raw_offset = (((prefix & 0x07ff) as u32) << 12) | (((suffix & 0x07ff) as u32) << 1);
        let offset = ((raw_offset << 9) as i32 >> 9) as u32;
        let target = pc.wrapping_add(4).wrapping_add(offset) & !3;
        self.cpu.reg_set(Mode::User, reg::LR, pc + 5);
        self.cpu.reg_set(Mode::User, reg::PC, target);
        let cpsr = self.register(reg::CPSR) & !(1 << 5);
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
        true
    }

    fn handle_semihosting(&mut self, pc: u32) -> Result<()> {
        let reason = self.register(0);
        let argument = self.register(1);
        match reason {
            3 => {
                if std::env::var("CBE_TRACE").is_ok() {
                    eprint!("{}", self.memory.r8(argument) as char);
                }
            }
            4 => {
                let mut bytes = Vec::new();
                for offset in 0..4096u32 {
                    let byte = self.memory.r8(argument.wrapping_add(offset));
                    if byte == 0 {
                        break;
                    }
                    bytes.push(byte);
                }
                let message = String::from_utf8_lossy(&bytes);
                if std::env::var("CBE_TRACE").is_ok() {
                    eprint!("{message}");
                }
            }
            _ => bail!("unhandled semihosting call reason={reason} at 0x{pc:08X}"),
        }
        self.cpu.reg_set(Mode::User, reg::PC, pc + 2);
        Ok(())
    }

    fn handle_service(&mut self, address: u32) -> Result<()> {
        if address == LOG_NOOP_SERVICE {
            self.return_from_service();
            return Ok(());
        }
        if (FIXED_MANAGER_INIT..FIXED_MANAGER_INIT + fixed_manager_specs().len() as u32 * 4)
            .contains(&address)
        {
            let index = ((address - FIXED_MANAGER_INIT) / 4) as usize;
            let (_, group, count) = fixed_manager_specs()[index];
            let destination = self.register(0);
            if destination != 0 {
                self.populate_table(destination, SERVICE_BASE + TABLE_STRIDE * group, count);
            }
            self.set_result(destination);
            self.return_from_service();
            return Ok(());
        }
        if (FIXED_GAMEOLD_OBJECT_SERVICE..FIXED_GAMEOLD_OBJECT_SERVICE + 15 * 4).contains(&address)
        {
            let index = (address - FIXED_GAMEOLD_OBJECT_SERVICE) / 4;
            self.handle_fixed_gameold_object_service(index);
            self.return_from_service();
            return Ok(());
        }
        if (FIXED_GAMEOLD_REGION_SERVICE..FIXED_GAMEOLD_REGION_SERVICE + 8 * 4).contains(&address) {
            let index = (address - FIXED_GAMEOLD_REGION_SERVICE) / 4;
            self.handle_fixed_gameold_region_service(index);
            self.return_from_service();
            return Ok(());
        }
        if address == NATIVE_DISPATCH_SERVICE {
            self.handle_native_dispatch_service();
            self.return_from_service();
            return Ok(());
        }
        if (NATIVE_SYSTEM_TIME_SERVICE..NATIVE_SYSTEM_TIME_SERVICE + 6 * 4).contains(&address) {
            let index = (address - NATIVE_SYSTEM_TIME_SERVICE) / 4;
            let value = match index {
                0 => 2026,
                1 => 8,
                2 => 7,
                3 => 12,
                4 => 0,
                _ => 0,
            };
            self.set_result(value);
            self.return_from_service();
            return Ok(());
        }
        if (MEMORY_BLOCK_SERVICE..MEMORY_BLOCK_SERVICE + 12).contains(&address) {
            let index = (address - MEMORY_BLOCK_SERVICE) / 4;
            *self.service_calls.entry((27, index)).or_default() += 1;
            self.handle_memory_block_service(index);
            self.return_from_service();
            return Ok(());
        }
        let offset = address - SERVICE_BASE;
        let group = offset / TABLE_STRIDE;
        let index = (offset % TABLE_STRIDE) / 4;
        *self.service_calls.entry((group, index)).or_default() += 1;
        let trace_service = service_trace_enabled(group, index);
        if self.recent_services.len() == 16 {
            self.recent_services.pop_front();
        }
        self.recent_services
            .push_back((group, index, self.register(reg::LR), self.register(0)));
        if trace_service {
            eprintln!(
                "service group={group} index={index} r0={:08X} r1={:08X} r2={:08X} r3={:08X} lr={:08X}",
                self.register(0),
                self.register(1),
                self.register(2),
                self.register(3),
                self.register(reg::LR),
            );
        }
        match group {
            0 => self.handle_root_service(index),
            1 => self.handle_system_service(index),
            2 => self.handle_memory_service(index),
            3 => self.handle_game_service(index),
            4 => self.handle_lcd_service(index),
            5 => self.handle_file_service(index),
            6 => self.handle_stdio_service(index),
            7 => self.handle_timer_service(index),
            10 => self.handle_game_util_service(index),
            11 => self.handle_df_engine_service(index),
            13 => self.handle_ucs2_service(index),
            14 => self.handle_screen_service(index),
            16 => self.handle_game_lcd_service(index),
            18 => self.handle_audio_service(index),
            20 => {
                if index == 6 {
                    let descriptor = self.register(0);
                    let destination = self.memory.r32(descriptor);
                    let capacity = self.memory.r16(descriptor + 4) as u32;
                    if destination != 0 {
                        self.populate_table(
                            destination,
                            SERVICE_BASE + TABLE_STRIDE * 28,
                            (capacity / 4).min(40),
                        );
                    }
                    self.set_result(APP_STORE_MANAGER);
                } else {
                    self.set_result(0);
                }
            }
            21 => self.handle_data_package_service(index),
            22 => self.handle_download_service(index),
            23 => self.set_result(0),
            24 => self.handle_payment_service(index),
            25 => self.handle_download_resource_service(index),
            26 => self.handle_download_image_service(index),
            28 => self.set_result(u32::from(index == 30)),
            _ => self.set_result(0),
        }
        if trace_service {
            eprintln!("service result r0={:08X}", self.register(0));
        }
        self.return_from_service();
        Ok(())
    }

    pub(super) fn register(&self, register: armv4t_emu::reg::Reg) -> u32 {
        self.cpu.reg_get(Mode::User, register)
    }

    pub(super) fn set_result(&mut self, value: u32) {
        self.cpu.reg_set(Mode::User, 0, value);
    }

    pub(super) fn return_from_service(&mut self) {
        let target = self.register(reg::LR);
        self.cpu.reg_set(Mode::User, reg::PC, target & !1);
        let mut cpsr = self.register(reg::CPSR);
        if target & 1 != 0 {
            cpsr |= 1 << 5
        } else {
            cpsr &= !(1 << 5)
        }
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
    }

    pub(super) fn invoke_callback(
        &mut self,
        entry: u32,
        r0: u32,
        r1: u32,
        r2: u32,
        instruction_limit: u64,
    ) -> Result<()> {
        if entry == 0 {
            return Ok(());
        }
        self.cpu.reg_set(Mode::User, 0, r0);
        self.cpu.reg_set(Mode::User, 1, r1);
        self.cpu.reg_set(Mode::User, 2, r2);
        self.cpu.reg_set(Mode::User, reg::LR, EXIT_ADDRESS | 1);
        self.cpu.reg_set(Mode::User, reg::PC, entry & !1);
        let mut cpsr = self.cpu.reg_get(Mode::User, reg::CPSR);
        if entry & 1 != 0 {
            cpsr |= 1 << 5;
        } else {
            cpsr &= !(1 << 5);
        }
        self.cpu.reg_set(Mode::User, reg::CPSR, cpsr);
        self.run_until_return(instruction_limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm_blx_immediate_switch_target_includes_h_bit() {
        assert_eq!(
            arm_blx_immediate_target(0x0102_e6d4, 0xfa00_0004),
            Some(0x0102_e6ec)
        );
        assert_eq!(arm_blx_immediate_target(0x1000, 0xfb00_0000), Some(0x100a));
        assert_eq!(arm_blx_immediate_target(0x1000, 0xea00_0000), None);
    }

    #[test]
    fn thumb_add_pc_preserves_halfword_aligned_program_counter() {
        assert_eq!(
            thumb_add_pc_target(0x0100_112c, 0x449f, 0x18),
            Some(0x0100_1148)
        );
        assert_eq!(
            thumb_add_pc_target(0x0100_81e6, 0x449f, 0x20),
            Some(0x0100_820a)
        );
        assert_eq!(thumb_add_pc_target(0x1000, 0x4478, 4), None);
    }
}
