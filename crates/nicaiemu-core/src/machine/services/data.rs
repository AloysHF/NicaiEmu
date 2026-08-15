//! Data-package and image-package services (group 21).

use armv4t_emu::Memory;
use encoding_rs::GBK;
use log::{debug, warn};

use super::super::{
    packages::HostResource, NicaiMachine, DATA_PACKAGE_SIZE, DREAM_FACTORY_PACKAGE_SLOT,
    SERVICE_BASE, TABLE_STRIDE,
};

impl NicaiMachine {
    pub(crate) fn handle_data_package_service(&mut self, index: u32) {
        let package = self.register(0);
        match index {
            0 => {
                let name = self.register(1);
                if name == 0 {
                    self.memory.w8(package, 0);
                    self.set_result(0);
                } else {
                    let result = self.add_data_package(package, name);
                    self.set_result(result);
                }
            }
            1 => {
                let result = self.release_data_package(package, self.register(1));
                self.set_result(result);
            }
            4 => {
                self.load_main_resource_package(package);
                self.set_result(0);
            }
            5 => {
                let result = self.locate_data_package(package, self.register(1));
                self.set_result(result);
            }
            6 => {
                let result = self.package_resource_by_name(package, self.register(1));
                self.set_result(result);
            }
            7 => {
                let result = self.package_resource_by_id(package, self.register(1));
                self.set_result(result);
            }
            8 => {
                let result = self.package_resource_name_by_id(package, self.register(1));
                self.set_result(result);
            }
            9 => {
                let result = self.package_resource_id_by_name(package, self.register(1));
                self.set_result(result.unwrap_or(u32::MAX));
            }
            10 => self.set_result(0),
            _ => self.set_result(0),
        }
    }

    pub(crate) fn initialize_data_package(&mut self, package: u32, capacity: u32) {
        for offset in [4, 8, 12, 16, 24, 28] {
            self.memory.w32(package + offset, 0);
        }
        self.memory.w8(package, 1);
        let entries = self.allocate(capacity.saturating_mul(4).max(4));
        self.memory.w32(package + 28, entries);
        self.memory.w32(package + 92, u32::MAX);
        self.memory.w32(package + 100, 0);
        for index in 0..=10 {
            self.memory.w32(
                package + 32 + index * 4,
                SERVICE_BASE + TABLE_STRIDE * 21 + index * 4,
            );
        }
        self.memory
            .w32(package + 80, SERVICE_BASE + TABLE_STRIDE * 21 + 11 * 4);
        self.set_result(SERVICE_BASE + TABLE_STRIDE * 21 + 10 * 4);
    }

    pub(crate) fn load_main_resource_package(&mut self, package: u32) {
        if !self.resource_packages.is_empty() {
            let packages = self.resource_packages.clone();
            let child_count = self.memory.r16(package + 10) as u32;
            let child_table = self.memory.r32(package + 28);
            let mut loaded = 0usize;
            for slot in 0..child_count {
                let child = self.memory.r32(child_table + slot * 4);
                if child == 0 || self.memory.r8(child) != 0 {
                    continue;
                }
                let name_address = self.memory.r32(child + 4);
                let name = self.read_gbk_string(name_address, 256);
                let Some(host_package) = packages
                    .iter()
                    .find(|candidate| candidate.name.eq_ignore_ascii_case(&name))
                else {
                    warn!("CBE resource package {name:?} was not found");
                    continue;
                };
                let first_id = (self.memory.r8(child + 1) as u32) << 8;
                self.populate_guest_resource_package(child, &host_package.resources, first_id);
                self.memory.w8(child, 1);
                loaded += 1;
            }
            debug!("loaded {loaded} named CBE resource packages into guest memory");
            return;
        }
        if self.resources.is_empty() {
            warn!("CBE main resource package is empty");
            return;
        }
        if self.resource_data.is_empty() {
            let resources = self.resources.clone();
            self.resource_data.reserve(resources.len());
            self.resource_names.reserve(resources.len());
            for resource in resources {
                let data = self.allocate(resource.data.len().max(1) as u32);
                self.memory.write_bytes(data, &resource.data);
                self.resource_data.push(data);

                let name = self.allocate_gbk_string(&resource.name);
                self.resource_names.push(name);
            }
        }

        let count = self.resources.len().min(u16::MAX as usize);
        let names = self.allocate((count * 4) as u32);
        let offsets = self.allocate((count * 4) as u32);
        let ids = self.allocate((count * 2) as u32);
        for index in 0..count {
            self.memory
                .w32(names + index as u32 * 4, self.resource_names[index]);
            self.memory
                .w32(offsets + index as u32 * 4, self.resource_data[index]);
            self.memory.w16(ids + index as u32 * 2, index as u16);
        }
        self.memory.w8(package, 1);
        self.memory.w16(package + 8, count as u16);
        self.memory.w32(package + 12, names);
        self.memory.w32(package + 16, offsets);
        self.memory.w32(package + 20, ids);
        self.memory.w32(package + 24, 0);
        self.memory.w32(package + 96, 0);
        debug!("loaded {count} CBE resources into guest memory");
    }

