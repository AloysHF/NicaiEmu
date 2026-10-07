//! System, root and control manager services (groups 0, 1 and 8).

use armv4t_emu::Memory;

use super::super::{
    fixed_manager_specs, manager_initializer_count, NicaiMachine, DL_IMAGE_MANAGER,
    DL_LOAD_MANAGER, DL_PAY_MANAGER, DL_RESOURCE_MANAGER, MANAGER_BASE, METHOD_KIND_AUTO,
    SERVICE_BASE, TABLE_STRIDE, VIDEO_MANAGER,
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
            28 | 29 => Some(29),
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

    /// Group 17 — the VmManager Init/Get directory (vmspec F_17).  Each
    /// manager is exposed as an Init/Get pair: the Init half may install a
    /// private table at the caller's pointer, and the Get half must hand
    /// back a non-null manager object.  Returning zero here made the guest
    /// dereference NULL fields at +0x38..+0x50 and abandon startup.
    pub(crate) fn handle_manager_service(&mut self, index: u32) {
        // Init/Get pairs share one slot in `fixed_manager_specs`; the odd
        // half of each pair is the getter.  A handful of trailing F_17
        // entries break the pairing and are mapped explicitly.
        let group = match index {
            0 | 2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 | 18 | 20 | 22 | 24 | 26 | 28 | 30 | 32 => {
                // VMInitXxxManager(void *) — populate the caller's table
                // when one is supplied and hand back the shared manager
                // object, matching the fixed-ABI init stub.  The guest
                // passes a slot pointer and then dereferences fields of the
                // manager it expects to find there, so a bare zero return
                // leaves it holding NULL.
                let slot = index / 2;
                let destination = self.register(0);
                let spec = fixed_manager_specs().get(slot as usize).copied();
                if destination != 0 {
                    if let Some((_, group, count)) = spec {
                        self.populate_table(
                            destination,
                            SERVICE_BASE + TABLE_STRIDE * group,
                            count,
                        );
                    }
                }
                let group = spec.map(|(_, group, _)| group).unwrap_or(1);
                self.set_result(MANAGER_BASE + TABLE_STRIDE * (group + 1));
                return;
            }
            34 | 35 | 37 | 39 | 42 | 44 => {
                // Trailing Init-only entries (VMInitManager, vMInitGSensor,
                // vMInitVmStd, vMInitDlLoad, vMInitDlRs, vMInitDlImage).
                self.set_result(0);
                return;
            }
            1 => 5,   // VMGetIoManager
            3 => 4,   // VMGetLcdManager
            5 => 7,   // VMGetTimeManager
            7 => 8,   // VMGetCtrlManager
            9 => 2,   // VMGetMemoryManager
            11 => 12, // VMGetBillingManager
            13 => 14, // VMGetScreenManager
            15 => 9,  // VMGetNetManager
            17 => 13, // VMGetUcs2StrManager
            19 => 1,  // VMGetSysManager
            21 => 15, // VMGetDFScriptManager
            23 => 16, // VMGetGameLcdManager
            25 => 10, // VMGetGameUtilManager
            27 => 11, // VMGetDFEnginelManager
            29 => 29, // VMGetNetAppManager
            31 => 18, // VMGetAudioManager
            33 => 3,  // VMGetGameManagerOld
            36 => 19, // vMGetGSensorManager
            38 => 22, // vMGetVmStdManager
            40 => 22, // vMGetDlLoadManager
            41 => 25, // vMGetDlRsManager
            43 => 26, // vMGetDlImageManager
            _ => {
                self.set_result(0);
                return;
            }
        };
        // Shared dense function table for the manager, so the guest can
        // treat the result as an object with callable slots.
        self.set_result(MANAGER_BASE + TABLE_STRIDE * (group + 1));
    }

    /// Group 8 — the control manager (fixed-manager slot 3).  Index 6 is the
    /// control factory: the guest passes a slot pointer, expects an instance
    /// written back into it, and then chains calls through the instance's
    /// method block at +0xE4..+0x138.  Returning zero left the slot empty, so
    /// the first `ldr r2, [obj, #0xe4]; bx r2` jumped to NULL.
    pub(crate) fn handle_ctrl_service(&mut self, index: u32) {
        match index {
            6 => {
                const CONTROL_SIZE: u32 = 0x200;
                let slot = self.register(0);
                let mut object = 0;
                if slot != 0 {
                    object = self.allocate(CONTROL_SIZE);
                    if object != 0 {
                        for offset in (0..CONTROL_SIZE).step_by(4) {
                            self.memory.w32(
                                object + offset,
                                Self::method_stub_address(METHOD_KIND_AUTO, offset),
                            );
                        }
                        self.memory.w32(slot, object);
                    }
                }
                self.set_result(object);
            }
            _ => self.set_result(0),
        }
    }

    pub(crate) fn handle_net_app_service(&mut self, index: u32) {
        let descriptor = self.register(0);
        if (index == 0 || index == 1) && descriptor != 0 {
            let callback = self.memory.r32(descriptor.wrapping_add(4));
            self.defer_callback(callback, vec![], "netAppEntry");
        }
        self.set_result(0);
    }

    /// Group 12 — the billing manager (vmspec F_2).  Games gate their
    /// startup on these: without a successful SMS/pay handshake the flow
    /// ends on a blank screen.  Payments report success through a deferred
    /// callback the firmware delivers after the service returns.
    pub(crate) fn handle_billing_service(&mut self, index: u32) {
        match index {
            0 => self.set_result(0),                                // GetPayNumByAppId
            1 => self.set_result(u32::from(self.register(1) == 3)), // GetRemainDay
            2 | 3 | 26 | 30 => {
                // Pay / PayMoreTimes / Pay2 / Pay3 — succeed through the
                // callback; the callback travels in r2 for every variant.
                let callback = self.argument(2);
                self.defer_callback(callback, vec![1], "payResult");
                self.set_result(0);
            }
            4 => {
                // IsRegisterBillingInfo — never pre-registered offline.
                let id = self.register(0) as u16;
                let used = self.billing_reg.get(&id).map(|e| e.1).unwrap_or(0);
                self.set_result(u32::from(used != 0));
            }
            5 => {
                // RegisterBillingInfo
                let id = self.register(0) as u16;
                self.billing_reg.insert(id, (0, 1));
                self.set_result(1);
            }
            6 => {
                // SetBillingStatus
                let id = self.register(0) as u16;
                let value = self.register(1) as u8;
                let ok = self.billing_reg.get_mut(&id).map(|e| e.0 = value).is_some();
                self.set_result(u32::from(ok));
            }
            7 => {
                // GetBillingStatus — unregistered apps read as active.
                let id = self.register(0) as u16;
                let status = self
                    .billing_reg
                    .get(&id)
                    .map(|e| u32::from(e.0))
                    .unwrap_or(1);
                self.set_result(status);
            }
            8 => self.set_result(0),  // IsNeedPay
            9 => self.set_result(1),  // IsInTryStatus
            10 => self.set_result(0), // OpenBillingPromptWin
            11 => self.set_result(1), // GetTryDay
            12 => {
                // PayForCBB
                let callback = self.argument(3);
                self.defer_callback(callback, vec![1], "payResult");
                self.set_result(0);
            }
            13 => {
                // PayForPwd
                let callback = self.argument(0);
                self.defer_callback(callback, vec![1], "payResult");
                self.set_result(0);
            }
            14 => self.set_result(0), // GetCdownOption5
            15 => {
                // Billing_SendSpecSms — the firmware only accepts app id 14
                // and reports the SMS outcome through the trailing callback.
                if self.register(0) == 14 {
                    let callback = self.argument(6);
                    self.defer_callback(callback, vec![0], "smsResult");
                    self.set_result(1);
                } else {
                    self.set_result(0);
                }
            }
            16 => {
                // Billing_CancelSms — drop the pending SMS result.
                self.pending_callbacks
                    .retain(|(_, _, tag)| *tag != "smsResult");
                self.set_result(1);
            }
            17 | 20 => self.set_result(0), // CDownIsMonthApp / NewMonthPay
            18 => {
                // CDownGetFileNameByAppID — no file name offline.
                let out = self.register(1);
                if out != 0 {
                    self.memory.w32(out, 0);
                }
                self.set_result(0);
            }
            19 => self.set_result(2), // GetSmsNum
            21 => {
                // NewMonthCancel
                let callback = self.argument(1);
                self.defer_callback(callback, vec![1], "payResult");
                self.set_result(0);
            }
            22 => {
                // CleanAppMonthBillInfo
                let id = self.register(0) as u16;
                self.billing_reg.remove(&id);
                self.set_result(1);
            }
            23 => self.set_result(30),          // GetValidDayByAppId
            24 | 25 | 29 => self.set_result(0), // SMS address / suffix / tip
            27 => {
                // GetAppUsedStatus
                let id = self.register(0) as u16;
                let used = self
                    .billing_reg
                    .get(&id)
                    .map(|e| u32::from(e.1))
                    .unwrap_or(0);
                self.set_result(used);
            }
            28 => {
                // SetAppUsedStatus
                let id = self.register(0) as u16;
                let value = self.register(1) as u8;
                if let Some(entry) = self.billing_reg.get_mut(&id) {
                    entry.1 = value;
                }
                self.set_result(0);
            }
            _ => self.set_result(0),
        }
    }

    /// Queue a guest callback for delivery after the initiating service
    /// returns, matching the firmware's deferred-result contract.
    fn defer_callback(&mut self, entry: u32, args: Vec<u32>, tag: &'static str) {
        if std::env::var("CBE_TRACE").is_ok() {
            let sp = self.register(armv4t_emu::reg::SP);
            eprintln!("[defer] tag={tag} entry={entry:08X} args={args:?} sp={sp:08X}");
        }
        if entry != 0 {
            self.pending_callbacks.push_back((entry, args, tag));
        }
    }

    /// Deliver queued guest callbacks (billing results and the like).
    pub(crate) fn dispatch_pending_callbacks(
        &mut self,
        instruction_limit: u64,
    ) -> anyhow::Result<()> {
        // Deliver at most this many per frame, matching the firmware pump.
        let mut budget = 64usize;
        while budget > 0 {
            let Some((entry, args, tag)) = self.pending_callbacks.pop_front() else {
                break;
            };
            budget -= 1;
            let r0 = args.first().copied().unwrap_or(0);
            let r1 = args.get(1).copied().unwrap_or(0);
            let r2 = args.get(2).copied().unwrap_or(0);
            if std::env::var("CBE_TRACE").is_ok() {
                eprintln!(
                    "[cb] begin tag={tag} entry={entry:08X} r0={r0:08X} frame={} icount={}",
                    self.frame_count, self.instruction_count
                );
            }
            self.invoke_callback(entry, r0, r1, r2, instruction_limit)?;
            if std::env::var("CBE_TRACE").is_ok() {
                eprintln!(
                    "[cb] end tag={tag} entry={entry:08X} state={:?} icount={}",
                    self.state, self.instruction_count
                );
            }
            if self.state == super::super::MachineState::Halted {
                return Ok(());
            }
        }
        Ok(())
    }
}
