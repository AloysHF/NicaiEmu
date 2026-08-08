use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use nicaiemu_core::{CbeArchive, NicaiMachine};

#[derive(Parser)]
#[command(about = "Run a CBE executable through the headless ARM core")]
struct Cli {
    file: PathBuf,
    #[arg(long, default_value_t = 5_000_000)]
    instruction_limit: u64,
    #[arg(long, default_value_t = 0)]
    frames: u32,
    #[arg(long)]
    press_key: Option<u8>,
    #[arg(long, value_delimiter = ',', default_value = "1")]
    press_frame: Vec<u32>,
    #[arg(long, value_parser = parse_key_event)]
    key_event: Vec<(u32, u8)>,
    #[arg(long, value_parser = parse_pointer_event)]
    pointer_event: Vec<(u32, i32, i32)>,
    #[arg(long)]
    screenshot: Option<PathBuf>,
}

fn parse_key_event(value: &str) -> Result<(u32, u8), String> {
    let (frame, key) = value
        .split_once(':')
        .ok_or_else(|| "key event must use FRAME:KEY syntax".to_owned())?;
    let frame = frame
        .parse()
        .map_err(|_| "key event frame must be an unsigned integer".to_owned())?;
    let key = key
        .parse()
        .map_err(|_| "key event key must be an unsigned byte".to_owned())?;
    Ok((frame, key))
}

fn parse_pointer_event(value: &str) -> Result<(u32, i32, i32), String> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 3 {
        return Err("pointer event must use FRAME:X:Y syntax".to_string());
    }
    let frame = parts[0]
        .parse()
        .map_err(|_| "pointer event frame must be an unsigned integer".to_string())?;
    let x = parts[1]
        .parse()
        .map_err(|_| "pointer event x must be an integer".to_string())?;
    let y = parts[2]
        .parse()
        .map_err(|_| "pointer event y must be an integer".to_string())?;
    Ok((frame, x, y))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let archive = CbeArchive::load(&cli.file)?;
    let mut machine = NicaiMachine::new(&archive)?;
    let mut result = machine.boot(cli.instruction_limit);
    for frame in 0..cli.frames {
        if result.is_ok() {
            let event_keys: Vec<u8> = cli
                .key_event
                .iter()
                .filter_map(|(event_frame, key)| (*event_frame == frame).then_some(*key))
                .collect();
            if cli.press_frame.contains(&frame) {
                if let Some(key) = cli.press_key {
                    machine.set_key(key, true);
                }
            }
            for key in &event_keys {
                machine.set_key(*key, true);
            }
            for &(event_frame, x, y) in &cli.pointer_event {
                if event_frame == frame {
                    machine.set_pointer(x, y, true);
                }
            }
            result = machine.run_frame(cli.instruction_limit);
            if cli.press_frame.contains(&frame) {
                if let Some(key) = cli.press_key {
                    machine.set_key(key, false);
                }
            }
            for key in event_keys {
                machine.set_key(key, false);
            }
            for &(event_frame, x, y) in &cli.pointer_event {
                if event_frame == frame {
                    machine.set_pointer(x, y, false);
                }
            }
        }
    }
    eprintln!("machine={machine:?}");
    eprintln!(
        "screen active=0x{:08X} pending=0x{:08X}",
        machine.active_screen(),
        machine.pending_screen()
    );
    let pixels = machine.frame_pixels();
    let nonzero = pixels.iter().filter(|pixel| **pixel != 0).count();
    let colors = pixels
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    eprintln!("frame nonzero={nonzero} colors={}", colors.len());
    if let Some(path) = &cli.screenshot {
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|pixel| [(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])
            .collect();
        image::save_buffer(path, &rgb, 240, 400, image::ColorType::Rgb8)?;
    }
    for register in 0..=14 {
        eprintln!("r{register}=0x{:08X}", machine.register_value(register));
    }
    let r0 = machine.register_value(0);
    for offset in (0..=0x50).step_by(4) {
        eprintln!(
            "[r0+0x{offset:02X}]=0x{:08X}",
            machine.read_u32(r0 + offset)
        );
    }
    let mut calls: Vec<_> = machine.service_calls().iter().collect();
    calls.sort_by_key(|((group, index), _)| (*group, *index));
    for ((group, index), count) in calls {
        eprintln!("service group={group} index={index} count={count}");
    }
    for address in machine.bad_accesses().iter().take(32) {
        eprintln!("unmapped address=0x{address:08X}");
    }
    result
}
