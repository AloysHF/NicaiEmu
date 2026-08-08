//! NicaiEmu desktop frontend.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use clap::Parser;
use log::{info, warn};
use minifb::{Key, ScaleMode, Window, WindowOptions};
use nicaiemu_core::{
    decode_machine, encode_machine, CbeArchive, NicaiMachine, AUDIO_SAMPLE_RATE, SERIALIZED_SIZE,
};

#[derive(Parser)]
#[command(name = "nicaiemu")]
#[command(about = "A desktop emulator for Nicai/MStar CBE games")]
struct Cli {
    /// Path to the CBE executable.
    #[arg(short, long)]
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
            resize: true,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .context("failed to create emulator window")?;
    window.set_target_fps(30);
    let audio_output = StandaloneAudio::try_new();
    if audio_output.is_none() {
        warn!("Audio output unavailable; running without sound");
    }

    info!("Controls: arrows/WASD move, Enter/F confirms, Q/E soft keys, R resets, Esc exits");
    while window.is_open() && !window.is_key_down(Key::Escape) {
        update_keys(&window, &mut machine);
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
            audio.push(machine.take_audio_samples(2048));
        }
        let pixels = machine.frame_pixels();
        window
            .update_with_buffer(&pixels, 240, 400)
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

fn update_keys(window: &Window, machine: &mut NicaiMachine) {
    const KEY_MAP: &[(u8, &[Key])] = &[
        (0, &[Key::Key0]),
        (1, &[Key::Key1]),
        (2, &[Key::Key2]),
        (3, &[Key::Key3]),
        (4, &[Key::Key4]),
        (5, &[Key::Key5]),
        (6, &[Key::Key6]),
        (7, &[Key::Key7]),
        (8, &[Key::Key8]),
        (9, &[Key::Key9]),
        (12, &[Key::Q]),
        (13, &[Key::E]),
        (14, &[Key::Enter, Key::F]),
        (15, &[Key::Left, Key::A]),
        (16, &[Key::Right, Key::D]),
        (17, &[Key::Up, Key::W]),
        (18, &[Key::Down, Key::S]),
        (19, &[Key::N]),
        (20, &[Key::M]),
    ];
    for &(guest_key, host_keys) in KEY_MAP {
        machine.set_key(
            guest_key,
            host_keys.iter().any(|key| window.is_key_down(*key)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_screenshot_options() {
        let cli = Cli::try_parse_from([
            "nicaiemu",
            "--file",
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
        let cli = Cli::try_parse_from(["nicaiemu", "--file", "game.CBE"]).unwrap();

        assert_eq!(cli.screenshot, None);
        assert_eq!(cli.screenshot_frames, 30);
    }

    #[test]
    fn parses_save_and_load_state_options() {
        let cli = Cli::try_parse_from([
            "nicaiemu",
            "--file",
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
}