    fn populate_guest_resource_package(
        &mut self,
        package: u32,
        resources: &[HostResource],
        first_id: u32,
    ) {
        let count = resources.len().min(u16::MAX as usize);
        let names = self.allocate((count * 4).max(4) as u32);
        let data = self.allocate((count * 4).max(4) as u32);
        let ids = self.allocate((count * 2).max(2) as u32);
        let data_size = resources.iter().take(count).fold(0u32, |size, resource| {
            size.saturating_add(resource.data.len() as u32)
        });
        let data_block = self.allocate(data_size.max(1));
        let mut data_offset = 0u32;
        for (index, resource) in resources.iter().take(count).enumerate() {
            let resource_data = data_block.saturating_add(data_offset);
            self.memory.write_bytes(resource_data, &resource.data);
            data_offset = data_offset.saturating_add(resource.data.len() as u32);
            let resource_name = self.allocate_gbk_string(&resource.name);
            self.memory.w32(names + index as u32 * 4, resource_name);
            self.memory.w32(data + index as u32 * 4, resource_data);
            self.memory.w16(
                ids + index as u32 * 2,
                first_id.saturating_add(index as u32) as u16,
            );
        }
        self.memory.w16(package + 8, count as u16);
        self.memory.w32(package + 12, names);
        self.memory.w32(package + 16, data);
        self.memory.w32(package + 20, ids);
    }

    fn add_data_package(&mut self, package: u32, name: u32) -> u32 {
        let mut count = self.memory.r16(package + 10) as u32;
        let mut table = self.memory.r32(package + 28);
        if table == 0 {
            table = self.allocate(4);
            self.memory.w32(package + 28, table);
        }
        let slot = (0..count)
            .find(|index| self.memory.r32(table + index * 4) == 0)
            .unwrap_or(count);
        if slot == count {
            count += 1;
            self.memory.w16(package + 10, count as u16);
        }

        let child = self.allocate(DATA_PACKAGE_SIZE);
        self.initialize_data_package(child, 0);
        let package_name = self.read_gbk_string(name, 256);
        let name_copy = self.allocate_gbk_string(&package_name);
        self.memory.w8(child, 0);
        self.memory.w8(child + 1, (slot + 1) as u8);
        self.memory.w32(child + 4, name_copy);
        self.memory.w32(table + slot * 4, child);
        slot + 1
    }

    fn release_data_package(&mut self, package: u32, name: u32) -> u32 {
        let requested = (name != 0).then(|| self.read_gbk_string(name, 256));
        let count = self.memory.r16(package + 10) as u32;
        let table = self.memory.r32(package + 28);
        for slot in 0..count {
            let child_address = table + slot * 4;
            let child = self.memory.r32(child_address);
            if child == 0 {
                continue;
            }
            let child_name_address = self.memory.r32(child + 4);
            let child_name = self.read_gbk_string(child_name_address, 256);
            if requested
                .as_ref()
                .is_none_or(|value| value.eq_ignore_ascii_case(&child_name))
            {
                self.memory.w32(child_address, 0);
            }
        }
        count
    }

