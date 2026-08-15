//! NicaiEmu desktop frontend.

mod standalone;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use clap::Parser;
use log::{info, warn};
use minifb::{Key, Window, WindowOptions};
use nicaiemu_core::{
    decode_machine, encode_machine, CbeArchive, NicaiMachine, AUDIO_SAMPLE_RATE, GUEST_FRAME_RATE,
    SERIALIZED_SIZE,
};
use standalone::gamepad_overlay::GamepadOverlay;
use standalone::input::{KeyboardMapper, RemapSpec};
use standalone::scaler::{DisplayScaler, ScaleFilter};

#[derive(Parser)]
#[command(name = "nicaiemu")]
#[command(about = "A desktop emulator for Nicai/MStar CBE games")]
#[command(version)]
struct Cli {
    /// Path to the CBE executable.
    #[arg(value_name = "GAME_PATH")]
    file: PathBuf,

    /// List packaged resources and exit.
    #[arg(short, long)]
    list: bool,

    /// Initial window width.
    #[arg(short, long, default_value_t = 480)]
    width: usize,

    /// Initial window height.
    #[arg(short = 'H', long, default_value_t = 800)]
    height: usize,

    /// Pixel scaling filter for display output.
    #[arg(long, value_enum, default_value_t = ScaleFilter::Nearest)]
    filter: ScaleFilter,

    /// Remap a guest key in GUEST_KEY:KEY format.
    #[arg(long = "remap", value_name = "GUEST_KEY:KEY")]
    remappings: Vec<RemapSpec>,

    /// Show a virtual gamepad overlay over the game frame.
    #[arg(long)]
    show_gamepad: bool,

    /// Run in fullscreen mode.
    #[arg(long)]
    fullscreen: bool,

    /// Audio volume (0-100).
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(0..=100))]
    volume: u32,

    /// Play the first packaged MIDI resource as background music when the
    /// game never issues audio-manager calls of its own.
    #[arg(long)]
    auto_bgm: bool,

    /// Run without opening a window.
    #[arg(long)]
    headless: bool,

    /// Number of frames to run in headless mode.
    #[arg(long, default_value_t = 60)]
    frames: u32,

    /// Maximum guest instructions per callback.
    #[arg(long, default_value_t = 5_000_000)]
    instruction_limit: u64,

    /// Take a PNG screenshot after running headlessly, then exit.
    #[arg(short = 'S', long, value_name = "PATH")]
    screenshot: Option<PathBuf>,

    /// Number of frames to run before taking a screenshot.
    #[arg(long, default_value_t = 30)]
    screenshot_frames: u32,

    /// Write a save state to this path when the emulator exits.
    #[arg(long, value_name = "PATH")]
    save_state: Option<PathBuf>,

    /// Load a save state from this path before running.
    #[arg(long, value_name = "PATH")]
    load_state: Option<PathBuf>,

    /// Enable verbose logging.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let log_level = if cli.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    env_logger::Builder::new().filter_level(log_level).init();

    let archive = CbeArchive::load(&cli.file)
        .with_context(|| format!("failed to load CBE file: {}", cli.file.display()))?;
    info!("{}", archive.summary());
    if cli.list {
        for (index, resource) in archive.resources().iter().enumerate() {
            println!(
                "{:3}. {} (offset=0x{:X}, size={})",
                index + 1,
                resource.name,
                resource.offset,
                resource.size
            );
        }
        return Ok(());
    }

    let mut machine;
    let mut boot_result = Ok(());
    if let Some(path) = &cli.load_state {
        machine = load_machine_state(&archive, path)?;
    } else {
        machine = NicaiMachine::new(&archive).context("failed to create CBE machine")?;
        boot_result = machine.boot(cli.instruction_limit);
    }
    machine.set_volume(cli.volume);
    machine.set_auto_bgm(cli.auto_bgm);

    if let Some(path) = &cli.screenshot {
        capture_screenshot(
            &mut machine,
            boot_result.err(),
            cli.screenshot_frames,
            cli.instruction_limit,
            path,
        )?;
        write_save_state(&machine, &archive, cli.save_state.as_deref())?;
        return Ok(());
    }

    if cli.headless {
        boot_result.context("failed to initialize CBE application")?;
        for frame in 0..cli.frames {
            machine
                .run_frame(cli.instruction_limit)
                .with_context(|| format!("guest screen callback stopped at frame {}", frame + 1))?;
        }
        info!("Headless run completed: {} frames", cli.frames);
        write_save_state(&machine, &archive, cli.save_state.as_deref())?;
        return Ok(());
    }

    boot_result.context("failed to initialize CBE application")?;

    let game_name = cli
        .file
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("CBE Game");
    let title = format!("NicaiEmu - {game_name}");
    let mut window = Window::new(
        &title,
        cli.width,
        cli.height,
        WindowOptions {
            resize: !cli.fullscreen,
            borderless: cli.fullscreen,
            scale_mode: minifb::ScaleMode::Stretch,
            ..WindowOptions::default()
        },
    )
    .context("failed to create emulator window")?;
    if cli.fullscreen {
        window.topmost(true);
        window.set_position(0, 0);
    }
    window.set_target_fps(GUEST_FRAME_RATE as usize);
    let audio_output = StandaloneAudio::try_new();
    if audio_output.is_none() {
        warn!("Audio output unavailable; running without sound");
    }

    info!("Controls: arrows/WASD move, Enter/F confirms, Q/E soft keys, R resets, Esc exits");
    let mut display_scaler = DisplayScaler::new(cli.filter);
    let keyboard = KeyboardMapper::new(&cli.remappings);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        keyboard.apply(&window, &mut machine);
        if let Some((mouse_x, mouse_y)) = window.get_mouse_pos(minifb::MouseMode::Clamp) {
            let x = (mouse_x * 240.0 / cli.width as f32) as i32;
            let y = (mouse_y * 400.0 / cli.height as f32) as i32;
            let down = window.get_mouse_down(minifb::MouseButton::Left);
            machine.set_pointer(x, y, down);
        }
        if window.is_key_pressed(Key::R, minifb::KeyRepeat::No) {
            if let Some(audio) = &audio_output {
                audio.clear();
            }
            machine
                .reset(&archive, cli.instruction_limit)
                .context("failed to reset CBE application")?;
            info!("Game reset");
        }
        machine
            .run_frame(cli.instruction_limit)
            .context("guest screen callback failed")?;
        if let Some(audio) = &audio_output {
            audio.push(machine.take_audio_samples((AUDIO_SAMPLE_RATE / GUEST_FRAME_RATE) as usize));
        }
        let mut pixels = machine.frame_pixels();
        if cli.show_gamepad {
            GamepadOverlay::draw(&mut pixels, 240, 400, machine.held_keys());
        }
        let (window_width, window_height) = window.get_size();
        let buffer = display_scaler.render(&pixels, 240, 400, window_width, window_height);
        window
            .update_with_buffer(buffer, window_width.max(1), window_height.max(1))
            .context("failed to update emulator window")?;
    }
    write_save_state(&machine, &archive, cli.save_state.as_deref())?;
    Ok(())
}

