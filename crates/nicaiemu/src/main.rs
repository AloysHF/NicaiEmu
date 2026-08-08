//! NicaiEmu desktop frontend.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use log::{info, warn};
use minifb::{Key, ScaleMode, Window, WindowOptions};
use nicaiemu_core::{CbeArchive, NicaiMachine};

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

    let mut machine = NicaiMachine::new(&archive).context("failed to create CBE machine")?;
    let boot_result = machine.boot(cli.instruction_limit);

    if let Some(path) = &cli.screenshot {
        capture_screenshot(
            &mut machine,
            boot_result.err(),
            cli.screenshot_frames,
            cli.instruction_limit,
            path,
        )?;
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

    info!("Controls: arrows/WASD move, Enter/F confirms, Q/E soft keys, R resets, Esc exits");
    while window.is_open() && !window.is_key_down(Key::Escape) {
        update_keys(&window, &mut machine);
        if window.is_key_pressed(Key::R, minifb::KeyRepeat::No) {
            machine
                .reset(&archive, cli.instruction_limit)
                .context("failed to reset CBE application")?;
            info!("Game reset");
        }
        machine
            .run_frame(cli.instruction_limit)
            .context("guest screen callback failed")?;
        let pixels = machine.frame_pixels();
        window
            .update_with_buffer(&pixels, 240, 400)
            .context("failed to update emulator window")?;
    }
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
}
