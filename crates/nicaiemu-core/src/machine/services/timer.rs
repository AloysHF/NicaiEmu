//! Guest timer services (group 7).

use anyhow::Result;

use super::super::{GuestTimer, NicaiMachine, MAX_TIMERS, TIMER_BASE_ID, TIMER_FRAME_MS};

impl NicaiMachine {
    pub(crate) fn handle_timer_service(&mut self, index: u32) {
        match index {
            // vMStartTimer(delay_ms, callback, context) -> timer handle.
            0 => {
                let delay_ms = self.register(0);
                let callback = self.register(1);
                let context = self.register(2);
                if callback == 0 {
                    self.set_result(0);
                    return;
                }
                let frames = delay_ms.saturating_add(TIMER_FRAME_MS - 1) / TIMER_FRAME_MS;
                let handle = self.timers.iter().position(|timer| !timer.active);
                match handle {
                    Some(slot) => {
                        self.timers[slot] = GuestTimer {
                            active: true,
                            callback,
                            context,
                            remaining_frames: frames.max(1),
                        };
                        self.set_result(TIMER_BASE_ID + slot as u32);
                    }
                    None => self.set_result(0),
                }
            }
            // vMStopTimer(handle).
            1 => {
                let handle = self.register(0);
                if handle >= TIMER_BASE_ID && (handle - TIMER_BASE_ID) < MAX_TIMERS as u32 {
                    self.timers[(handle - TIMER_BASE_ID) as usize].active = false;
                }
                self.set_result(0);
            }
            // vMGetTickCount() -> milliseconds since the frame counter started.
            2 => self.set_result((self.frame_count * TIMER_FRAME_MS as u64) as u32),
            // vMGetTotalSeconds / vMGetCurrentTime -> synthetic epoch seconds.
            3 | 4 => self.set_result(
                1_786_080_000u32
                    .wrapping_add((self.frame_count * TIMER_FRAME_MS as u64 / 1000) as u32),
            ),
            // vMSysSleep.
            5 => self.set_result(0),
            _ => self.set_result(0),
        }
    }

    /// Advance scheduled timers by one frame and fire due callbacks.
    pub(crate) fn dispatch_timers(&mut self, instruction_limit: u64) -> Result<()> {
        let mut due = Vec::new();
        for timer in &mut self.timers {
            if !timer.active {
                continue;
            }
            timer.remaining_frames = timer.remaining_frames.saturating_sub(1);
            if timer.remaining_frames == 0 {
                timer.active = false;
                due.push((timer.callback, timer.context));
            }
        }
        for (callback, context) in due {
            self.invoke_callback(callback, context, 0, 0, instruction_limit)?;
        }
        Ok(())
    }
}
