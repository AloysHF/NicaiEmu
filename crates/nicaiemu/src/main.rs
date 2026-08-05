//! NicaiEmu desktop frontend.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use log::info;
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
    machine
        .boot(cli.instruction_limit)
        .context("failed to initialize CBE application")?;

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

    info!("Controls: arrows/WASD move, Enter/F confirms, Q/E are soft keys, Esc exits");
    while window.is_open() && !window.is_key_down(Key::Escape) {
        update_keys(&window, &mut machine);
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
