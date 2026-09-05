//! Content-identity display-rotation profile for landscape CBE games.
//!
//! Landscape titles draw 400x240 art pre-rotated into the portrait 240x400
//! framebuffer and rely on the original phone's rotated LCD output, so the
//! emulator must present the raw framebuffer rotated 90 degrees
//! counterclockwise. Known titles are keyed by archive CRC-32 plus byte
//! length (stable across renames, sensitive to any repack); frontends can
//! register additional entries from a user-supplied CSV so new games do not
//! require a core code change.

use crate::machine::Rotation;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::sync::{OnceLock, RwLock};

/// A rotation override entry: (archive CRC-32, archive byte length, rotation).
pub type RotationEntry = (u32, u64, Rotation);

/// Built-in landscape profile for the local corpus; every entry needs a 90
/// degree counterclockwise rotation.
const BUILTIN_PROFILE: &[(u32, u64)] = &[
    (0xEE5A53AC, 341737),  // 暴力摩托
    (0x7A5C0A30, 728876),  // 捕鱼猎人
    (0x50528857, 961146),  // 法老祖玛2
    (0x9C5E0674, 958874),  // 愤怒的小鸟
    (0x52DAD535, 611925),  // 疯狂捕鸟
    (0xF3283516, 606493),  // 疯狂斗地主
    (0x7BCDA1EB, 396952),  // 疯狂企鹅大冒险
    (0x4A849388, 910806),  // 机场指挥部
    (0x701C7D4B, 539016),  // 僵尸先生
    (0x5F320C34, 1413319), // 开心大富翁
    (0x8EDDE44F, 1292332), // 美女桌球
    (0x282FE73D, 1143317), // 三国群殴传
    (0xC6488351, 400101),  // 士兵突袭
    (0xBC3CD75C, 734986),  // 水果达人
    (0x2CB6103B, 1074317), // 吸血鬼猎人
    (0x145C46B4, 1016330), // 小鸟愤怒冬季版
    (0x5E8B5904, 319424),  // 幸运扑克机
];

/// User-supplied entries registered by a frontend; they win over the built-in
/// profile.
fn user_overrides() -> &'static RwLock<Vec<RotationEntry>> {
    static OVERRIDES: OnceLock<RwLock<Vec<RotationEntry>>> = OnceLock::new();
    OVERRIDES.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register user-supplied rotation entries, replacing any earlier set.
pub fn register_rotation_overrides(entries: Vec<RotationEntry>) {
    *user_overrides().write().unwrap() = entries;
}

/// Resolve the automatic rotation for guest content: user overrides first,
/// then the built-in landscape profile, else no rotation.
pub fn rotation_for_archive(bytes: &[u8]) -> Rotation {
    lookup_rotation(crc32fast::hash(bytes), bytes.len() as u64)
}

/// Resolve the rotation for one content identity: user overrides first, then
/// the built-in landscape profile.
pub(crate) fn lookup_rotation(crc: u32, length: u64) -> Rotation {
    user_overrides()
        .read()
        .unwrap()
        .iter()
        .find(|&&(entry_crc, entry_length, _)| entry_crc == crc && entry_length == length)
        .map(|&(_, _, rotation)| rotation)
        .or_else(|| builtin_rotation(crc, length))
        .unwrap_or(Rotation::None)
}

/// Look up the built-in landscape profile by content identity.
pub(crate) fn builtin_rotation(crc: u32, length: u64) -> Option<Rotation> {
    BUILTIN_PROFILE
        .iter()
        .any(|&(expected_crc, expected_length)| crc == expected_crc && length == expected_length)
        .then_some(Rotation::Ccw)
}

/// Parse rotation overrides from CSV text with one `crc32,length,rotation`
/// entry per line: `crc32` is hex (optional `0x` prefix), `length` is a byte
/// count in decimal, and `rotation` is one of `none`, `cw`, or `ccw`. Blank
/// lines and `#` comments are ignored.
pub fn parse_rotation_overrides(text: &str) -> Result<Vec<RotationEntry>> {
    let mut entries = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        let [crc, length, rotation] = fields.as_slice() else {
            bail!(
                "line {}: expected `crc32,length,rotation`, found {line:?}",
                index + 1
            );
        };
        let crc = parse_crc32(crc).with_context(|| format!("line {}: invalid crc32", index + 1))?;
        let length = length
            .parse::<u64>()
            .with_context(|| format!("line {}: invalid length", index + 1))?;
        let rotation = match *rotation {
            "none" => Rotation::None,
            "cw" => Rotation::Cw,
            "ccw" => Rotation::Ccw,
            other => bail!(
                "line {}: unknown rotation {other:?} (use none, cw, or ccw)",
                index + 1
            ),
        };
        entries.push((crc, length, rotation));
    }
    Ok(entries)
}

