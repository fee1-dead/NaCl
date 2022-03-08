use core::ptr::NonNull;

use stivale_boot::v2::StivaleStruct;

#[derive(Clone, Copy)]
pub struct Mapper {
    pub physical_memory_offset: usize,
}

impl Mapper {
    #[inline]
    pub fn new(boot_info: &StivaleStruct) -> Self {
        Self {
            physical_memory_offset: boot_info.vmap().unwrap().address as usize,
        }
    }

    #[inline]
    pub fn phys_to_virt(&self, phys: usize) -> usize {
        self.physical_memory_offset + phys
    }

    #[inline]
    pub fn phys_to_virt_ptr(&self, phys: usize) -> NonNull<u8> {
        NonNull::new(self.phys_to_virt(phys) as *mut u8).unwrap()
    }
}

impl acpi::AcpiHandler for Mapper {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        acpi::PhysicalMapping::new(
            physical_address,
            NonNull::new_unchecked((self.physical_memory_offset + physical_address) as *mut _),
            size,
            size,
            *self,
        )
    }

    // No-op since we don't remove entries.
    fn unmap_physical_region<T>(_: &acpi::PhysicalMapping<Self, T>) {}
}
