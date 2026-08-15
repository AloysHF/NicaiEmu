//! Guest resource package parsing (native, flat, and grouped layouts).

use encoding_rs::GBK;
use serde::{Deserialize, Serialize};

fn decode_name(bytes: &[u8]) -> String {
    GBK.decode(bytes).0.into_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HostResource {
    pub(crate) name: String,
    pub(crate) data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct HostResourcePackage {
    pub(crate) name: String,
    pub(crate) resources: Vec<HostResource>,
}

pub(crate) fn native_package_resources(data: &[u8], start: usize) -> Vec<HostResource> {
    let read_u32 = |offset: usize| {
        data.get(offset..offset + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
    };
    let Some(header_size) = read_u32(start).map(|value| value as usize) else {
        return Vec::new();
    };
    if header_size < 8 || read_u32(start + 8) != Some(1) {
        return Vec::new();
    }
    let Some(metadata_size) = read_u32(start + 12).map(|value| value as usize) else {
        return Vec::new();
    };
    let metadata_start = start + 16;
    let Some(data_size) = read_u32(metadata_start).map(|value| value as usize) else {
        return Vec::new();
    };
    let Some(count) = read_u32(metadata_start + 4).map(|value| value as usize) else {
        return Vec::new();
    };
    if !(1..=10_000).contains(&count) {
        return Vec::new();
    }
    let Some(data_start) = metadata_start.checked_add(metadata_size) else {
        return Vec::new();
    };
    if data_start
        .checked_add(data_size)
        .is_none_or(|end| end > data.len())
    {
        return Vec::new();
    }

    let mut offsets = Vec::with_capacity(count);
    let mut cursor = metadata_start + 8;
    for _ in 0..count {
        let Some(offset) = read_u32(cursor).map(|value| value as usize) else {
            return Vec::new();
        };
        if offset > data_size || offsets.last().is_some_and(|previous| *previous > offset) {
            return Vec::new();
        }
        offsets.push(offset);
        cursor += 4;
    }

    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        let Some(&length) = data.get(cursor) else {
            return Vec::new();
        };
        let length = length as usize;
        let Some(end) = cursor.checked_add(1 + length) else {
            return Vec::new();
        };
        let Some(name) = data.get(cursor + 1..end) else {
            return Vec::new();
        };
        names.push(decode_name(name));
        cursor = end;
    }
    if cursor > data_start {
        return Vec::new();
    }

    names
        .into_iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let resource_start = data_start.checked_add(offsets[index])?;
            let resource_end =
                data_start.checked_add(offsets.get(index + 1).copied().unwrap_or(data_size))?;
            (resource_start <= resource_end && resource_end <= data.len()).then(|| HostResource {
                name,
                data: data[resource_start..resource_end].to_vec(),
            })
        })
        .collect()
}

pub(crate) fn flat_package_resources(
    data: &[u8],
    start: usize,
) -> Option<(Vec<HostResource>, usize)> {
    let read_u32 = |offset: usize| {
        data.get(offset..offset + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
    };
    let header_size = read_u32(start)? as usize;
    let data_size = read_u32(start + 4)? as usize;
    let count = read_u32(start + 8)? as usize;
    if header_size < 8 || !(1..=10_000).contains(&count) {
        return None;
    }
    let data_start = start.checked_add(4)?.checked_add(header_size)?;
    let package_end = data_start.checked_add(data_size)?;
    if package_end > data.len() {
        return None;
    }

    let mut offsets = Vec::with_capacity(count);
    let mut cursor = start.checked_add(12)?;
    for _ in 0..count {
        let offset = read_u32(cursor)? as usize;
        if offset > data_size || offsets.last().is_some_and(|previous| *previous > offset) {
            return None;
        }
        offsets.push(offset);
        cursor += 4;
    }
    if offsets.first() != Some(&0) {
        return None;
    }

    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        let length = *data.get(cursor)? as usize;
        let end = cursor.checked_add(1 + length)?;
        let name = data.get(cursor + 1..end)?;
        names.push(decode_name(name));
        cursor = end;
    }
    if cursor > data_start {
        return None;
    }

    let resources = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let end = offsets.get(index + 1).copied().unwrap_or(data_size);
            HostResource {
                name,
                data: data[data_start + offsets[index]..data_start + end].to_vec(),
            }
        })
        .collect();
    Some((resources, package_end))
}

