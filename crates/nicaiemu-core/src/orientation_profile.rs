//! Content-identity display-orientation profile for landscape CBE games.
//!
//! Landscape titles draw 400x240 art pre-rotated into the portrait 240x400
//! framebuffer and rely on the original phone's rotated LCD output, so the
//! emulator must present the raw framebuffer rotated 90 degrees
//! counterclockwise. Known titles are keyed by archive CRC-32 plus byte
//! length (stable across renames, sensitive to any repack); frontends can
//! register additional entries from a user-supplied CSV so new games do not
//! require a core code change.
//!
//! The content-identity keying is deliberate: the landscape layout is an
//! authoring convention with no runtime signal to detect. Landscape games
//! blit through the same LCD services with in-bounds portrait coordinates
//! (the sprites themselves are stored pre-rotated), the executable header
//! carries no screen-geometry metadata, and the guest never rewrites the
//! 240x400 screen-struct fields. A repacked variant of a known title
//! therefore needs a new profile entry rather than re-detection.

use crate::machine::DisplayOrientation;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::sync::{OnceLock, RwLock};

/// A display-orientation override entry: (archive CRC-32, archive byte
/// length, orientation). Entries never carry `Auto`; they are the resolved
/// answer for one content identity.
pub type OrientationEntry = (u32, u64, DisplayOrientation);

/// Built-in landscape profile for the local corpus; every entry needs the
/// counterclockwise landscape presentation.
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
fn user_overrides() -> &'static RwLock<Vec<OrientationEntry>> {
    static OVERRIDES: OnceLock<RwLock<Vec<OrientationEntry>>> = OnceLock::new();
    OVERRIDES.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register user-supplied orientation entries, replacing any earlier set.
pub fn register_orientation_overrides(entries: Vec<OrientationEntry>) {
    *user_overrides().write().unwrap() = entries;
}

/// Resolve the automatic orientation for guest content: user overrides first,
/// then the built-in landscape profile, else portrait.
pub fn orientation_for_archive(bytes: &[u8]) -> DisplayOrientation {
    lookup_orientation(crc32fast::hash(bytes), bytes.len() as u64)
}

/// Resolve the orientation for one content identity: user overrides first,
/// then the built-in landscape profile.
pub(crate) fn lookup_orientation(crc: u32, length: u64) -> DisplayOrientation {
    user_overrides()
        .read()
        .unwrap()
        .iter()
        .find(|&&(entry_crc, entry_length, _)| entry_crc == crc && entry_length == length)
        .map(|&(_, _, orientation)| orientation)
        .or_else(|| builtin_orientation(crc, length))
        .unwrap_or(DisplayOrientation::Portrait)
}

/// Look up the built-in landscape profile by content identity.
pub(crate) fn builtin_orientation(crc: u32, length: u64) -> Option<DisplayOrientation> {
    BUILTIN_PROFILE
        .iter()
        .any(|&(expected_crc, expected_length)| crc == expected_crc && length == expected_length)
        .then_some(DisplayOrientation::Landscape)
}

/// Parse orientation overrides from CSV text with one
/// `crc32,length,orientation` entry per line: `crc32` is hex (optional `0x`
/// prefix), `length` is a byte count in decimal, and `orientation` is either
/// `landscape` or `portrait`. Blank lines and `#` comments are ignored.
pub fn parse_orientation_overrides(text: &str) -> Result<Vec<OrientationEntry>> {
    let mut entries = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        let [crc, length, orientation] = fields.as_slice() else {
            bail!(
                "line {}: expected `crc32,length,orientation`, found {line:?}",
                index + 1
            );
        };
        let crc = parse_crc32(crc).with_context(|| format!("line {}: invalid crc32", index + 1))?;
        let length = length
            .parse::<u64>()
            .with_context(|| format!("line {}: invalid length", index + 1))?;
        let orientation = match *orientation {
            "landscape" => DisplayOrientation::Landscape,
            "portrait" => DisplayOrientation::Portrait,
            other => bail!(
                "line {}: unknown orientation {other:?} (use landscape or portrait)",
                index + 1
            ),
        };
        entries.push((crc, length, orientation));
    }
    Ok(entries)
}

