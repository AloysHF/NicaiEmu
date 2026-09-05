//! Save-state serialization for the Nicai/MStar CBE emulator.
//!
//! The payload is a bincode snapshot of the full machine state, compressed
//! with LZ4 and guarded by a versioned header plus CRC32 checksums so that
//! corrupted or mismatched save files are rejected before use.

use anyhow::{bail, Context, Result};
use bincode::Options;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::machine::NicaiMachine;

const MAGIC: &[u8; 8] = b"NICAISTM";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 32;
const MAX_DECODED_SIZE: usize = 64 * 1024 * 1024;

/// Fixed capacity required by the libretro serialization API.
pub const SERIALIZED_SIZE: usize = 48 * 1024 * 1024;

/// Encode a machine snapshot into the fixed-size save-state buffer.
pub fn encode_machine(machine: &NicaiMachine, content_crc32: u32, output: &mut [u8]) -> Result<()> {
    encode_value(machine, content_crc32, output)
}

/// Decode a machine snapshot, rejecting states from other content builds.
pub fn decode_machine(input: &[u8], expected_content_crc32: u32) -> Result<NicaiMachine> {
    let mut machine: NicaiMachine = decode_value(input, expected_content_crc32)?;
    machine.normalize_input_after_load();
    Ok(machine)
}

