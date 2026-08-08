// libretro API implementation for the Nicai/MStar CBE emulator.

#![allow(static_mut_refs)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::callbacks;
use super::constants::*;
use super::types::*;

const DISPLAY_WIDTH: u32 = 240;
const DISPLAY_HEIGHT: u32 = 400;
const DISPLAY_FPS: f64 = 30.0;

// ============================================================
// Callback registration
// ============================================================

#[no_mangle]
pub extern "C" fn retro_set_environment(cb: retro_environment_t) {
    callbacks::set_environment(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_video_refresh(cb: retro_video_refresh_t) {
    callbacks::set_video_refresh(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample(cb: retro_audio_sample_t) {
    callbacks::set_audio_sample(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(cb: retro_audio_sample_batch_t) {
    callbacks::set_audio_sample_batch(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_input_poll(cb: retro_input_poll_t) {
    callbacks::set_input_poll(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_input_state(cb: retro_input_state_t) {
    callbacks::set_input_state(cb);
}

#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(_port: u32, _device: u32) {
    // The Nicai phone keypad is exposed as a single RetroPad.
}

// ============================================================
// Lifecycle
// ============================================================

#[no_mangle]
pub extern "C" fn retro_api_version() -> u32 {
    RETRO_API_VERSION
}

#[no_mangle]
pub extern "C" fn retro_init() {
    callbacks::init_log();
    super::logger::init();
    log::info!("NicaiEmu libretro core initialized");
}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    log::info!("NicaiEmu libretro core deinitialized");
}

#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut retro_system_info) {
    unsafe {
        (*info) = retro_system_info {
            library_name: c"NicaiEmu".as_ptr(),
            library_version: c"0.1.0".as_ptr(),
            valid_extensions: c"cbe".as_ptr(),
            need_fullpath: true,
            block_extract: false,
        };
    }
}

#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut retro_system_av_info) {
    unsafe {
        (*info) = retro_system_av_info {
            geometry: retro_game_geometry {
                base_width: DISPLAY_WIDTH,
                base_height: DISPLAY_HEIGHT,
                max_width: DISPLAY_WIDTH,
                max_height: DISPLAY_HEIGHT,
                aspect_ratio: DISPLAY_WIDTH as f32 / DISPLAY_HEIGHT as f32,
            },
            timing: retro_system_timing {
                fps: DISPLAY_FPS,
                // Audio output arrives in a later milestone.
                sample_rate: 0.0,
            },
        };
    }
}

// ============================================================
// Content lifecycle (implemented in later milestones)
// ============================================================

#[no_mangle]
pub extern "C" fn retro_load_game(_info: *const retro_game_info) -> bool {
    log::error!("retro_load_game is not implemented yet");
    false
}

#[no_mangle]
pub extern "C" fn retro_load_game_special(
    _type: u32,
    _info: *const retro_game_info,
    _num: usize,
) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_unload_game() {
    log::info!("Game unloaded");
}

#[no_mangle]
pub extern "C" fn retro_run() {
    log::warn!("retro_run called without loaded content");
}

#[no_mangle]
pub extern "C" fn retro_reset() {
    log::warn!("retro_reset called without loaded content");
}

#[no_mangle]
pub extern "C" fn retro_get_region() -> u32 {
    RETRO_REGION_NTSC
}

// ============================================================
// Save states (implemented in a later milestone)
// ============================================================

#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    0
}

#[no_mangle]
pub extern "C" fn retro_serialize(_data: *mut std::ffi::c_void, _size: usize) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_unserialize(_data: *const std::ffi::c_void, _size: usize) -> bool {
    false
}
