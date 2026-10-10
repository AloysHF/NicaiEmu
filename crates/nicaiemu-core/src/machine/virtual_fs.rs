//! Sandboxed in-memory filesystem exposed through the guest file manager.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// CoolBar system files the firmware treats as always-present: opening one
/// for read materialises an empty file instead of failing, so boot code that
/// expects to find (or create) its record store keeps going.
const GLUE_FILE_PREFIXES: &[&str] = &[
    "dfwsms",
    "dfwmix",
    "wpay",
    "cdlist",
    "cwstorecfg",
    "wstore_host",
    "coolbar_list",
    "downinfo3",
];

#[derive(Clone, Debug)]
struct VirtualFileHandle {
    path: String,
    position: usize,
    readable: bool,
    writable: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct VirtualFileSystem {
    files: HashMap<String, Vec<u8>>,
    directories: BTreeSet<String>,
    handles: Vec<Option<VirtualFileHandle>>,
    /// Lowercase relative path → real path for files that sit next to the
    /// loaded CBE (save records, CoolBar pay-kernel plugins, update data).
    /// The firmware resolves these through the device filesystem; the host
    /// directory stands in for it.
    host_files: HashMap<String, PathBuf>,
}

impl Default for VirtualFileSystem {
    fn default() -> Self {
        let mut directories = BTreeSet::new();
        directories.insert(String::new());
        Self {
            files: HashMap::new(),
            directories,
            handles: vec![None; 16],
            host_files: HashMap::new(),
        }
    }
}

impl VirtualFileSystem {
    /// Index every file under `dir` (the directory containing the loaded
    /// CBE) so guest opens can fall back to the sidecar the emulator was
    /// shipped with.  Keys are lowercase relative paths with `/` separators,
    /// matching `normalize_path`.
    pub(crate) fn set_host_dir(&mut self, dir: Option<&Path>) {
        self.host_files.clear();
        let Some(dir) = dir else {
            return;
        };
        let mut stack = vec![(dir.to_path_buf(), PathBuf::new())];
        // A runaway tree must not hang the loader; the depth limit only
        // truncates exotic layouts, sidecar directories are flat.
        let mut budget = 4096usize;
        while let Some((absolute, relative)) = stack.pop() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            let Ok(entries) = std::fs::read_dir(&absolute) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                let name = entry.file_name().to_string_lossy().into_owned();
                let rel = if relative.as_os_str().is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", relative.to_string_lossy(), name)
                };
                if file_type.is_dir() {
                    stack.push((entry.path(), PathBuf::from(&rel)));
                } else {
                    self.host_files.insert(rel.to_lowercase(), entry.path());
                }
            }
        }
    }

    /// Load a sidecar file that exists next to the CBE into the in-memory
    /// table.  Read-only: host bytes are never modified by guest writes.
    fn materialize_host_file(&self, path: &str) -> Option<Vec<u8>> {
        let host = self.host_files.get(path)?;
        std::fs::read(host).ok()
    }

    pub(crate) fn open(&mut self, path: &str, mode: &str, flags: u32) -> i32 {
        let Some(path) = normalize_path(path) else {
            return -1;
        };
        let mode = if mode.is_empty() {
            match flags {
                1 => "w",
                3 => "r+",
                value if value & 0x10 != 0 => "a+",
                value if value & 0x08 != 0 => "w+",
                value if value & 0x04 != 0 => "r+",
                _ => "r",
            }
        } else {
            mode
        };
        let readable = mode.starts_with('r') || mode.contains('+');
        let writable = mode.starts_with('w') || mode.starts_with('a') || mode.contains('+');
        // Non-truncating opens start from the host sidecar when the file
        // only exists next to the CBE (pay kernels, save records, update
        // data).  Truncating `w`/`w+` deliberately starts empty so guest
        // writes stay inside the sandbox.
        if !mode.starts_with('w') && !self.files.contains_key(&path) {
            if let Some(data) = self.materialize_host_file(&path) {
                self.files.insert(path.clone(), data);
            }
        }
        if mode.starts_with('r') && !self.files.contains_key(&path) {
            if !is_glue_file(&path) {
                return -1;
            }
            // Auto-materialise CoolBar glue files on first read-open.
            self.files.insert(path.clone(), Vec::new());
        }
        if mode.starts_with('w') {
            self.files.insert(path.clone(), Vec::new());
        } else if writable {
            self.files.entry(path.clone()).or_default();
        }
        let Some(handle) = self.handles.iter().position(Option::is_none) else {
            return -1;
        };
        let position = if mode.starts_with('a') {
            self.files.get(&path).map(Vec::len).unwrap_or(0)
        } else {
            0
        };
        self.handles[handle] = Some(VirtualFileHandle {
            path,
            position,
            readable,
            writable,
        });
        handle as i32
    }

    pub(crate) fn close(&mut self, handle: u32) -> i32 {
        let Some(slot) = self.handles.get_mut(handle as usize) else {
            return -1;
        };
        if slot.take().is_some() {
            0
        } else {
            -1
        }
    }

    pub(crate) fn read(&mut self, handle: u32, size: usize) -> Option<Vec<u8>> {
        let open = self.handles.get_mut(handle as usize)?.as_mut()?;
        if !open.readable {
            return None;
        }
        let file = self.files.get(&open.path)?;
        // A seek past the end is legal; reading there yields EOF (empty),
        // matching fread, instead of panicking on an inverted slice range.
        if open.position >= file.len() {
            return Some(Vec::new());
        }
        let end = open.position.saturating_add(size).min(file.len());
        let data = file[open.position..end].to_vec();
        open.position = end;
        Some(data)
    }

    pub(crate) fn write(&mut self, handle: u32, data: &[u8]) -> Option<usize> {
        let open = self.handles.get_mut(handle as usize)?.as_mut()?;
        if !open.writable {
            return None;
        }
        let file = self.files.get_mut(&open.path)?;
        if open.position > file.len() {
            file.resize(open.position, 0);
        }
        let end = open.position.checked_add(data.len())?;
        if end > file.len() {
            file.resize(end, 0);
        }
        file[open.position..end].copy_from_slice(data);
        open.position = end;
        Some(data.len())
    }

    pub(crate) fn seek(&mut self, handle: u32, offset: i32, origin: u32) -> Option<usize> {
        let open = self.handles.get_mut(handle as usize)?.as_mut()?;
        let base = match origin {
            0 => 0,
            1 => open.position,
            2 => self.files.get(&open.path)?.len(),
            _ => return None,
        };
        let position = (base as i64).checked_add(offset as i64)?;
        open.position = usize::try_from(position).ok()?;
        Some(open.position)
    }

    pub(crate) fn tell(&self, handle: u32) -> Option<usize> {
        self.handles
            .get(handle as usize)?
            .as_ref()
            .map(|open| open.position)
    }

    pub(crate) fn size(&self, handle: u32) -> Option<usize> {
        let open = self.handles.get(handle as usize)?.as_ref()?;
        self.files.get(&open.path).map(Vec::len)
    }

    pub(crate) fn file_exists(&self, path: &str) -> bool {
        normalize_path(path).is_some_and(|path| {
            self.files.contains_key(&path) || self.host_files.contains_key(&path)
        })
    }

    /// Read an entire file by path without allocating a handle.
    pub(crate) fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        let path = normalize_path(path)?;
        self.files.get(&path).cloned()
    }

    pub(crate) fn write_file(&mut self, path: &str, data: Vec<u8>) -> bool {
        let Some(path) = normalize_path(path) else {
            return false;
        };
        self.files.insert(path, data);
        true
    }

    pub(crate) fn directory_exists(&self, path: &str) -> bool {
        normalize_path(path).is_some_and(|path| {
            path.is_empty()
                || self.directories.contains(&path)
                || self.files.keys().any(|file| {
                    file.strip_prefix(&path)
                        .is_some_and(|tail| tail.starts_with('/'))
                })
        })
    }

    pub(crate) fn create_directory(&mut self, path: &str) -> bool {
        let Some(path) = normalize_path(path) else {
            return false;
        };
        let mut current = String::new();
        for component in path.split('/') {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(component);
            self.directories.insert(current.clone());
        }
        true
    }

    pub(crate) fn remove_file(&mut self, path: &str) -> bool {
        normalize_path(path).is_some_and(|path| self.files.remove(&path).is_some())
    }

    pub(crate) fn rename(&mut self, old_path: &str, new_path: &str) -> bool {
        let (Some(old_path), Some(new_path)) = (normalize_path(old_path), normalize_path(new_path))
        else {
            return false;
        };
        let Some(data) = self.files.remove(&old_path) else {
            return false;
        };
        self.files.insert(new_path.clone(), data);
        for open in self.handles.iter_mut().flatten() {
            if open.path == old_path {
                open.path = new_path.clone();
            }
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn file(&self, path: &str) -> Option<&[u8]> {
        let path = normalize_path(path)?;
        self.files.get(&path).map(Vec::as_slice)
    }

    pub(crate) fn file_count(&self) -> usize {
        self.files.len()
    }

    #[cfg(test)]
    pub(crate) fn paths(&self) -> Vec<(&str, usize)> {
        let mut paths: Vec<_> = self
            .files
            .iter()
            .map(|(path, data)| (path.as_str(), data.len()))
            .collect();
        paths.sort_unstable_by_key(|(path, _)| *path);
        paths
    }
}

fn is_glue_file(path: &str) -> bool {
    let base = path
        .rsplit('/')
        .next()
        .unwrap_or("")
        .rsplit('\\')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    GLUE_FILE_PREFIXES
        .iter()
        .any(|prefix| base.starts_with(prefix))
}

fn normalize_path(path: &str) -> Option<String> {
    let mut components = Vec::new();
    for component in path.replace('\\', "/").split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop()?;
            }
            value => components.push(value.to_lowercase()),
        }
    }
    Some(components.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_writes_and_seeks_with_normalized_paths() {
        let mut fs = VirtualFileSystem::default();
        assert!(fs.create_directory("./\\Game\\Data"));
        let handle = fs.open("game/data/FILE.bin", "w", 0);
        assert_eq!(handle, 0);
        assert_eq!(fs.write(handle as u32, &[1, 2, 3]), Some(3));
        assert_eq!(fs.seek(handle as u32, -2, 2), Some(1));
        assert_eq!(fs.write(handle as u32, &[4]), Some(1));
        assert_eq!(fs.close(handle as u32), 0);

        let handle = fs.open("GAME\\DATA\\file.BIN", "r", 0);
        assert_eq!(fs.read(handle as u32, 8), Some(vec![1, 4, 3]));
        assert_eq!(fs.read(handle as u32, 8), Some(Vec::new()));
        assert!(fs.directory_exists("game/data"));
    }

    #[test]
    fn rejects_paths_that_escape_the_virtual_root() {
        let mut fs = VirtualFileSystem::default();
        assert_eq!(fs.open("../outside", "w", 0), -1);
        assert!(!fs.create_directory("../../outside"));
    }

    #[test]
    fn read_past_the_end_returns_empty_like_fread() {
        let mut fs = VirtualFileSystem::default();
        let handle = fs.open("a.bin", "w+", 0);
        assert!(handle >= 0);
        assert_eq!(fs.write(handle as u32, b"1234"), Some(4));
        // A seek past the end is legal; reading there is EOF, not a panic.
        assert_eq!(fs.seek(handle as u32, 100, 0), Some(100));
        assert_eq!(fs.read(handle as u32, 16), Some(Vec::new()));
    }

    #[test]
    fn loads_sidecar_files_from_the_host_directory() {
        let dir = std::env::temp_dir().join(format!("nicaiemu-vfs-sidecar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("WpayKer10V100.CBM"), b"pay-kernel").unwrap();

        let mut fs = VirtualFileSystem::default();
        fs.set_host_dir(Some(&dir));
        // Case-insensitive match against the guest's lowercase path.
        assert!(fs.file_exists("./\\WpayKer10V100.CBM"));
        let handle = fs.open("./\\WpayKer10V100.CBM", "r", 0);
        assert!(handle >= 0);
        assert_eq!(fs.read(handle as u32, 64), Some(b"pay-kernel".to_vec()));
        // Guest writes stay in the sandbox; the host file is untouched.
        let rw = fs.open("./\\WpayKer10V100.CBM", "r+", 0);
        assert!(rw >= 0);
        assert_eq!(fs.write(rw as u32, b"corrupt"), Some(7));
        assert_eq!(
            std::fs::read(dir.join("WpayKer10V100.CBM")).unwrap(),
            b"pay-kernel"
        );
        // Truncating opens start empty even when a sidecar exists.
        let write = fs.open("wpayker10v100.cbm", "w+", 0);
        assert!(write >= 0);
        assert_eq!(fs.read(write as u32, 64), Some(Vec::new()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_sidecar_read_open_still_fails_without_glue() {
        let dir =
            std::env::temp_dir().join(format!("nicaiemu-vfs-sidecar-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut fs = VirtualFileSystem::default();
        fs.set_host_dir(Some(&dir));
        assert_eq!(fs.open("not-there.sav", "r", 0), -1);
        assert!(!fs.file_exists("not-there.sav"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