pub(crate) fn grouped_package_resources(
    data: &[u8],
    start: usize,
    size: usize,
) -> Vec<HostResource> {
    let Some(header_size) = data
        .get(start..start + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .map(|value| value as usize)
    else {
        return Vec::new();
    };
    let Some(mut cursor) = start
        .checked_add(4)
        .and_then(|value| value.checked_add(header_size))
    else {
        return Vec::new();
    };
    let limit = start.saturating_add(size).min(data.len());
    let mut resources = Vec::new();
    while cursor < limit {
        let Some((mut group, next)) = flat_package_resources(data, cursor) else {
            break;
        };
        if next <= cursor || next > limit {
            break;
        }
        resources.append(&mut group);
        cursor = next;
    }
    resources
}

pub(crate) fn named_package_resources(
    data: &[u8],
    start: usize,
    size: usize,
) -> Vec<HostResourcePackage> {
    let read_u32 = |offset: usize| {
        data.get(offset..offset + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
    };
    let Some(header_size) = read_u32(start).map(|value| value as usize) else {
        return Vec::new();
    };
    let Some(package_count) = read_u32(start + 8).map(|value| value as usize) else {
        return Vec::new();
    };
    if header_size < 8 || !(2..=256).contains(&package_count) {
        return Vec::new();
    }
    let limit = start.saturating_add(size).min(data.len());
    let Some(root_block) = start.checked_add(4 + header_size) else {
        return Vec::new();
    };
    if root_block > limit {
        return Vec::new();
    }

    let mut cursor = start + 12;
    let mut packages = Vec::with_capacity(package_count - 1);
    for _ in 1..package_count {
        let Some(&name_length) = data.get(cursor) else {
            return Vec::new();
        };
        let name_length = name_length as usize;
        let Some(name_end) = cursor.checked_add(1 + name_length) else {
            return Vec::new();
        };
        let Some(name_bytes) = data.get(cursor + 1..name_end) else {
            return Vec::new();
        };
        let Some(block_offset) = read_u32(name_end).map(|value| value as usize) else {
            return Vec::new();
        };
        let Some(block_start) = start.checked_add(block_offset) else {
            return Vec::new();
        };
        let Some((resources, block_end)) = flat_package_resources(data, block_start) else {
            return Vec::new();
        };
        if block_start < root_block || block_end > limit {
            return Vec::new();
        }
        packages.push(HostResourcePackage {
            name: decode_name(name_bytes),
            resources,
        });
        cursor = name_end + 4;
    }
    if cursor > root_block {
        return Vec::new();
    }
    packages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_data_package_resources() {
        let mut data = vec![0u8; 39];
        data[0..4].copy_from_slice(&8u32.to_le_bytes());
        data[8..12].copy_from_slice(&1u32.to_le_bytes());
        data[12..16].copy_from_slice(&20u32.to_le_bytes());
        data[16..20].copy_from_slice(&3u32.to_le_bytes());
        data[20..24].copy_from_slice(&2u32.to_le_bytes());
        data[24..28].copy_from_slice(&0u32.to_le_bytes());
        data[28..32].copy_from_slice(&1u32.to_le_bytes());
        data[32..36].copy_from_slice(&[1, b'a', 1, b'b']);
        data[36..39].copy_from_slice(&[0x11, 0x22, 0x33]);

        let resources = native_package_resources(&data, 0);
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].name, "a");
        assert_eq!(resources[0].data, [0x11]);
        assert_eq!(resources[1].name, "b");
        assert_eq!(resources[1].data, [0x22, 0x33]);
    }

    #[test]
    fn parses_grouped_data_package_resources() {
        let mut data = vec![0u8; 39];
        data[0..4].copy_from_slice(&8u32.to_le_bytes());
        data[12..16].copy_from_slice(&20u32.to_le_bytes());
        data[16..20].copy_from_slice(&3u32.to_le_bytes());
        data[20..24].copy_from_slice(&2u32.to_le_bytes());
        data[24..28].copy_from_slice(&0u32.to_le_bytes());
        data[28..32].copy_from_slice(&1u32.to_le_bytes());
        data[32..36].copy_from_slice(&[1, b'a', 1, b'b']);
        data[36..39].copy_from_slice(&[0x11, 0x22, 0x33]);

        let resources = grouped_package_resources(&data, 0, data.len());
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].name, "a");
        assert_eq!(resources[1].name, "b");
    }

    #[test]
    fn parses_named_child_packages() {
        let mut data = vec![0u8; 67];
        data[0..4].copy_from_slice(&16u32.to_le_bytes());
        data[8..12].copy_from_slice(&2u32.to_le_bytes());
        data[12] = 3;
        data[13..16].copy_from_slice(b"pkg");
        data[16..20].copy_from_slice(&24u32.to_le_bytes());
        data[24..28].copy_from_slice(&20u32.to_le_bytes());
        data[28..32].copy_from_slice(&3u32.to_le_bytes());
        data[32..36].copy_from_slice(&2u32.to_le_bytes());
        data[36..40].copy_from_slice(&0u32.to_le_bytes());
        data[40..44].copy_from_slice(&1u32.to_le_bytes());
        data[44..48].copy_from_slice(&[1, b'a', 1, b'b']);
        data[48..51].copy_from_slice(&[0x11, 0x22, 0x33]);

        let packages = named_package_resources(&data, 0, data.len());
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "pkg");
        assert_eq!(packages[0].resources.len(), 2);
        assert_eq!(packages[0].resources[0].data, [0x11]);
        assert_eq!(packages[0].resources[1].data, [0x22, 0x33]);
    }
}
