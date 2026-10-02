// libretro API implementation for the Nicai/MStar CBE emulator.

#![allow(static_mut_refs)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::callbacks;
use super::constants::*;
use super::options;
use super::types::*;
use nicaiemu_core::{
    decode_machine, encode_machine, load_orientation_overrides, CbeArchive, NicaiMachine,
    AUDIO_SAMPLE_RATE, DEFAULT_INSTRUCTION_LIMIT, GUEST_FRAME_RATE, SERIALIZED_SIZE,
};
use std::ffi::{c_char, c_void, CStr};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

const DISPLAY_WIDTH: u32 = 240;
const DISPLAY_HEIGHT: u32 = 400;
/// Frontend framebuffer bounds that fit both presented orientations: the
/// portrait 240x400 display and the rotated 400x240 landscape layout.
const MAX_DISPLAY_WIDTH: u32 = DISPLAY_HEIGHT;
const MAX_DISPLAY_HEIGHT: u32 = DISPLAY_HEIGHT;
/// Frontend pacing rate reported through the AV info.
///
/// The rate only decides how often the frontend calls `retro_run`; the
/// guest still advances its own 100 ms screen tick on every
/// [`PACING_FPS`] / [`GUEST_FRAME_RATE`] th call. It must be a rate the
/// frontend can pace on its own: RetroArch derives the automatic swap
/// interval from this value and caps it at 4, so reporting the 10 Hz
/// guest rate made silent games run at the display refresh rate
/// (issue #43). 60 Hz matches the common display refresh rates exactly,
/// and the audio stream always flows at this rate (silence-padded while
/// the guest is quiet), so audio-sync pacing keeps the speed exact
/// everywhere else.
const PACING_FPS: u32 = 60;
/// Stereo frames submitted to the frontend per `retro_run` call. The
/// guest produces `AUDIO_SAMPLE_RATE / GUEST_FRAME_RATE` frames per
/// tick, so the constant flow averages to the same 44.1 kHz stream.
const PACING_SAMPLES_PER_RUN: usize = (AUDIO_SAMPLE_RATE / PACING_FPS) as usize;
const PERFORMANCE_LEVEL: u32 = 3;
/// Optional user orientation overrides looked up in the frontend system
/// directory, loaded once per process before content loading.
const ORIENTATION_PROFILE_FILE: &str = "nicaiemu_orientation.csv";

/// Converts frontend frames into guest screen ticks.
///
/// Each `retro_run` call contributes `GUEST_FRAME_RATE / PACING_FPS` of
/// a guest tick; the tick runs when the accumulated credit reaches a
/// whole tick. This keeps the guest rate exact at
/// `GUEST_FRAME_RATE` ticks per second regardless of how often the
/// frontend paces the core.
struct GuestTickPacer {
    /// Outstanding credit in 1/PACING_FPS frontend-frame units.
    credit: u32,
}

impl GuestTickPacer {
    fn new() -> Self {
        Self { credit: 0 }
    }

    /// Record one frontend frame and return the due guest tick count.
    fn advance(&mut self) -> u32 {
        self.credit += GUEST_FRAME_RATE;
        let mut ticks = 0;
        while self.credit >= PACING_FPS {
            self.credit -= PACING_FPS;
            ticks += 1;
        }
        ticks
    }
}

/// Loaded emulator state shared by the libretro entry points.
struct Emulator {
    archive: CbeArchive,
    machine: NicaiMachine,
    instruction_limit: u64,
    stopped: bool,
    touch_input: bool,
    content_crc32: u32,
    /// Display size last reported to the frontend, to detect geometry changes
    /// from a live orientation override.
    presented_size: (u32, u32),
    /// Guest tick credit accumulated from the frontend pacing rate.
    tick_pacer: GuestTickPacer,
}

/// Global emulator instance.
static mut EMULATOR: Option<Emulator> = None;

/// Presented display size of the loaded content.
///
/// Portrait titles present the native 240x400 framebuffer; landscape titles
/// resolved by the content-identity orientation profile present it rotated to
/// 400x240. Falls back to the portrait display when no content is loaded.
fn display_size() -> (u32, u32) {
    unsafe {
        EMULATOR
            .as_ref()
            .map(|emulator| emulator.machine.display_size())
            .unwrap_or((DISPLAY_WIDTH, DISPLAY_HEIGHT))
    }
}

// ============================================================
// Callback registration
// ============================================================

