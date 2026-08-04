//! NicaiEmu - Desktop Application
//!
//! A desktop emulator for Nicai/MStar CBE format games.

use std::path::PathBuf;
use anyhow::{Context, Result};
use clap::Parser;
use log::info;
use minifb::{Key, Window, WindowOptions};

use nicaiemu_core::{CbeArchive, NicaiRuntime};

/// NicaiEmu - Nicai/MStar CBE Game Emulator
#[derive(Parser)]
#[command(name = "nicaiemu")]
#[command(about = "A desktop emulator for Nicai/MStar CBE format games")]
struct Cli {
    /// Path to the CBE file to load
    #[arg(short, long)]
    file: PathBuf,

    /// Scene to load (optional, loads first scene if not specified)
    #[arg(short, long)]
    scene: Option<String>,

    /// Window width
    #[arg(short, long, default_value = "480")]
    width: usize,

    /// Window height
    #[arg(short, long, default_value = "800")]
    height: usize,

    /// Enable verbose logging
    #[arg(short, v)]
    verbose: bool,
}

fn main() -> Result<()> {
    // Initialize logger
    let cli = Cli::parse();

    let log_level = if cli.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    env_logger::Builder::new()
        .filter_level(log_level)
        .init();

    info!("NicaiEmu - Nicai/MStar CBE Game Emulator");
    info!("Loading: {}", cli.file.display());

    // Load the CBE archive
    let archive = CbeArchive::load(&cli.file)
        .with_context(|| format!("Failed to load CBE file: {}", cli.file.display()))?;

    // Print archive summary
    let summary = archive.summary();
    info!("\n{}", summary);

    // Create runtime
    let mut runtime = NicaiRuntime::new(archive);

    // Load scene
    if let Some(scene_name) = &cli.scene {
        runtime.load_scene(scene_name)?;
    } else {
        runtime.load_first_scene()?;
    }

    // Create window
    let mut window = Window::new(
        "NicaiEmu",
        cli.width,
        cli.height,
        WindowOptions {
            resize: true,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .context("Failed to create window")?;

    // Limit to ~60fps
    window.set_target_fps(60);

    info!("Window created, starting main loop...");

    // Main loop
    let mut last_time = std::time::Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Calculate delta time
        let now = std::time::Instant::now();
        let dt = now.duration_since(last_time).as_secs_f32();
        last_time = now;

        // Update runtime
        runtime.update(dt);

        // Render frame
        let frame_buffer = runtime.render();

        // Update window
        // Convert RGBA to u32 for minifb
        let buffer: Vec<u32> = frame_buffer
            .data
            .chunks_exact(4)
            .map(|pixel| {
                let r = pixel[0] as u32;
                let g = pixel[1] as u32;
                let b = pixel[2] as u32;
                (r << 16) | (g << 8) | b
            })
            .collect();

        window
            .update_with_buffer(&buffer, frame_buffer.width as usize, frame_buffer.height as usize)
            .context("Failed to update window")?;
    }

    info!("Shutting down...");

    Ok(())
}