/// Ring-buffered stereo audio output backed by rodio.
struct StandaloneAudio {
    _sink: rodio::MixerDeviceSink,
    _player: rodio::Player,
    ring: Arc<Mutex<VecDeque<f32>>>,
}

impl StandaloneAudio {
    /// Create the output device, or return None when no device is available.
    fn try_new() -> Option<Self> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink().ok()?;
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        let player = rodio::Player::connect_new(sink.mixer());
        player.append(RingSource { ring: ring.clone() });
        Some(Self {
            _sink: sink,
            _player: player,
            ring,
        })
    }

    fn push(&self, samples: Vec<i16>) {
        if samples.is_empty() {
            return;
        }
        let mut ring = self.ring.lock().unwrap();
        ring.extend(samples.into_iter().map(|sample| sample as f32 / 32768.0));
        let overflow = ring.len().saturating_sub(MAX_RING_SAMPLES);
        if overflow > 0 {
            ring.drain(..overflow);
        }
    }

    fn clear(&self) {
        self.ring.lock().unwrap().clear();
    }
}

/// Two seconds of stereo samples at 44.1 kHz.
const MAX_RING_SAMPLES: usize = AUDIO_SAMPLE_RATE as usize * 2 * 2;

/// Pull-style source that reads from the shared ring buffer.
struct RingSource {
    ring: Arc<Mutex<VecDeque<f32>>>,
}

impl Iterator for RingSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        Some(self.ring.lock().unwrap().pop_front().unwrap_or(0.0))
    }
}

impl rodio::Source for RingSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        rodio::ChannelCount::new(2).unwrap()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        rodio::SampleRate::new(AUDIO_SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

fn load_machine_state(archive: &CbeArchive, path: &Path) -> Result<NicaiMachine> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read save state: {}", path.display()))?;
    let content_crc32 = crc32fast::hash(archive.bytes());
    decode_machine(&bytes, content_crc32)
        .with_context(|| format!("failed to load save state: {}", path.display()))
}

fn write_save_state(
    machine: &NicaiMachine,
    archive: &CbeArchive,
    path: Option<&Path>,
) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    let mut buffer = vec![0u8; SERIALIZED_SIZE];
    let content_crc32 = crc32fast::hash(archive.bytes());
    encode_machine(machine, content_crc32, &mut buffer)
        .with_context(|| format!("failed to encode save state: {}", path.display()))?;
    std::fs::write(path, &buffer)
        .with_context(|| format!("failed to write save state: {}", path.display()))?;
    info!("Save state written to: {}", path.display());
    Ok(())
}

