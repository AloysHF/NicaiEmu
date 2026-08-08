// libretro API implementation for the Nicai/MStar CBE emulator.

#![allow(static_mut_refs)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::callbacks;
use super::constants::*;
use super::types::*;
use nicaiemu_core::{CbeArchive, NicaiMachine};
use std::ffi::{c_char, c_void, CStr};
use std::path::Path;
use std::ptr;

const DISPLAY_WIDTH: u32 = 240;
const DISPLAY_HEIGHT: u32 = 400;
const DISPLAY_FPS: f64 = 30.0;
const PERFORMANCE_LEVEL: u32 = 3;
const DEFAULT_INSTRUCTION_LIMIT: u64 = 5_000_000;

/// Loaded emulator state shared by the libretro entry points.
struct Emulator {
    archive: CbeArchive,
    machine: NicaiMachine,
    instruction_limit: u64,
    stopped: bool,
}

/// Global emulator instance.
static mut EMULATOR: Option<Emulator> = None;

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
    unsafe {
        EMULATOR = None;
    }
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
// Content lifecycle
// ============================================================

#[no_mangle]
pub extern "C" fn retro_load_game(info: *const retro_game_info) -> bool {
    unsafe {
        if info.is_null() {
            log::error!("Game info is null");
            return false;
        }
        let game_info = &*info;
        if game_info.path.is_null() {
            log::error!("Game path is null");
            return false;
        }
        let path = match CStr::from_ptr(game_info.path).to_str() {
            Ok(path) => path,
            Err(error) => {
                log::error!("Invalid game path: {error}");
                return false;
            }
        };

        let pixel_format = retro_pixel_format::RETRO_PIXEL_FORMAT_XRGB8888;
        if !callbacks::environment(
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
            &pixel_format as *const _ as *mut c_void,
        ) {
            log::error!("Failed to set pixel format");
            return false;
        }

        register_input_descriptors();

        callbacks::environment(
            RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL,
            &PERFORMANCE_LEVEL as *const _ as *mut c_void,
        );

        match load_machine(Path::new(path)) {
            Ok((archive, mut machine)) => {
                if let Err(error) = machine.boot(DEFAULT_INSTRUCTION_LIMIT) {
                    log::error!("Failed to boot CBE application: {error:#}");
                    return false;
                }
                log::info!("Game loaded: {path}");
                EMULATOR = Some(Emulator {
                    archive,
                    machine,
                    instruction_limit: DEFAULT_INSTRUCTION_LIMIT,
                    stopped: false,
                });
                true
            }
            Err(error) => {
                log::error!("Failed to load game: {error:#}");
                false
            }
        }
    }
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
    unsafe {
        EMULATOR = None;
    }
    log::info!("Game unloaded");
}

#[no_mangle]
pub extern "C" fn retro_run() {
    unsafe {
        let Some(emulator) = EMULATOR.as_mut() else {
            log::warn!("retro_run called before loading a game");
            return;
        };

        callbacks::input_poll();
        update_phone_keys(emulator);

        if !emulator.stopped {
            if let Err(error) = emulator.machine.run_frame(emulator.instruction_limit) {
                log::warn!("CBE frame callback stopped: {error:#}");
                emulator.stopped = true;
            }
        }

        // Present the last valid guest framebuffer even after a stop.
        let pixels: Vec<u32> = emulator
            .machine
            .frame_pixels()
            .into_iter()
            .map(|pixel| 0xFF00_0000 | pixel)
            .collect();
        callbacks::video_refresh(
            pixels.as_ptr() as *const c_void,
            DISPLAY_WIDTH,
            DISPLAY_HEIGHT,
            (DISPLAY_WIDTH * 4) as usize,
        );
    }
}

fn load_machine(path: &Path) -> anyhow::Result<(CbeArchive, NicaiMachine)> {
    let archive = CbeArchive::load(path)?;
    let machine = NicaiMachine::new(&archive)?;
    Ok((archive, machine))
}

fn input_descriptors() -> [retro_input_descriptor; 10] {
    [
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_UP,
            description: c"D-Pad Up".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_DOWN,
            description: c"D-Pad Down".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_LEFT,
            description: c"D-Pad Left".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_RIGHT,
            description: c"D-Pad Right".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_A,
            description: c"Confirm".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_B,
            description: c"Confirm".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_X,
            description: c"Left Soft Key".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_Y,
            description: c"Right Soft Key".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_START,
            description: c"Confirm".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_NONE,
            index: 0,
            id: 0,
            description: ptr::null(),
        },
    ]
}

fn register_input_descriptors() {
    let descriptors = input_descriptors();
    callbacks::environment(
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS,
        descriptors.as_ptr() as *mut c_void,
    );
}