/// Load and register orientation overrides from a CSV file, returning the
/// number of entries applied.
pub fn load_orientation_overrides(path: &Path) -> Result<usize> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read orientation profile {}", path.display()))?;
    let entries = parse_orientation_overrides(&text)
        .with_context(|| format!("invalid orientation profile {}", path.display()))?;
    let count = entries.len();
    register_orientation_overrides(entries);
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
        b"sample content for an orientation override".to_vec()
    }

    #[test]
    fn builtin_profile_resolves_landscape_titles() {
        assert_eq!(
            builtin_orientation(0x282FE73D, 1143317),
            Some(DisplayOrientation::Landscape)
        );
        assert_eq!(
            builtin_orientation(0xEE5A53AC, 341737),
            Some(DisplayOrientation::Landscape)
        );
        assert_eq!(builtin_orientation(0x282FE73D, 1), None);
        assert_eq!(builtin_orientation(0x1234_5678, 1143317), None);
    }

    #[test]
    fn orientation_for_archive_falls_back_to_portrait() {
        assert_eq!(orientation_for_archive(b""), DisplayOrientation::Portrait);
        assert_ne!(
            orientation_for_archive(&[0; 341737]),
            DisplayOrientation::Landscape
        );
    }

    #[test]
    fn registered_overrides_win_over_the_builtin_profile() {
        let _guard = REGISTRY_LOCK.lock().unwrap();
        let content = sample_content();
        let crc = crc32fast::hash(&content);

        register_orientation_overrides(vec![(
            crc,
            content.len() as u64,
            DisplayOrientation::Landscape,
        )]);
        assert_eq!(
            orientation_for_archive(&content),
            DisplayOrientation::Landscape
        );

        // A matching CRC with a different length must not match.
        let mut longer = content.clone();
        longer.push(b'!');
        assert_eq!(
            orientation_for_archive(&longer),
            DisplayOrientation::Portrait
        );

        // Re-registering replaces the previous set.
        register_orientation_overrides(vec![(
            crc,
            content.len() as u64,
            DisplayOrientation::Portrait,
        )]);
        assert_eq!(
            orientation_for_archive(&content),
            DisplayOrientation::Portrait
        );

        register_orientation_overrides(Vec::new());
        assert_eq!(
            orientation_for_archive(&content),
            DisplayOrientation::Portrait
        );
    }

    #[test]
    fn override_entries_take_precedence_over_builtin_entries() {
        let _guard = REGISTRY_LOCK.lock().unwrap();
        // Same identity as a built-in landscape title, overridden to portrait.
        assert_eq!(
            lookup_orientation(0x282FE73D, 1143317),
            DisplayOrientation::Landscape
        );
        register_orientation_overrides(vec![(0x282FE73D, 1143317, DisplayOrientation::Portrait)]);
        assert_eq!(
            lookup_orientation(0x282FE73D, 1143317),
            DisplayOrientation::Portrait
        );
        assert_eq!(
            lookup_orientation(0xEE5A53AC, 341737),
            DisplayOrientation::Landscape
        );
        register_orientation_overrides(Vec::new());
        assert_eq!(
            lookup_orientation(0x282FE73D, 1143317),
            DisplayOrientation::Landscape
        );
    }

    #[test]
    fn parse_handles_comments_hex_and_decimal() {
        let text = "# comment line\n\
                    282fe73d,1143317,landscape\n\
                    0xEE5A53AC, 341737 , portrait\n\
                    \n\
                    0x12345678,42,portrait\n";
        let entries = parse_orientation_overrides(text).unwrap();
        assert_eq!(
            entries,
            vec![
                (0x282FE73D, 1143317, DisplayOrientation::Landscape),
                (0xEE5A53AC, 341737, DisplayOrientation::Portrait),
                (0x1234_5678, 42, DisplayOrientation::Portrait),
            ]
        );
    }

    #[test]
    fn parse_rejects_malformed_entries() {
        let cases = [
            ("282fe73d,1143317", "missing field"),
            ("282fe73d,1143317,sideways", "unknown orientation"),
            // Legacy four-state vocabulary is no longer accepted.
            ("282fe73d,1143317,ccw", "legacy rotation value"),
            ("282fe73d,1143317,cw", "legacy rotation value"),
            ("282fe73d,1143317,none", "legacy rotation value"),
            ("zzzz,1143317,landscape", "invalid crc"),
            ("282fe73d,length,landscape", "invalid length"),
        ];
        for (text, reason) in cases {
            assert!(parse_orientation_overrides(text).is_err(), "{reason}");
        }
    }

    #[test]
    fn load_registers_entries_from_a_csv_file() {
        let _guard = REGISTRY_LOCK.lock().unwrap();
        let content = sample_content();
        let crc = crc32fast::hash(&content);
        let path =
            std::env::temp_dir().join(format!("nicaiemu-orientation-{}.csv", std::process::id()));
        std::fs::write(&path, format!("{crc:08x},{},landscape\n", content.len())).unwrap();

        let count = load_orientation_overrides(&path).unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            orientation_for_archive(&content),
            DisplayOrientation::Landscape
        );
        std::fs::remove_file(&path).ok();
        register_orientation_overrides(Vec::new());
    }
}
