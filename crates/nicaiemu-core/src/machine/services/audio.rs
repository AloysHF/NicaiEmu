//! Audio manager services (group 18).

use armv4t_emu::Memory;

use super::super::NicaiMachine;

impl NicaiMachine {
    pub(crate) fn handle_audio_service(&mut self, index: u32) {
        match index {
            // vMAudioSetVolume(volume)
            0 => {
                self.audio.set_volume(self.register(0));
                self.set_result(0);
            }
            // vMAudioPlayByData(pointer, length)
            1 => self.play_guest_audio(self.register(0), self.register(1)),
            // vMAudioPlayWithDataPackage: data-package ABI not recovered yet.
            2 => self.set_result(0),
            // vMAudioPlayForGame / vMAudioPlayForApp.
            3 | 4 => self.set_result(0),
            // vMAudioPause.
            5 => {
                self.audio.pause();
                self.set_result(0);
            }
            // vMAudioResume.
            6 => {
                self.audio.resume();
                self.set_result(0);
            }
            // vMAudioStop.
            7 => {
                self.audio.stop();
                self.set_result(0);
            }
            // vMAduioGetState.
            8 => self.set_result(self.audio.state()),
            // vm_mp3PlayBystream(pointer, length).
            9 => self.play_guest_audio(self.register(0), self.register(1)),
            // vm_mp3PauseByStream.
            10 => {
                self.audio.pause();
                self.set_result(0);
            }
            // vm_mp3ResumeByStream.
            11 => {
                self.audio.resume();
                self.set_result(0);
            }
            // vm_mp3StopBystream.
            12 => {
                self.audio.stop();
                self.set_result(0);
            }
            // File-based MP3 control and progress remain neutral until the
            // file-service ABI is recovered.
            13..=17 => self.set_result(0),
            _ => self.set_result(0),
        }
    }

    /// Read a CBE audio resource from guest memory and queue decoded PCM.
    ///
    /// `repeats` is the firmware's loop count for `vMAudioPlayByData`.
    fn play_guest_audio(&mut self, pointer: u32, length: u32) {
        const MAX_AUDIO_BYTES: u32 = 4 * 1024 * 1024;
        let header: [u8; 5] = [
            self.memory.r8(pointer),
            self.memory.r8(pointer.wrapping_add(1)),
            self.memory.r8(pointer.wrapping_add(2)),
            self.memory.r8(pointer.wrapping_add(3)),
            self.memory.r8(pointer.wrapping_add(4)),
        ];
        let payload_len = ((header[2] as u32) << 16) | ((header[3] as u32) << 8) | header[4] as u32;
        let total = 5u32.saturating_add(payload_len).min(MAX_AUDIO_BYTES);
        let bytes: Vec<u8> = (0..total)
            .map(|offset| self.memory.r8(pointer.wrapping_add(offset)))
            .collect();
        match self.audio.play_bytes_repeats(&bytes, length) {
            Ok(()) => log::debug!("Guest audio queued ({} bytes)", bytes.len()),
            Err(error) => log::warn!("Guest audio rejected: {error:#}"),
        }
        self.set_result(0);
    }
}