fn capture_screenshot(
    machine: &mut NicaiMachine,
    mut stopped_error: Option<anyhow::Error>,
    frames: u32,
    instruction_limit: u64,
    path: &Path,
) -> Result<()> {
    if stopped_error.is_none() {
        for frame in 0..frames {
            if let Err(error) = machine.run_frame(instruction_limit) {
                warn!(
                    "CBE screen callback stopped at frame {}: {error:#}",
                    frame + 1
                );
                stopped_error = Some(error);
                break;
            }
        }
    }

    let pixels = machine.frame_pixels();
    let colors = pixels
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if colors.len() <= 1 {
        if let Some(error) = stopped_error {
            return Err(error).context("guest stopped before rendering a screenshot");
        }
        bail!("guest did not render a screenshot within {frames} frames");
    }
    let nonzero = pixels.iter().filter(|pixel| **pixel != 0).count();
    info!("frame nonzero={nonzero} colors={}", colors.len());
    let rgb: Vec<u8> = pixels
        .iter()
        .flat_map(|pixel| [(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])
        .collect();
    image::save_buffer(path, &rgb, 240, 400, image::ColorType::Rgb8)
        .with_context(|| format!("failed to save screenshot: {}", path.display()))?;
    info!("Screenshot saved to: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn parses_game_path_as_positional_argument() {
        let cli = Cli::try_parse_from(["nicaiemu", "game.CBE"]).unwrap();

        assert_eq!(cli.file, PathBuf::from("game.CBE"));
        assert!(Cli::try_parse_from(["nicaiemu", "--file", "game.CBE"]).is_err());
    }

    #[test]
    fn parses_screenshot_options() {
        let cli = Cli::try_parse_from([
            "nicaiemu",
            "game.CBE",
            "--screenshot",
            "frame.png",
            "--screenshot-frames",
            "120",
        ])
        .unwrap();

        assert_eq!(cli.screenshot, Some(PathBuf::from("frame.png")));
        assert_eq!(cli.screenshot_frames, 120);
    }

    #[test]
    fn screenshot_frames_default_to_thirty() {
        let cli = Cli::try_parse_from(["nicaiemu", "game.CBE"]).unwrap();

        assert_eq!(cli.screenshot, None);
        assert_eq!(cli.screenshot_frames, 30);
    }

    #[test]
    fn parses_save_and_load_state_options() {
        let cli = Cli::try_parse_from([
            "nicaiemu",
            "game.CBE",
            "--save-state",
            "game.sav",
            "--load-state",
            "old.sav",
        ])
        .unwrap();

        assert_eq!(cli.save_state, Some(PathBuf::from("game.sav")));
        assert_eq!(cli.load_state, Some(PathBuf::from("old.sav")));
    }

    #[test]
    fn parses_every_display_filter() {
        for (name, expected) in [
            ("nearest", ScaleFilter::Nearest),
            ("bilinear", ScaleFilter::Bilinear),
            ("bicubic", ScaleFilter::Bicubic),
            ("xbrz", ScaleFilter::Xbrz),
        ] {
            let cli = Cli::try_parse_from(["nicaiemu", "game.CBE", "--filter", name]).unwrap();
            assert_eq!(cli.filter, expected);
        }
    }

    #[test]
    fn display_filter_defaults_to_nearest() {
        let cli = Cli::try_parse_from(["nicaiemu", "game.CBE"]).unwrap();

        assert_eq!(cli.filter, ScaleFilter::Nearest);
    }

    #[test]
    fn parses_key_remappings() {
        let cli = Cli::try_parse_from([
            "nicaiemu",
            "game.CBE",
            "--remap",
            "enter:space",
            "--remap",
            "up:w",
        ])
        .unwrap();

        assert_eq!(cli.remappings.len(), 2);
        assert_eq!(cli.remappings[0].to_string(), "enter:space");
    }

    #[test]
    fn rejects_invalid_key_remappings() {
        assert!(Cli::try_parse_from(["nicaiemu", "game.CBE", "--remap", "enter:escape",]).is_err());
    }

    #[test]
    fn parses_frontend_experience_options() {
        let cli = Cli::try_parse_from([
            "nicaiemu",
            "game.CBE",
            "--fullscreen",
            "--volume",
            "35",
            "--headless",
            "--frames",
            "120",
        ])
        .unwrap();

        assert!(cli.fullscreen);
        assert_eq!(cli.volume, 35);
        assert!(cli.headless);
        assert_eq!(cli.frames, 120);
    }

    #[test]
    fn frontend_experience_options_have_sensible_defaults() {
        let cli = Cli::try_parse_from(["nicaiemu", "game.CBE"]).unwrap();

        assert!(!cli.fullscreen);
        assert_eq!(cli.volume, 100);
        assert!(!cli.headless);
        assert_eq!(cli.frames, 60);
    }

    #[test]
    fn rejects_out_of_range_volume() {
        assert!(Cli::try_parse_from(["nicaiemu", "game.CBE", "--volume", "101",]).is_err());
    }

    #[test]
    fn prints_version() {
        let output = Cli::command().render_version();
        assert!(output.contains("nicaiemu"));
        assert!(output.contains(env!("CARGO_PKG_VERSION")));
    }
}
