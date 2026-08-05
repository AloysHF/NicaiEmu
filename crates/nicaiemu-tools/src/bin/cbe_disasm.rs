use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;
use nicaiemu_core::{CbeArchive, CbeExecutable};
use unarm::{parse_thumb, Options};

#[derive(Parser)]
#[command(about = "Disassemble a Thumb range from a CBE executable")]
struct Cli {
    file: PathBuf,
    #[arg(long)]
    address: u32,
    #[arg(long, default_value_t = 0x100)]
    length: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let archive = CbeArchive::load(&cli.file)?;
    let executable = CbeExecutable::parse(archive.bytes())?;
    let relative = cli
        .address
        .checked_sub(executable.code_address())
        .ok_or_else(|| anyhow::anyhow!("address precedes the CBE code image"))?
        as usize;
    let start = executable.code_offset + relative;
    let end = start.saturating_add(cli.length).min(archive.bytes().len());
    if start >= end {
        bail!("disassembly range is outside the CBE code image");
    }
    let options = Options::default();
    let mut offset = start;
    let mut address = cli.address & !1;
    while offset + 2 <= end {
        let first = u16::from_le_bytes(archive.bytes()[offset..offset + 2].try_into()?);
        let second = if offset + 4 <= end {
            u16::from_le_bytes(archive.bytes()[offset + 2..offset + 4].try_into()?)
        } else {
            0
        };
        let (instruction, size) =
            parse_thumb(first as u32 | ((second as u32) << 16), address, &options);
        println!("{address:08X}: {}", instruction.display(&options));
        offset += size as usize;
        address += size;
    }
    Ok(())
}
