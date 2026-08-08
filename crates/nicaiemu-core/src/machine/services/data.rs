//! Data-package and image-package services (group 21).

use armv4t_emu::Memory;
use log::{debug, warn};

use super::super::{
    NicaiMachine, DATA_PACKAGE_SIZE, DREAM_FACTORY_PACKAGE_SLOT, SERVICE_BASE, TABLE_STRIDE,
};

impl NicaiMachine {
    pub(crate) fn handle_data_package_service(&mut self, index: u32) {
        let package = self.register(0);
        match index {
            0 => {
                if self.register(1) == 0 {
                    self.memory.w8(package, 0);
                    self.set_result(0);
                } else {
                    self.set_result(0);
                }
            }
            4 => {
                self.load_main_resource_package(package);
                self.set_result(0);
            }
            5 => self.set_result(package),
            6 => {
                let result = self.resource_by_name(self.register(1));
                self.set_result(result);
            }
            7 => {
                let result = self.resource_by_id(self.register(1));
                self.set_result(result);
            }
            8 => {
                let result = self.resource_name_by_id(self.register(1));
                self.set_result(result);
            }
            9 => {
                let result = self.resource_id_by_name(self.register(1));
                self.set_result(result.unwrap_or(u32::MAX));
            }
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

                let name = self.allocate(resource.name.len() as u32 + 1);
                self.memory.write_bytes(name, resource.name.as_bytes());
                self.memory.w8(name + resource.name.len() as u32, 0);
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

    pub(crate) fn resource_by_id(&self, id: u32) -> u32 {
        self.resource_data.get(id as usize).copied().unwrap_or(0)
    }

    pub(crate) fn resource_name_by_id(&self, id: u32) -> u32 {
        self.resource_names.get(id as usize).copied().unwrap_or(0)
    }

    pub(crate) fn resource_by_name(&mut self, name: u32) -> u32 {
        let Some(id) = self.resource_id_by_name(name) else {
            return 0;
        };
        self.resource_by_id(id)
    }

    pub(crate) fn resource_id_by_name(&mut self, address: u32) -> Option<u32> {
        let name = self.read_c_string(address, 256);
        self.resources
            .iter()
            .position(|resource| resource.name.eq_ignore_ascii_case(&name))
            .map(|index| index as u32)
    }
}