#[no_mangle]
pub extern "C" fn retro_set_environment(cb: retro_environment_t) {
    callbacks::set_environment(cb);
    // Declare core options as early as possible so the frontend can show them
    // before any content is loaded.
    options::set_core_options();
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
    let (base_width, base_height) = display_size();
    unsafe {
        (*info) = retro_system_av_info {
            geometry: retro_game_geometry {
                base_width,
                base_height,
                max_width: MAX_DISPLAY_WIDTH,
                max_height: MAX_DISPLAY_HEIGHT,
                aspect_ratio: base_width as f32 / base_height as f32,
            },
            timing: retro_system_timing {
                fps: PACING_FPS as f64,
                sample_rate: AUDIO_SAMPLE_RATE as f64,
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

        // User orientation entries must be registered before the machine is
        // created, which is where the automatic profile is resolved.
        load_user_orientation_profile();

        match load_machine(Path::new(path)) {
            Ok((archive, mut machine)) => {
                if let Err(error) = machine.boot(DEFAULT_INSTRUCTION_LIMIT) {
                    log::error!("Failed to boot CBE application: {error:#}");
                    return false;
                }
                log::info!("Game loaded: {path}");
                let content_crc32 = crc32fast::hash(archive.bytes());
                let presented_size = machine.display_size();
                EMULATOR = Some(Emulator {
                    archive,
                    machine,
                    instruction_limit: DEFAULT_INSTRUCTION_LIMIT,
                    stopped: false,
                    touch_input: true,
                    content_crc32,
                    presented_size,
                    tick_pacer: GuestTickPacer::new(),
                });
                if let Some(emulator) = EMULATOR.as_mut() {
                    apply_core_options(emulator);
                    emulator.presented_size = emulator.machine.display_size();
                    register_memory_maps(emulator);
                }
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

        if options::core_options_changed() {
            apply_core_options(emulator);
        }

        // A live orientation override swaps the presented geometry; tell the
        // frontend so it resizes instead of stretching the old viewport.
        let (display_width, display_height) = emulator.machine.display_size();
        if (display_width, display_height) != emulator.presented_size {
            notify_display_geometry(display_width, display_height);
            emulator.presented_size = (display_width, display_height);
        }

        callbacks::input_poll();
        update_phone_keys(emulator);
        update_pointer(emulator);

        if !emulator.stopped {
            for _ in 0..emulator.tick_pacer.advance() {
                if let Err(error) = emulator.machine.run_frame(emulator.instruction_limit) {
                    log::warn!("CBE frame callback stopped: {error:#}");
                    emulator.stopped = true;
                    break;
                }
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
            display_width,
            display_height,
            (display_width * 4) as usize,
        );

        // Keep the sample flow at exactly the reported pacing rate: the
        // frontend paces the core by audio sync, so a silent guest must
        // still submit silence or the frontend would fall back to
        // display-rate pacing and the game would run too fast (issue #43).
        let mut samples = emulator.machine.take_audio_samples(PACING_SAMPLES_PER_RUN);
        samples.resize(PACING_SAMPLES_PER_RUN * 2, 0);
        callbacks::audio_sample_batch(samples.as_ptr(), PACING_SAMPLES_PER_RUN);
    }
}

fn load_machine(path: &Path) -> anyhow::Result<(CbeArchive, NicaiMachine)> {
    let archive = CbeArchive::load(path)?;
    let machine = NicaiMachine::new(&archive)?;
    Ok((archive, machine))
}

/// Load the optional user orientation profile once per process.
///
/// The file is `<system_dir>/nicaiemu_orientation.csv`; a missing or invalid
/// file is non-fatal because the built-in profile still applies.
fn load_user_orientation_profile() {
    static LOADED: AtomicBool = AtomicBool::new(false);
    if LOADED.swap(true, Ordering::Relaxed) {
        return;
    }
    let mut system_dir: *const c_char = ptr::null();
    let ok = callbacks::environment(
        RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY,
        &mut system_dir as *mut _ as *mut c_void,
    );
    if !ok || system_dir.is_null() {
        log::info!("No frontend system directory; user orientation profile not loaded");
        return;
    }
    let Ok(dir) = unsafe { CStr::from_ptr(system_dir) }.to_str() else {
        log::warn!("Frontend system directory is not valid UTF-8");
        return;
    };
    if dir.is_empty() {
        log::info!("Frontend system directory is empty; user orientation profile not loaded");
        return;
    }
    let path = Path::new(dir).join(ORIENTATION_PROFILE_FILE);
    match load_orientation_overrides(&path) {
        Ok(count) => log::info!(
            "Loaded {count} user orientation entries from {}",
            path.display()
        ),
        Err(error) => log::info!("No user orientation profile applied: {error:#}"),
    }
}

fn input_descriptors() -> [retro_input_descriptor; 15] {
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
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_SELECT,
            description: c"Star (*) Key".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_L,
            description: c"Left Soft Key".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_R,
            description: c"Right Soft Key".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_L2,
            description: c"Star (*) Key".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_R2,
            description: c"Hash (#) Key".as_ptr(),
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

fn register_memory_maps(emulator: &mut Emulator) {
    let regions = emulator.machine.memory_regions();
    let mut descriptors: Vec<retro_memory_descriptor> = Vec::with_capacity(regions.len() + 1);
    for region in &regions {
        let Some(ptr) = emulator.machine.memory_pointer(region.base) else {
            continue;
        };
        let mut flags = 0u64;
        if region.base == NicaiMachine::system_ram_base() {
            flags |= RETRO_MEMDESC_SYSTEM_RAM;
        }
        if region.base == NicaiMachine::video_ram_base() {
            flags |= RETRO_MEMDESC_VIDEO_RAM;
        }
        descriptors.push(retro_memory_descriptor {
            flags,
            ptr: ptr.cast(),
            offset: 0,
            start: region.base as usize,
            select: 0,
            disconnect: 0,
            len: region.size,
            addrspace: c"Nicai".as_ptr(),
        });
    }
    descriptors.push(retro_memory_descriptor {
        flags: 0,
        ptr: ptr::null_mut(),
        offset: 0,
        start: 0,
        select: 0,
        disconnect: 0,
        len: 0,
        addrspace: ptr::null(),
    });

    let map = retro_memory_map {
        descriptors: descriptors.as_ptr(),
        num_descriptors: descriptors.len() as u32,
    };
    if callbacks::environment(
        RETRO_ENVIRONMENT_SET_MEMORY_MAPS,
        &map as *const _ as *mut c_void,
    ) {
        log::info!("Registered guest memory descriptors");
    } else {
        log::warn!("Frontend did not accept guest memory descriptors");
    }
}

/// Apply the frontend core option selections to the running emulator.
fn apply_core_options(emulator: &mut Emulator) {
    let options = options::read_core_options(options::get_core_option);
    emulator.machine.set_volume(options.volume);
    emulator.touch_input = options.touch_input;
    emulator.machine.set_auto_bgm(options.auto_bgm);
    emulator.machine.set_orientation(options.orientation);
    emulator.machine.resolve_auto_orientation(&emulator.archive);
    super::logger::set_debug_logging(options.debug_logging);
    log::info!(
        "Core options applied: volume={} touch_input={} auto_bgm={} debug_logging={} orientation={:?}",
        options.volume,
        options.touch_input,
        options.auto_bgm,
        options.debug_logging,
        options.orientation
    );
}

/// Report a changed presented geometry (from a live orientation override) so
/// the frontend resizes instead of stretching the previous viewport.
fn notify_display_geometry(width: u32, height: u32) {
    let geometry = retro_game_geometry {
        base_width: width,
        base_height: height,
        max_width: MAX_DISPLAY_WIDTH,
        max_height: MAX_DISPLAY_HEIGHT,
        aspect_ratio: width as f32 / height as f32,
    };
    if callbacks::environment(
        RETRO_ENVIRONMENT_SET_GEOMETRY,
        &geometry as *const _ as *mut c_void,
    ) {
        log::info!("Frontend accepted display geometry {width}x{height}");
    } else {
        log::warn!("Frontend did not accept display geometry {width}x{height}");
    }
}

/// Analog stick deflection (of the 0x7fff full range) required before a
/// direction counts, matching the standalone gamepad mapper's 0.5 deadzone.
const ANALOG_DEADZONE: i16 = 0x4000;

/// Map RetroPad buttons and analog sticks onto phone keypad ABI key states.
///
/// Returns one entry per guest key so every key is written each poll and
/// releases propagate. Pure so the shared RetroPad layout can be unit-tested
/// without a frontend; kept in lockstep with the standalone `GamepadMapper`.
fn retro_pad_key_states(
    is_pressed: impl Fn(u32) -> bool,
    analog: impl Fn(u32, u32) -> i16,
) -> [(u8, bool); 9] {
    // libretro analog axes report +Y downwards; the guest D-pad does not.
    let stick_up = analog(RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_Y)
        < -ANALOG_DEADZONE
        || analog(RETRO_DEVICE_INDEX_ANALOG_RIGHT, RETRO_DEVICE_ID_ANALOG_Y) < -ANALOG_DEADZONE;
    let stick_down = analog(RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_Y)
        > ANALOG_DEADZONE
        || analog(RETRO_DEVICE_INDEX_ANALOG_RIGHT, RETRO_DEVICE_ID_ANALOG_Y) > ANALOG_DEADZONE;
    let stick_left = analog(RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_X)
        < -ANALOG_DEADZONE
        || analog(RETRO_DEVICE_INDEX_ANALOG_RIGHT, RETRO_DEVICE_ID_ANALOG_X) < -ANALOG_DEADZONE;
    let stick_right = analog(RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_ID_ANALOG_X)
        > ANALOG_DEADZONE
        || analog(RETRO_DEVICE_INDEX_ANALOG_RIGHT, RETRO_DEVICE_ID_ANALOG_X) > ANALOG_DEADZONE;

    let up = is_pressed(RETRO_DEVICE_ID_JOYPAD_UP) || stick_up;
    let down = is_pressed(RETRO_DEVICE_ID_JOYPAD_DOWN) || stick_down;
    let left = is_pressed(RETRO_DEVICE_ID_JOYPAD_LEFT) || stick_left;
    let right = is_pressed(RETRO_DEVICE_ID_JOYPAD_RIGHT) || stick_right;
    // Confirm on any of the three natural face/start buttons.
    let confirm = is_pressed(RETRO_DEVICE_ID_JOYPAD_A)
        || is_pressed(RETRO_DEVICE_ID_JOYPAD_B)
        || is_pressed(RETRO_DEVICE_ID_JOYPAD_START);
    // X plus L duplicate the left soft key; Y plus R duplicate the right.
    let soft_left = is_pressed(RETRO_DEVICE_ID_JOYPAD_X) || is_pressed(RETRO_DEVICE_ID_JOYPAD_L);
    let soft_right = is_pressed(RETRO_DEVICE_ID_JOYPAD_Y) || is_pressed(RETRO_DEVICE_ID_JOYPAD_R);
    // Select plus L2 fire the `*` key (guest 19); R2 fires the `#` key (20).
    let star = is_pressed(RETRO_DEVICE_ID_JOYPAD_SELECT) || is_pressed(RETRO_DEVICE_ID_JOYPAD_L2);
    let hash = is_pressed(RETRO_DEVICE_ID_JOYPAD_R2);

    [
        (12, soft_left),
        (13, soft_right),
        (14, confirm),
        (15, left),
        (16, right),
        (17, up),
        (18, down),
        (19, star),
        (20, hash),
    ]
}

/// Map RetroPad buttons to phone keypad ABI key codes.
fn update_phone_keys(emulator: &mut Emulator) {
    let joypad = |id: u32| callbacks::input_state(0, RETRO_DEVICE_JOYPAD, 0, id) != 0;
    let analog = |index: u32, id: u32| callbacks::input_state(0, RETRO_DEVICE_ANALOG, index, id);
    for (key, pressed) in retro_pad_key_states(joypad, analog) {
        emulator.machine.set_key(key, pressed);
    }
}

/// Map RetroArch pointer coordinates (-0x7fff..0x7fff) to the displayed screen.
fn pointer_to_screen(value: i32, screen_size: i32) -> i32 {
    let numerator = (value.saturating_add(0x7fff)) as i64 * screen_size as i64 + 0x7fff;
    (numerator / 0xFFFF).clamp(0, (screen_size - 1).max(0) as i64) as i32
}

/// Poll the libretro pointer device (mouse or touchscreen) into the machine.
///
/// Pointer coordinates arrive in the presented display space and are mapped
/// back to guest framebuffer coordinates so taps stay correct on the rotated
/// 400x240 landscape output.
fn update_pointer(emulator: &mut Emulator) {
    if !emulator.touch_input {
        return;
    }
    let x = callbacks::input_state(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_X);
    let y = callbacks::input_state(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_Y);
    let pressed =
        callbacks::input_state(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_PRESSED) != 0;
    if x != 0 || y != 0 || pressed {
        let (display_width, display_height) = emulator.machine.display_size();
        let display_x = pointer_to_screen(x as i32, display_width as i32);
        let display_y = pointer_to_screen(y as i32, display_height as i32);
        let (frame_x, frame_y) = emulator
            .machine
            .display_to_framebuffer(display_x, display_y);
        emulator.machine.set_pointer(frame_x, frame_y, pressed);
    }
}

#[no_mangle]
pub extern "C" fn retro_reset() {
    unsafe {
        let Some(emulator) = EMULATOR.as_mut() else {
            log::warn!("retro_reset called before loading a game");
            return;
        };
        match emulator
            .machine
            .reset(&emulator.archive, emulator.instruction_limit)
        {
            Ok(()) => {
                emulator.stopped = false;
                apply_core_options(emulator);
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
    unsafe {
        if EMULATOR.is_none() {
            0
        } else {
            SERIALIZED_SIZE
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_serialize(data: *mut c_void, size: usize) -> bool {
    unsafe {
        let Some(emulator) = EMULATOR.as_ref() else {
            return false;
        };
        if data.is_null() || size < SERIALIZED_SIZE {
            return false;
        }
        let buffer = std::slice::from_raw_parts_mut(data as *mut u8, SERIALIZED_SIZE);
        match encode_machine(&emulator.machine, emulator.content_crc32, buffer) {
            Ok(()) => true,
            Err(error) => {
                log::error!("Failed to serialize game state: {error:#}");
                false
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_unserialize(data: *const c_void, size: usize) -> bool {
    unsafe {
        let Some(emulator) = EMULATOR.as_mut() else {
            return false;
        };
        if data.is_null() || size == 0 {
            return false;
        }
        let buffer = std::slice::from_raw_parts(data as *const u8, size);
        match decode_machine(buffer, emulator.content_crc32) {
            Ok(machine) => {
                emulator.machine = machine;
                emulator.stopped = false;
                // apply_core_options re-resolves the display orientation: it is
                // presentation state skipped by the save-state codec.
                apply_core_options(emulator);
                emulator.presented_size = emulator.machine.display_size();
                log::info!("Game state restored");
                true
            }
            Err(error) => {
                log::error!("Failed to restore game state: {error:#}");
                false
            }
        }
    }
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
pub extern "C" fn retro_get_memory_data(id: u32) -> *mut c_void {
    unsafe {
        let Some(emulator) = EMULATOR.as_mut() else {
            return ptr::null_mut();
        };
        match id & RETRO_MEMORY_MASK {
            RETRO_MEMORY_SYSTEM_RAM => emulator
                .machine
                .system_ram_pointer()
                .unwrap_or(ptr::null_mut())
                .cast(),
            RETRO_MEMORY_VIDEO_RAM => emulator
                .machine
                .video_ram_pointer()
                .unwrap_or(ptr::null_mut())
                .cast(),
            _ => ptr::null_mut(),
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_get_memory_size(id: u32) -> usize {
    unsafe {
        if EMULATOR.is_none() {
            return 0;
        }
        match id & RETRO_MEMORY_MASK {
            RETRO_MEMORY_SYSTEM_RAM => NicaiMachine::system_ram_size(),
            RETRO_MEMORY_VIDEO_RAM => NicaiMachine::video_ram_size(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::sync::Mutex;

    /// Captured video frame: width, height, pitch, and raw XRGB8888 bytes.
    type VideoFrame = (u32, u32, usize, Vec<u8>);

    static LAST_VIDEO: Mutex<Option<VideoFrame>> = Mutex::new(None);
    static AUDIO_FRAMES: Mutex<usize> = Mutex::new(0);
    /// Stereo frames received with nonzero content. The core pads silent
    /// stretches with zeros to keep the sample flow constant, so guest
    /// audio delivery must be judged on nonzero samples.
    static AUDIO_NONZERO_FRAMES: Mutex<usize> = Mutex::new(0);
    /// Serializes the real-content tests: the libretro core is a process-wide
    /// singleton, so concurrent content loads would clobber each other.
    static CONTENT_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" fn test_environment(cmd: u32, _data: *mut c_void) -> bool {
        matches!(
            cmd,
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT
                | RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS
                | RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL
                | RETRO_ENVIRONMENT_SET_MEMORY_MAPS
                | RETRO_ENVIRONMENT_SET_VARIABLES
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

    unsafe extern "C" fn test_audio_batch(data: *const i16, frames: usize) -> usize {
        *AUDIO_FRAMES.lock().unwrap() += frames;
        if !data.is_null() {
            let samples = std::slice::from_raw_parts(data, frames * 2);
            let nonzero = samples.iter().filter(|&&sample| sample != 0).count();
            *AUDIO_NONZERO_FRAMES.lock().unwrap() += nonzero;
        }
        frames
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
    fn av_info_defaults_to_portrait_display_without_content() {
        let mut av = std::mem::MaybeUninit::<retro_system_av_info>::zeroed();
        retro_get_system_av_info(av.as_mut_ptr());
        let av = unsafe { av.assume_init() };
        assert_eq!(av.geometry.base_width, DISPLAY_WIDTH);
        assert_eq!(av.geometry.base_height, DISPLAY_HEIGHT);
        // The bounds must also fit the rotated landscape output.
        assert_eq!(av.geometry.max_width, MAX_DISPLAY_WIDTH);
        assert_eq!(av.geometry.max_height, MAX_DISPLAY_HEIGHT);
        assert!(
            (av.geometry.aspect_ratio - DISPLAY_WIDTH as f32 / DISPLAY_HEIGHT as f32).abs() < 1e-6
        );
        assert_eq!(av.timing.fps, PACING_FPS as f64);
        assert_eq!(av.timing.sample_rate, AUDIO_SAMPLE_RATE as f64);
    }

    /// Regression for issue #43: the guest must advance its 10 Hz screen
    /// tick on every sixth frontend frame while the core reports a 60 Hz
    /// pacing rate the frontend can actually pace (RetroArch caps the
    /// automatic swap interval at 4, so a reported 10 Hz rate made silent
    /// games run at the display refresh rate).
    #[test]
    fn pacer_advances_guest_ticks_at_the_guest_frame_rate() {
        let mut pacer = GuestTickPacer::new();
        let pattern: Vec<u32> = (0..12).map(|_| pacer.advance()).collect();
        assert_eq!(pattern, [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1]);

        let mut pacer = GuestTickPacer::new();
        let ticks: u32 = (0..600).map(|_| pacer.advance()).sum();
        assert_eq!(ticks, 100);
    }

    #[test]
    fn input_descriptors_cover_retropad_mapping() {
        let descriptors = input_descriptors();
        let ids: Vec<u32> = descriptors[..14]
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
                RETRO_DEVICE_ID_JOYPAD_SELECT,
                RETRO_DEVICE_ID_JOYPAD_L,
                RETRO_DEVICE_ID_JOYPAD_R,
                RETRO_DEVICE_ID_JOYPAD_L2,
                RETRO_DEVICE_ID_JOYPAD_R2,
            ]
        );
        assert_eq!(descriptors[14].device, RETRO_DEVICE_NONE);
        assert!(descriptors[14].description.is_null());
    }

    /// The shared RetroPad layout must expose every non-digit guest key:
    /// soft keys 12/13, confirm 14, D-pad 15-18, and the star/hash keys 19/20.
    #[test]
    fn retro_pad_mapping_reaches_every_non_digit_guest_key() {
        let none = |_id: u32| false;
        let zero = |_index: u32, _id: u32| 0;
        let mut reached = std::collections::HashSet::new();
        for button in [
            RETRO_DEVICE_ID_JOYPAD_UP,
            RETRO_DEVICE_ID_JOYPAD_DOWN,
            RETRO_DEVICE_ID_JOYPAD_LEFT,
            RETRO_DEVICE_ID_JOYPAD_RIGHT,
            RETRO_DEVICE_ID_JOYPAD_A,
            RETRO_DEVICE_ID_JOYPAD_B,
            RETRO_DEVICE_ID_JOYPAD_X,
            RETRO_DEVICE_ID_JOYPAD_Y,
            RETRO_DEVICE_ID_JOYPAD_START,
            RETRO_DEVICE_ID_JOYPAD_SELECT,
            RETRO_DEVICE_ID_JOYPAD_L,
            RETRO_DEVICE_ID_JOYPAD_R,
            RETRO_DEVICE_ID_JOYPAD_L2,
            RETRO_DEVICE_ID_JOYPAD_R2,
        ] {
            let states = retro_pad_key_states(|id| id == button, zero);
            for (key, pressed) in states {
                if pressed {
                    reached.insert(key);
                }
            }
        }
        assert_eq!(
            reached,
            [12u8, 13, 14, 15, 16, 17, 18, 19, 20].into_iter().collect()
        );
    }

    #[test]
    fn retro_pad_mapping_matches_the_shared_layout() {
        let press = |id: u32| id == RETRO_DEVICE_ID_JOYPAD_SELECT;
        let states = retro_pad_key_states(press, |_index, _id| 0);
        // Select fires the star key (19) and nothing else.
        assert_eq!(
            states
                .iter()
                .copied()
                .filter(|(_, pressed)| *pressed)
                .collect::<Vec<_>>(),
            [(19, true)]
        );

        let press = |id: u32| id == RETRO_DEVICE_ID_JOYPAD_R2;
        let states = retro_pad_key_states(press, |_index, _id| 0);
        assert_eq!(
            states
                .iter()
                .copied()
                .filter(|(_, pressed)| *pressed)
                .collect::<Vec<_>>(),
            [(20, true)]
        );

        // Shoulders duplicate the soft keys, matching the standalone mapper.
        let press = |id: u32| id == RETRO_DEVICE_ID_JOYPAD_L;
        let states = retro_pad_key_states(press, |_index, _id| 0);
        assert_eq!(
            states
                .iter()
                .copied()
                .filter(|(_, pressed)| *pressed)
                .collect::<Vec<_>>(),
            [(12, true)]
        );
    }

    #[test]
    fn retro_pad_analog_sticks_drive_directions() {
        let none = |_id: u32| false;
        // libretro +Y is downwards: full up deflection reaches only guest 17.
        let states = retro_pad_key_states(none, |index, id| {
            if index == RETRO_DEVICE_INDEX_ANALOG_LEFT && id == RETRO_DEVICE_ID_ANALOG_Y {
                -i16::MAX
            } else {
                0
            }
        });
        assert_eq!(
            states
                .iter()
                .copied()
                .filter(|(_, pressed)| *pressed)
                .collect::<Vec<_>>(),
            [(17, true)]
        );

        // Full +X deflection reaches only guest 16 (right).
        let states = retro_pad_key_states(none, |index, id| {
            if index == RETRO_DEVICE_INDEX_ANALOG_RIGHT && id == RETRO_DEVICE_ID_ANALOG_X {
                i16::MAX
            } else {
                0
            }
        });
        assert_eq!(
            states
                .iter()
                .copied()
                .filter(|(_, pressed)| *pressed)
                .collect::<Vec<_>>(),
            [(16, true)]
        );

        // Values inside the deadzone report nothing.
        let states = retro_pad_key_states(none, |_index, _id| ANALOG_DEADZONE - 1);
        assert!(states.iter().all(|(_, pressed)| !pressed));
    }

    #[test]
    fn pointer_coordinates_map_to_the_screen() {
        assert_eq!(pointer_to_screen(0, 240), 120);
        assert_eq!(pointer_to_screen(-0x7fff, 240), 0);
        assert_eq!(pointer_to_screen(0x7fff, 240), 239);
        assert_eq!(pointer_to_screen(0, 400), 200);
        assert_eq!(pointer_to_screen(0x7fff, 400), 399);
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
        assert!(core_info.contains("savestate = \"true\""));
        assert!(core_info.contains("cheats = \"false\""));
        assert!(core_info.contains("input_descriptors = \"true\""));
        assert!(core_info.contains("memory_descriptors = \"true\""));
        assert!(core_info.contains("core_options = \"true\""));
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_boots_and_renders_frames() {
        let _content_lock = CONTENT_LOCK.lock().unwrap();
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("激情砖块.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());
        let game_path = CString::new(game_path.to_string_lossy().as_bytes()).unwrap();

        retro_set_environment(test_environment);
        retro_set_video_refresh(test_video_refresh);
        retro_set_audio_sample_batch(test_audio_batch);
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
        assert_eq!(
            retro_get_memory_size(RETRO_MEMORY_SYSTEM_RAM),
            NicaiMachine::system_ram_size()
        );
        assert_eq!(
            retro_get_memory_size(RETRO_MEMORY_VIDEO_RAM),
            NicaiMachine::video_ram_size()
        );
        assert!(!retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM).is_null());
        assert!(!retro_get_memory_data(RETRO_MEMORY_VIDEO_RAM).is_null());

        for _ in 0..30 {
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

        let state_size = retro_serialize_size();
        assert_eq!(state_size, SERIALIZED_SIZE);
        let mut state = vec![0u8; state_size];
        assert!(retro_serialize(state.as_mut_ptr().cast(), state.len()));
        assert!(retro_unserialize(state.as_ptr().cast(), state.len()));
        for _ in 0..5 {
            retro_run();
        }
        assert!(LAST_VIDEO.lock().unwrap().is_some());
        assert!(
            *AUDIO_NONZERO_FRAMES.lock().unwrap() > 0,
            "guest audio never reached the libretro sample callback"
        );

        retro_unload_game();
        assert!(retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM).is_null());
        assert_eq!(retro_get_memory_size(RETRO_MEMORY_SYSTEM_RAM), 0);
        retro_deinit();
    }

    /// Regression for issue #40: landscape titles present rotated 400x240
    /// frames, so the core must report the swapped geometry and matching
    /// frame dimensions instead of the fixed portrait 240x400 layout.
    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_landscape_content_presents_rotated_frames() {
        let _content_lock = CONTENT_LOCK.lock().unwrap();
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("三国群殴传.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());
        let game_path = CString::new(game_path.to_string_lossy().as_bytes()).unwrap();

        retro_set_environment(test_environment);
        retro_set_video_refresh(test_video_refresh);
        retro_set_audio_sample_batch(test_audio_batch);
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

        let mut av = std::mem::MaybeUninit::<retro_system_av_info>::zeroed();
        retro_get_system_av_info(av.as_mut_ptr());
        let av = unsafe { av.assume_init() };
        assert_eq!(av.geometry.base_width, 400);
        assert_eq!(av.geometry.base_height, 240);
        assert_eq!(av.geometry.max_width, MAX_DISPLAY_WIDTH);
        assert_eq!(av.geometry.max_height, MAX_DISPLAY_HEIGHT);
        assert!((av.geometry.aspect_ratio - 400.0 / 240.0).abs() < 1e-6);

        *LAST_VIDEO.lock().unwrap() = None;
        for _ in 0..30 {
            retro_run();
        }
        {
            let last = LAST_VIDEO.lock().unwrap();
            let (width, height, pitch, bytes) =
                last.as_ref().expect("video refresh was never called");
            assert_eq!(*width, 400);
            assert_eq!(*height, 240);
            assert_eq!(*pitch, (400 * 4) as usize);
            assert_eq!(bytes.len(), (240 * 400 * 4) as usize);
        }

        // The orientation is presentation state skipped by the save-state codec;
        // restoring a save state must keep presenting rotated frames.
        let state_size = retro_serialize_size();
        assert_eq!(state_size, SERIALIZED_SIZE);
        let mut state = vec![0u8; state_size];
        assert!(retro_serialize(state.as_mut_ptr().cast(), state.len()));
        assert!(retro_unserialize(state.as_ptr().cast(), state.len()));
        *LAST_VIDEO.lock().unwrap() = None;
        for _ in 0..5 {
            retro_run();
        }
        let last = LAST_VIDEO.lock().unwrap();
        let (width, height, _, _) = last.as_ref().expect("video refresh was never called");
        assert_eq!(*width, 400);
        assert_eq!(*height, 240);

        retro_unload_game();
        retro_deinit();
    }

    /// Regression for issue #43: the frontend paces `retro_run` at the
    /// reported 60 Hz rate, so sixty calls must advance exactly ten guest
    /// screen ticks while the sample callback keeps receiving a constant
    /// 735-frame flow (silence included) for audio-sync pacing.
    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_content_guest_ticks_track_the_pacing_rate() {
        let _content_lock = CONTENT_LOCK.lock().unwrap();
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = std::path::PathBuf::from(game_dir).join("孤岛.CBE");
        assert!(game_path.is_file(), "missing {}", game_path.display());
        let game_path = CString::new(game_path.to_string_lossy().as_bytes()).unwrap();

        retro_set_environment(test_environment);
        retro_set_video_refresh(test_video_refresh);
        retro_set_audio_sample_batch(test_audio_batch);
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

        *AUDIO_FRAMES.lock().unwrap() = 0;
        let ticks_before = unsafe { EMULATOR.as_ref().unwrap().machine.frame_count() };
        for _ in 0..60 {
            retro_run();
        }
        let ticks_after = unsafe { EMULATOR.as_ref().unwrap().machine.frame_count() };
        assert_eq!(ticks_after - ticks_before, 10);
        assert_eq!(
            *AUDIO_FRAMES.lock().unwrap(),
            60 * PACING_SAMPLES_PER_RUN,
            "sample flow must stay at the reported pacing rate"
        );
        assert!(LAST_VIDEO.lock().unwrap().is_some());

        retro_unload_game();
        retro_deinit();
    }
}