/// Load and register rotation overrides from a CSV file, returning the number
/// of entries applied.
pub fn load_rotation_overrides(path: &Path) -> Result<usize> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read rotation profile {}", path.display()))?;
    let entries = parse_rotation_overrides(&text)
        .with_context(|| format!("invalid rotation profile {}", path.display()))?;
    let count = entries.len();
    register_rotation_overrides(entries);
    Ok(count)
}

/// Parse a hex CRC-32 with an optional `0x` prefix.
fn parse_crc32(value: &str) -> Result<u32> {
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    u32::from_str_radix(digits, 16).context("crc32 must be a hex number")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The registry is process-global; serialize the tests that touch it.
    static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

    fn sample_content() -> Vec<u8> {
        b"sample content for a rotation override".to_vec()
    }

    #[test]
    fn builtin_profile_resolves_landscape_titles() {
        assert_eq!(builtin_rotation(0x282FE73D, 1143317), Some(Rotation::Ccw));
        assert_eq!(builtin_rotation(0xEE5A53AC, 341737), Some(Rotation::Ccw));
        assert_eq!(builtin_rotation(0x282FE73D, 1), None);
        assert_eq!(builtin_rotation(0x1234_5678, 1143317), None);
    }

    #[test]
    fn rotation_for_archive_falls_back_to_none() {
        assert_eq!(rotation_for_archive(b""), Rotation::None);
        assert_ne!(rotation_for_archive(&[0; 341737]), Rotation::Ccw);
    }

    #[test]
    fn registered_overrides_win_over_the_builtin_profile() {
        let _guard = REGISTRY_LOCK.lock().unwrap();
        let content = sample_content();
        let crc = crc32fast::hash(&content);

        register_rotation_overrides(vec![(crc, content.len() as u64, Rotation::Cw)]);
        assert_eq!(rotation_for_archive(&content), Rotation::Cw);

        // A matching CRC with a different length must not match.
        let mut longer = content.clone();
        longer.push(b'!');
        assert_eq!(rotation_for_archive(&longer), Rotation::None);

        // Re-registering replaces the previous set.
        register_rotation_overrides(vec![(crc, content.len() as u64, Rotation::Ccw)]);
        assert_eq!(rotation_for_archive(&content), Rotation::Ccw);

        register_rotation_overrides(Vec::new());
        assert_eq!(rotation_for_archive(&content), Rotation::None);
    }

    #[test]
    fn override_entries_take_precedence_over_builtin_entries() {
        let _guard = REGISTRY_LOCK.lock().unwrap();
        // Same identity as a built-in landscape title, overridden to none.
        assert_eq!(lookup_rotation(0x282FE73D, 1143317), Rotation::Ccw);
        register_rotation_overrides(vec![(0x282FE73D, 1143317, Rotation::None)]);
        assert_eq!(lookup_rotation(0x282FE73D, 1143317), Rotation::None);
        assert_eq!(lookup_rotation(0xEE5A53AC, 341737), Rotation::Ccw);
        register_rotation_overrides(Vec::new());
        assert_eq!(lookup_rotation(0x282FE73D, 1143317), Rotation::Ccw);
    }

    #[test]
    fn parse_handles_comments_hex_and_decimal() {
        let text = "# comment line\n\
                    282fe73d,1143317,ccw\n\
                    0xEE5A53AC, 341737 , cw\n\
                    \n\
                    0x12345678,42,none\n";
        let entries = parse_rotation_overrides(text).unwrap();
        assert_eq!(
            entries,
            vec![
                (0x282FE73D, 1143317, Rotation::Ccw),
                (0xEE5A53AC, 341737, Rotation::Cw),
                (0x1234_5678, 42, Rotation::None),
            ]
        );
    }

    #[test]
    fn parse_rejects_malformed_entries() {
        let cases = [
            ("282fe73d,1143317", "missing field"),
            ("282fe73d,1143317,sideways", "unknown rotation"),
            ("zzzz,1143317,ccw", "invalid crc"),
            ("282fe73d,length,ccw", "invalid length"),
        ];
        for (text, reason) in cases {
            assert!(parse_rotation_overrides(text).is_err(), "{reason}");
        }
    }

    #[test]
    fn load_registers_entries_from_a_csv_file() {
        let _guard = REGISTRY_LOCK.lock().unwrap();
        let content = sample_content();
        let crc = crc32fast::hash(&content);
        let path =
            std::env::temp_dir().join(format!("nicaiemu-rotation-{}.csv", std::process::id()));
        std::fs::write(&path, format!("{crc:08x},{},ccw\n", content.len())).unwrap();

        let count = load_rotation_overrides(&path).unwrap();
        assert_eq!(count, 1);
        assert_eq!(rotation_for_archive(&content), Rotation::Ccw);
        std::fs::remove_file(&path).ok();
        register_rotation_overrides(Vec::new());
    }
}