    fn locate_data_package(&mut self, package: u32, name: u32) -> u32 {
        if name == 0 {
            return package;
        }
        let requested = self.read_gbk_string(name, 256);
        let count = self.memory.r16(package + 10) as u32;
        let table = self.memory.r32(package + 28);
        for index in 0..count {
            let child = self.memory.r32(table + index * 4);
            if child == 0 {
                continue;
            }
            let child_name_address = self.memory.r32(child + 4);
            let child_name = self.read_gbk_string(child_name_address, 256);
            if child_name.eq_ignore_ascii_case(&requested) {
                return child;
            }
        }
        0
    }

    fn package_resource_by_id(&mut self, package: u32, id: u32) -> u32 {
        let count = self.memory.r16(package + 8) as u32;
        let ids = self.memory.r32(package + 20);
        let data = self.memory.r32(package + 16);
        for index in 0..count {
            if self.memory.r16(ids + index * 2) as u32 == id {
                if self.memory.r8(package + 84) != 0 {
                    return self.read_file_backed_package_resource(package, data, count, index);
                }
                return self
                    .memory
                    .r32(data + index * 4)
                    .wrapping_add(self.memory.r32(package + 24));
            }
        }
        let child_count = self.memory.r16(package + 10) as u32;
        let children = self.memory.r32(package + 28);
        for index in 0..child_count {
            let child = self.memory.r32(children + index * 4);
            if child == 0 {
                continue;
            }
            let result = self.package_resource_by_id(child, id);
            if result != 0 {
                return result;
            }
        }
        0
    }

    fn read_file_backed_package_resource(
        &mut self,
        package: u32,
        data: u32,
        count: u32,
        index: u32,
    ) -> u32 {
        let start = self.memory.r32(data + index * 4);
        let end = if index + 1 < count {
            self.memory.r32(data + (index + 1) * 4)
        } else {
            self.memory.r32(package + 96)
        };
        let Some(size) = end.checked_sub(start) else {
            return 0;
        };
        let Some(offset) = self.memory.r32(package + 88).checked_add(start) else {
            return 0;
        };
        let Ok(offset) = i32::try_from(offset) else {
            return 0;
        };
        let handle = self.memory.r32(package + 92);
        if self.virtual_fs.seek(handle, offset, 0).is_none() {
            return 0;
        }
        let Some(bytes) = self.virtual_fs.read(handle, size as usize) else {
            return 0;
        };
        if bytes.len() != size as usize {
            return 0;
        }
        let output = self.allocate(size.max(1));
        if output != 0 {
            self.memory.write_bytes(output, &bytes);
        }
        output
    }

    fn package_resource_name_by_id(&mut self, package: u32, id: u32) -> u32 {
        let count = self.memory.r16(package + 8) as u32;
        let ids = self.memory.r32(package + 20);
        let names = self.memory.r32(package + 12);
        for index in 0..count {
            if self.memory.r16(ids + index * 2) as u32 == id {
                return self.memory.r32(names + index * 4);
            }
        }
        let child_count = self.memory.r16(package + 10) as u32;
        let children = self.memory.r32(package + 28);
        for index in 0..child_count {
            let child = self.memory.r32(children + index * 4);
            if child == 0 {
                continue;
            }
            let result = self.package_resource_name_by_id(child, id);
            if result != 0 {
                return result;
            }
        }
        0
    }

    fn package_resource_id_by_name(&mut self, package: u32, name: u32) -> Option<u32> {
        let requested = self.read_gbk_string(name, 256);
        self.package_resource_id_by_decoded_name(package, &requested)
    }