fn encode_value<T: Serialize>(value: &T, content_crc32: u32, output: &mut [u8]) -> Result<()> {
    if output.len() < SERIALIZED_SIZE {
        bail!(
            "save-state buffer is too small: got {}, need {}",
            output.len(),
            SERIALIZED_SIZE
        );
    }

    let decoded = codec()
        .serialize(value)
        .context("failed to encode save-state payload")?;
    if decoded.len() > MAX_DECODED_SIZE {
        bail!("save-state payload exceeds the decoded size limit");
    }

    let payload = lz4_flex::compress(&decoded);
    if payload.len() > SERIALIZED_SIZE - HEADER_SIZE {
        bail!("save state exceeds the fixed serialization capacity");
    }

    output.fill(0);
    output[..8].copy_from_slice(MAGIC);
    output[8..12].copy_from_slice(&VERSION.to_le_bytes());
    output[12..16].copy_from_slice(&content_crc32.to_le_bytes());
    output[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    output[20..24].copy_from_slice(&(decoded.len() as u32).to_le_bytes());
    output[24..28].copy_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    output[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(&payload);
    Ok(())
}

fn decode_value<T: DeserializeOwned>(input: &[u8], expected_content_crc32: u32) -> Result<T> {
    if input.len() < HEADER_SIZE {
        bail!("save state is truncated");
    }
    if &input[..8] != MAGIC {
        bail!("invalid save-state signature");
    }

    let version = read_u32(input, 8);
    if version != VERSION {
        bail!("unsupported save-state version {version}");
    }
    if read_u32(input, 12) != expected_content_crc32 {
        bail!("save state belongs to different content data");
    }

    let payload_len = read_u32(input, 16) as usize;
    let decoded_len = read_u32(input, 20) as usize;
    if decoded_len > MAX_DECODED_SIZE {
        bail!("save-state decoded size exceeds the limit");
    }

    let payload_end = HEADER_SIZE
        .checked_add(payload_len)
        .filter(|&end| end <= input.len() && end <= SERIALIZED_SIZE)
        .context("invalid save-state payload length")?;
    let payload = &input[HEADER_SIZE..payload_end];
    if crc32fast::hash(payload) != read_u32(input, 24) {
        bail!("save-state checksum mismatch");
    }

    let decoded =
        lz4_flex::decompress(payload, decoded_len).context("failed to decompress save state")?;
    codec()
        .with_limit(MAX_DECODED_SIZE as u64)
        .deserialize(&decoded)
        .context("failed to decode save state")
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::NicaiMachine;
    use crate::CbeArchive;
    use serde::Deserialize;
    use std::path::Path;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestValue {
        text: String,
        number: u64,
    }

    fn encoded_value() -> Vec<u8> {
        let value = TestValue {
            text: "NicaiEmu".to_string(),
            number: 42,
        };
        let mut output = vec![0u8; SERIALIZED_SIZE];
        encode_value(&value, 0x1234_5678, &mut output).unwrap();
        output
    }

    #[test]
    fn codec_round_trip_and_checksum() {
        let mut output = encoded_value();
        assert_eq!(
            decode_value::<TestValue>(&output, 0x1234_5678).unwrap(),
            TestValue {
                text: "NicaiEmu".to_string(),
                number: 42,
            }
        );

        output[HEADER_SIZE] ^= 1;
        assert!(decode_value::<TestValue>(&output, 0x1234_5678).is_err());
    }

    #[test]
    fn codec_rejects_wrong_content_version_and_truncation() {
        let mut output = encoded_value();
        assert!(decode_value::<TestValue>(&output, 0x8765_4321).is_err());

        output[8..12].copy_from_slice(&2u32.to_le_bytes());
        assert!(decode_value::<TestValue>(&output, 0x1234_5678).is_err());

        assert!(decode_value::<TestValue>(&output[..HEADER_SIZE - 1], 0x1234_5678).is_err());
    }

    fn framebuffer_crc32(machine: &mut NicaiMachine) -> u32 {
        let bytes: Vec<u8> = machine
            .frame_pixels()
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        crc32fast::hash(&bytes)
    }

    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_machine_round_trip_restores_execution() {
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

        let archive = CbeArchive::load(Path::new(game_path)).unwrap();
        let content_crc32 = crc32fast::hash(archive.bytes());
        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(5_000_000).unwrap();
        for _ in 0..60 {
            machine.run_frame(5_000_000).unwrap();
        }

        let mut state = vec![0u8; SERIALIZED_SIZE];
        encode_machine(&machine, content_crc32, &mut state).unwrap();

        let mut restored = decode_machine(&state, content_crc32).unwrap();
        restored.run_frame(5_000_000).unwrap();
        machine.run_frame(5_000_000).unwrap();
        assert_eq!(restored.instruction_count(), machine.instruction_count());
        assert_eq!(restored.last_pc(), machine.last_pc());
        assert_eq!(
            framebuffer_crc32(&mut restored),
            framebuffer_crc32(&mut machine)
        );
    }

    /// Regression for issue #40's standalone follow-up: the rotation is
    /// presentation state skipped by the codec, so a restored machine loses
    /// it and `resolve_auto_rotation` must recover it for landscape content.
    #[test]
    #[ignore = "requires local CBE game assets (set NICAI_GAME_DIR)"]
    fn real_landscape_rotation_survives_save_load_after_resolve() {
        let game_dir = std::env::var_os("NICAI_GAME_DIR").expect("NICAI_GAME_DIR is not set");
        let game_path = Path::new(&game_dir).join("三国群殴传.CBE");
        let archive = CbeArchive::load(&game_path).unwrap();
        let content_crc32 = crc32fast::hash(archive.bytes());

        let mut machine = NicaiMachine::new(&archive).unwrap();
        machine.boot(5_000_000).unwrap();
        assert_eq!(machine.display_size(), (400, 240));

        let mut state = vec![0u8; SERIALIZED_SIZE];
        encode_machine(&machine, content_crc32, &mut state).unwrap();
        let mut restored = decode_machine(&state, content_crc32).unwrap();

        // The codec skips presentation state: the restored machine presents
        // unrotated until the frontend re-resolves the rotation.
        assert_eq!(restored.display_size(), (240, 400));
        restored.resolve_auto_rotation(&archive);
        assert_eq!(restored.display_size(), (400, 240));
        assert_eq!(
            framebuffer_crc32(&mut restored),
            framebuffer_crc32(&mut machine)
        );

        // An explicit override wins over the automatic profile.
        let mut overridden = decode_machine(&state, content_crc32).unwrap();
        overridden.set_rotation(crate::Rotation::None);
        overridden.resolve_auto_rotation(&archive);
        assert_eq!(overridden.display_size(), (240, 400));
    }
}