/// Map RetroPad buttons to phone keypad ABI key codes.
fn update_phone_keys(emulator: &mut Emulator) {
    let joypad = |id: u32| callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, id) != 0;
    emulator
        .machine
        .set_key(17, joypad(RETRO_DEVICE_ID_JOYPAD_UP));
    emulator
        .machine
        .set_key(18, joypad(RETRO_DEVICE_ID_JOYPAD_DOWN));
    emulator
        .machine
        .set_key(15, joypad(RETRO_DEVICE_ID_JOYPAD_LEFT));
    emulator
        .machine
        .set_key(16, joypad(RETRO_DEVICE_ID_JOYPAD_RIGHT));
    let confirm = joypad(RETRO_DEVICE_ID_JOYPAD_A)
        || joypad(RETRO_DEVICE_ID_JOYPAD_B)
        || joypad(RETRO_DEVICE_ID_JOYPAD_START);
    emulator.machine.set_key(14, confirm);
    emulator
        .machine
        .set_key(12, joypad(RETRO_DEVICE_ID_JOYPAD_X));
    emulator
        .machine
        .set_key(13, joypad(RETRO_DEVICE_ID_JOYPAD_Y));
}

#[no_mangle]
pub extern "C" fn retro_reset() {
    unsafe {
        let Some(emulator) = EMULATOR.as_mut() else {
            log::warn!("retro_reset called before loading a game");
            return;
        };
        match NicaiMachine::new(&emulator.archive).and_then(|mut machine| {
            machine.boot(emulator.instruction_limit)?;
            Ok(machine)
        }) {
            Ok(machine) => {
                emulator.machine = machine;
                emulator.stopped = false;
                log::info!("Game reset");
            }
            Err(error) => log::error!("Failed to reset game: {error:#}"),
        }
    }
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

// ============================================================
// Optional API stubs required for frontend symbol resolution
// ============================================================

#[no_mangle]
pub extern "C" fn retro_cheat_reset() {
    // Cheat slots arrive in a later milestone.
}

#[no_mangle]
pub extern "C" fn retro_cheat_set(_index: u32, _enabled: bool, _code: *const c_char) {
    // Cheat slots arrive in a later milestone.
}

#[no_mangle]
pub extern "C" fn retro_get_memory_data(_id: u32) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn retro_get_memory_size(_id: u32) -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::sync::Mutex;

    /// Captured video frame: width, height, pitch, and raw XRGB8888 bytes.
    type VideoFrame = (u32, u32, usize, Vec<u8>);

    static LAST_VIDEO: Mutex<Option<VideoFrame>> = Mutex::new(None);

    unsafe extern "C" fn test_environment(cmd: u32, _data: *mut c_void) -> bool {
        matches!(
            cmd,
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT
                | RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS
                | RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL
        )
    }

    unsafe extern "C" fn test_video_refresh(
        data: *const c_void,
        width: u32,
        height: u32,
        pitch: usize,
    ) {
        let bytes = if data.is_null() {
            Vec::new()
        } else {
            let len = pitch.saturating_mul(height as usize).min(240 * 400 * 4);
            std::slice::from_raw_parts(data as *const u8, len).to_vec()
        };
        *LAST_VIDEO.lock().unwrap() = Some((width, height, pitch, bytes));
    }

    unsafe extern "C" fn test_input_poll() {}

    unsafe extern "C" fn test_input_state(_port: u32, _device: u32, _index: u32, _id: u32) -> i16 {
        0
    }

    #[test]
    fn system_info_reports_metadata_and_cbe_extensions() {
        let mut info = std::mem::MaybeUninit::<retro_system_info>::zeroed();
        retro_get_system_info(info.as_mut_ptr());
        let info = unsafe { info.assume_init() };
        assert_eq!(
            unsafe { CStr::from_ptr(info.library_name) }
                .to_str()
                .unwrap(),
            "NicaiEmu"
        );
        assert_eq!(
            unsafe { CStr::from_ptr(info.valid_extensions) }
                .to_str()
                .unwrap(),
            "cbe"
        );
        assert!(info.need_fullpath);
        assert!(!info.block_extract);
    }

    #[test]
    fn av_info_reports_native_display_and_timing() {
        let mut av = std::mem::MaybeUninit::<retro_system_av_info>::zeroed();
        retro_get_system_av_info(av.as_mut_ptr());
        let av = unsafe { av.assume_init() };
        assert_eq!(av.geometry.base_width, DISPLAY_WIDTH);
        assert_eq!(av.geometry.base_height, DISPLAY_HEIGHT);
        assert_eq!(av.geometry.max_width, DISPLAY_WIDTH);
        assert_eq!(av.geometry.max_height, DISPLAY_HEIGHT);
        assert!(
            (av.geometry.aspect_ratio - DISPLAY_WIDTH as f32 / DISPLAY_HEIGHT as f32).abs() < 1e-6
        );
        assert_eq!(av.timing.fps, DISPLAY_FPS);
        assert_eq!(av.timing.sample_rate, 0.0);
    }

    #[test]
    fn input_descriptors_cover_retropad_mapping() {
        let descriptors = input_descriptors();
        let ids: Vec<u32> = descriptors[..9]
            .iter()
            .map(|descriptor| descriptor.id)
            .collect();
        assert_eq!(
            ids,
            [
                RETRO_DEVICE_ID_JOYPAD_UP,
                RETRO_DEVICE_ID_JOYPAD_DOWN,
                RETRO_DEVICE_ID_JOYPAD_LEFT,
                RETRO_DEVICE_ID_JOYPAD_RIGHT,
                RETRO_DEVICE_ID_JOYPAD_A,
                RETRO_DEVICE_ID_JOYPAD_B,
                RETRO_DEVICE_ID_JOYPAD_X,
                RETRO_DEVICE_ID_JOYPAD_Y,
                RETRO_DEVICE_ID_JOYPAD_START,
            ]
        );
        assert_eq!(descriptors[9].device, RETRO_DEVICE_NONE);
        assert!(descriptors[9].description.is_null());
    }

    #[test]
    fn load_game_rejects_null_and_missing_paths() {
        retro_set_environment(test_environment);
        assert!(!retro_load_game(ptr::null()));

        let missing = CString::new("does-not-exist.CBE").unwrap();
        let info = retro_game_info {
            path: missing.as_ptr(),
            data: ptr::null(),
            size: 0,
            meta: ptr::null(),
        };
        assert!(!retro_load_game(&info));
        unsafe {
            assert!(EMULATOR.is_none());
        }
    }

    #[test]
    fn lifecycle_without_content_is_safe() {
        retro_set_environment(test_environment);
        retro_set_video_refresh(test_video_refresh);
        retro_set_input_poll(test_input_poll);
        retro_set_input_state(test_input_state);
        retro_init();
        retro_run();
        retro_reset();
        assert_eq!(retro_serialize_size(), 0);
        assert!(!retro_serialize(ptr::null_mut(), 0));
        assert!(!retro_unserialize(ptr::null(), 0));
        retro_deinit();
    }

    #[test]
    fn metadata_manifests_match_implemented_features() {
        let core_info = include_str!("../../nicaiemu_libretro.info");
        let libretro_manifest = include_str!("../../Cargo.toml");
        let workspace_manifest = include_str!("../../../../Cargo.toml");
        let buildbot_config = include_str!("../../../../.gitlab-ci.yml");

        assert!(libretro_manifest.contains("name = \"nicaiemu-libretro\""));
        assert!(libretro_manifest.contains("name = \"nicaiemu\""));
        assert!(workspace_manifest.contains("\"crates/nicaiemu-libretro\""));
        assert!(buildbot_config.contains("CORENAME: nicaiemu"));
        assert!(core_info.contains("corename = \"nicaiemu\""));
        assert!(core_info.contains("savestate = \"false\""));
        assert!(core_info.contains("cheats = \"false\""));
        assert!(core_info.contains("input_descriptors = \"true\""));
        assert!(core_info.contains("memory_descriptors = \"false\""));
        assert!(core_info.contains("core_options = \"false\""));
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_boots_and_renders_frames() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let mut candidates: Vec<std::path::PathBuf> = std::fs::read_dir(&game_dir)
            .unwrap()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("cbe"))
            })
            .collect();
        candidates.sort();
        let game_path = candidates
            .first()
            .expect("no .CBE file found in NICAI_GAME_DIR");
        let game_path = CString::new(game_path.to_string_lossy().as_bytes()).unwrap();

        retro_set_environment(test_environment);
        retro_set_video_refresh(test_video_refresh);
        retro_set_input_poll(test_input_poll);
        retro_set_input_state(test_input_state);
        retro_init();

        let info = retro_game_info {
            path: game_path.as_ptr(),
            data: ptr::null(),
            size: 0,
            meta: ptr::null(),
        };
        assert!(
            retro_load_game(&info),
            "failed to load {}",
            game_path.to_string_lossy()
        );

        for _ in 0..120 {
            retro_run();
        }

        let last = LAST_VIDEO.lock().unwrap();
        let (width, height, _, bytes) = last.as_ref().expect("video refresh was never called");
        assert_eq!(*width, DISPLAY_WIDTH);
        assert_eq!(*height, DISPLAY_HEIGHT);
        assert_eq!(bytes.len(), (DISPLAY_WIDTH * DISPLAY_HEIGHT * 4) as usize);
        drop(last);

        retro_reset();
        for _ in 0..5 {
            retro_run();
        }
        assert!(LAST_VIDEO.lock().unwrap().is_some());

        retro_unload_game();
        retro_deinit();
    }
}