    fn package_resource_id_by_decoded_name(
        &mut self,
        package: u32,
        requested: &str,
    ) -> Option<u32> {
        let count = self.memory.r16(package + 8) as u32;
        let ids = self.memory.r32(package + 20);
        let names = self.memory.r32(package + 12);
        for index in 0..count {
            let candidate_address = self.memory.r32(names + index * 4);
            let candidate = self.read_gbk_string(candidate_address, 256);
            if resource_names_match(&candidate, requested) {
                return Some(self.memory.r16(ids + index * 2) as u32);
            }
        }
        let child_count = self.memory.r16(package + 10) as u32;
        let children = self.memory.r32(package + 28);
        for index in 0..child_count {
            let child = self.memory.r32(children + index * 4);
            if child == 0 {
                continue;
            }
            if let Some(id) = self.package_resource_id_by_decoded_name(child, requested) {
                return Some(id);
            }
        }
        None
    }

    fn package_resource_by_name(&mut self, package: u32, name: u32) -> u32 {
        self.package_resource_id_by_name(package, name)
            .map(|id| self.package_resource_by_id(package, id))
            .unwrap_or(0)
    }

    pub(crate) fn ensure_image_package(&mut self, inner: bool) -> u32 {
        let existing = if inner {
            self.inner_image_package
        } else {
            self.app_image_package
        };
        if existing != 0 {
            return existing;
        }

        let package = self.allocate(DATA_PACKAGE_SIZE);
        if inner {
            self.inner_image_package = package;
        } else {
            self.app_image_package = package;
        }
        package
    }

    pub(crate) fn initialize_image_data_page(&mut self, inner: bool) -> u32 {
        let package = self.ensure_image_package(inner);
        if package == 0 {
            return 0;
        }
        self.current_image_package = package;
        self.memory.w32(DREAM_FACTORY_PACKAGE_SLOT, package);
        let count = self.memory.r16(package + 8) as u32;
        if count != 0 {
            return count;
        }
        self.initialize_data_package(package, 5);
        self.load_main_resource_package(package);
        self.memory.r16(package + 8) as u32
    }

    pub(crate) fn create_image_from_data_package(
        &mut self,
        image_id: u32,
        package: u32,
        output: u32,
    ) -> u32 {
        if package == 0 || image_id >= self.memory.r16(package + 8) as u32 {
            return 0;
        }
        self.create_image_from_resource_index(image_id as usize, output)
    }

    pub(crate) fn resource_by_id(&mut self, id: u32) -> u32 {
        let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
        if package != 0 {
            let result = self.package_resource_by_id(package, id);
            if result != 0 {
                return result;
            }
        }
        self.resource_data.get(id as usize).copied().unwrap_or(0)
    }

    pub(crate) fn resource_name_by_id(&mut self, id: u32) -> u32 {
        let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
        if package != 0 {
            let result = self.package_resource_name_by_id(package, id);
            if result != 0 {
                return result;
            }
        }
        self.resource_names.get(id as usize).copied().unwrap_or(0)
    }

    pub(crate) fn resource_by_name(&mut self, name: u32) -> u32 {
        let Some(id) = self.resource_id_by_name(name) else {
            return 0;
        };
        self.resource_by_id(id)
    }

    pub(crate) fn resource_id_by_name(&mut self, address: u32) -> Option<u32> {
        let name = self.read_gbk_string(address, 256);
        let package = self.memory.r32(DREAM_FACTORY_PACKAGE_SLOT);
        if package != 0 {
            if let Some(id) = self.package_resource_id_by_decoded_name(package, &name) {
                return Some(id);
            }
        }
        self.resources
            .iter()
            .position(|resource| resource.name.eq_ignore_ascii_case(&name))
            .map(|index| index as u32)
    }

    fn allocate_gbk_string(&mut self, value: &str) -> u32 {
        let (encoded, _, _) = GBK.encode(value);
        let address = self.allocate(encoded.len() as u32 + 1);
        self.memory.write_bytes(address, &encoded);
        self.memory.w8(address + encoded.len() as u32, 0);
        address
    }
}

fn resource_names_match(left: &str, right: &str) -> bool {
    if left.eq_ignore_ascii_case(right) {
        return true;
    }
    let left = left.rsplit(['/', '\\']).next().unwrap_or(left);
    let right = right.rsplit(['/', '\\']).next().unwrap_or(right);
    left.eq_ignore_ascii_case(right)
}
