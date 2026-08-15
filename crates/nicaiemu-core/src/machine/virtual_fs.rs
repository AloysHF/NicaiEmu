//! Sandboxed in-memory filesystem exposed through the guest file manager.

use std::collections::{BTreeSet, HashMap};

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
}

impl Default for VirtualFileSystem {
    fn default() -> Self {
        let mut directories = BTreeSet::new();
        directories.insert(String::new());
        Self {
            files: HashMap::new(),
            directories,
            handles: vec![None; 16],
        }
    }
}

impl VirtualFileSystem {
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
        if mode.starts_with('r') && !self.files.contains_key(&path) {
            return -1;
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
        normalize_path(path).is_some_and(|path| self.files.contains_key(&path))
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
}
