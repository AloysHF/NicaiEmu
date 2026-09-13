//! Audio manager services (group 18).

use armv4t_emu::Memory;

use super::super::NicaiMachine;

impl NicaiMachine {
    pub(crate) fn handle_audio_service(&mut self, index: u32) {
        // Any explicit guest audio-manager call means the game owns audio;
        // hand the auto-BGM compatibility layer over to the game.
        if self.auto_bgm {
            self.auto_bgm_gave_way = true;
            self.audio.stop();
            self.playing_resource_id = None;
        }
        match index {
            // vMAudioSetVolume(volume)
            0 => {
                self.audio.set_volume(self.register(0));
                self.set_result(0);
            }
            // vMAudioPlayByData(pointer, length)
            1 => self.play_guest_audio(self.register(0), self.register(1)),
            // vMAudioPlayWithDataPackage(id, repeats) — same resource-id ABI
            // as PlayForGame. Corpus evidence (碰嘭球): r0=270 maps to the
            // packaged `game.mid`; r2 is an unused package/context pointer.
            2 => {
                let id = self.register(0);
                let repeats = self.register(1);
                self.play_resource_by_id(id, repeats, true);
            }
            // vMAudioPlayForGame(id, repeats) / vMAudioPlayForApp(id, repeats).
            // Guest r0 is the package resource id; r1 is the firmware repeat
            // count. PlayForGame is the corpus's dominant BGM path and is
            // looped until Stop; PlayForApp is one-shot unless repeats > 0.
            3 => {
                let id = self.register(0);
                let repeats = self.register(1);
                self.play_resource_by_id(id, repeats, true);
            }
            4 => {
                let id = self.register(0);
                let repeats = self.register(1);
                self.play_resource_by_id(id, repeats, repeats > 0);
            }
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
                self.playing_resource_id = None;
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
                self.playing_resource_id = None;
                self.set_result(0);
            }
            // vm_mp3PlayByFile(path, repeats) — path is a GBK/UCS2 guest
            // string resolved through the sandboxed VFS, with a fallback to
            // packaged resources that share the same basename.
            13 => {
                let path = self.read_file_path(self.register(0));
                let repeats = self.register(1);
                self.play_file_audio(&path, repeats);
            }
            // vm_mp3PauseByFile / ResumeByFile / StopByFile. The corpus has
            // no caller yet; pause/resume/stop the single engine channel.
            14 => {
                self.audio.pause();
                self.set_result(0);
            }
            15 => {
                self.audio.resume();
                self.set_result(0);
            }
            16 => {
                self.audio.stop();
                self.playing_resource_id = None;
                self.set_result(0);
            }
            // vMAudioget_progress_time — no playback clock yet.
            17 => self.set_result(0),
            // vm_mp3PlayByFileEx — treated as the same path-based play.
            25 => {
                let path = self.read_file_path(self.register(0));
                let repeats = self.register(1);
                self.play_file_audio(&path, repeats);
            }
            _ => self.set_result(0),
        }
    }

    /// Queue the packaged resource referenced by a firmware resource id.
    ///
    /// `vMAudioPlayForGame` / `vMAudioPlayForApp` pass `(resource_id, repeats)`.
    /// Games often re-issue the same call every 100 ms tick as a BGM keep-alive;
    /// while the same id still has buffered audio, a restart is ignored so the
    /// track is not re-decoded and re-queued each frame.
    ///
    /// When `loop` is true the engine re-decodes the cue after the queue
    /// drains, matching firmware background-music behavior.
    pub(crate) fn play_resource_by_id(&mut self, id: u32, repeats: u32, bgm_loop: bool) {
        if self.playing_resource_id == Some(id)
            && self.audio.state() == 1
            && self.audio.buffered_frames() > 0
        {
            self.set_result(0);
            return;
        }
        let Some(bytes) = self.resource_bytes_by_id(id) else {
            log::warn!("PlayForGame: resource id {id} was not found");
            self.set_result(0);
            return;
        };
        match self.audio.play_bytes_repeats(&bytes, repeats) {
            Ok(()) => {
                self.playing_resource_id = Some(id);
                if bgm_loop {
                    self.audio.set_loop_source(bytes.clone());
                } else {
                    self.audio.clear_loop();
                }
                log::debug!(
                    "PlayForGame queued id={id} bytes={} repeats={repeats} loop={bgm_loop}",
                    bytes.len()
                );
            }
            Err(error) => {
                self.playing_resource_id = None;
                log::warn!("PlayForGame rejected id={id}: {error:#}");
            }
        }
        self.set_result(0);
    }

    /// Resolve a package resource id to its raw host bytes.
    ///
    /// Main-package ids are sequential and match [`Self::resources`]. Child
    /// package ids are looked up by the guest-visible name.
    fn resource_bytes_by_id(&mut self, id: u32) -> Option<Vec<u8>> {
        if (id as usize) < self.resources.len() {
            let pointer = self.resource_by_id(id);
            if pointer != 0 {
                return Some(self.resources[id as usize].data.clone());
            }
        }
        let name_pointer = self.resource_name_by_id(id);
        if name_pointer == 0 {
            return None;
        }
        let name = self.read_gbk_string(name_pointer, 256);
        let basename = name.rsplit(['/', '\\']).next().unwrap_or(&name).to_owned();
        self.resources
            .iter()
            .chain(
                self.resource_packages
                    .iter()
                    .flat_map(|package| package.resources.iter()),
            )
            .find(|resource| {
                let candidate = resource
                    .name
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&resource.name);
                candidate.eq_ignore_ascii_case(&basename)
            })
            .map(|resource| resource.data.clone())
    }

    /// Queue audio from a guest filesystem path (`vm_mp3PlayByFile*`).
    ///
    /// Looks up the sandboxed VFS first, then falls back to packaged CBE
    /// resources that share the same basename (some titles store BGM in the
    /// package but address it by filename).
    pub(crate) fn play_file_audio(&mut self, path: &str, repeats: u32) {
        if path.is_empty() {
            log::warn!("PlayByFile: empty path");
            self.set_result(0);
            return;
        }
        let bytes = self.virtual_fs.read_file(path).or_else(|| {
            let basename = path.rsplit(['/', '\\']).next().unwrap_or(path);
            self.resource_bytes_by_file_name(basename)
        });
        let Some(bytes) = bytes else {
            log::warn!("PlayByFile: path {path:?} was not found");
            self.set_result(0);
            return;
        };
        match self.audio.play_bytes_repeats(&bytes, repeats) {
            Ok(()) => {
                self.playing_resource_id = None;
                self.audio.clear_loop();
                log::debug!(
                    "PlayByFile queued path={path:?} bytes={} repeats={repeats}",
                    bytes.len()
                );
            }
            Err(error) => log::warn!("PlayByFile rejected path={path:?}: {error:#}"),
        }
        self.set_result(0);
    }

    fn resource_bytes_by_file_name(&mut self, basename: &str) -> Option<Vec<u8>> {
        self.resources
            .iter()
            .chain(
                self.resource_packages
                    .iter()
                    .flat_map(|package| package.resources.iter()),
            )
            .find(|resource| {
                let candidate = resource
                    .name
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&resource.name);
                candidate.eq_ignore_ascii_case(basename)
            })
            .map(|resource| resource.data.clone())
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
            Ok(()) => {
                self.playing_resource_id = None;
                self.audio.clear_loop();
                log::debug!("Guest audio queued ({} bytes)", bytes.len());
            }
            Err(error) => log::warn!("Guest audio rejected: {error:#}"),
        }
        self.set_result(0);
    }
}
