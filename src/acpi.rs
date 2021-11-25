use core::ops::Add;
use core::ptr::NonNull;

use acpi::{AcpiTables, PlatformInfo};
use x86_64::VirtAddr;

use crate::sprintln;

#[derive(Clone, Copy)]
struct Handler {
    physmemoff: u64,
}

impl acpi::AcpiHandler for Handler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        acpi::PhysicalMapping::new(
            physical_address,
            NonNull::new_unchecked((self.physmemoff + physical_address as u64) as *mut _),
            size,
            size,
            *self,
        )
    }

    // No-op since we don't remove entries.
    fn unmap_physical_region<T>(_: &acpi::PhysicalMapping<Self, T>) {}
}

pub fn print_acpi_tables(physmemoff: u64) {
    let tables = unsafe { acpi::AcpiTables::search_for_rsdp_bios(Handler { physmemoff }) }
        .expect("ACPI tables");
    let info = PlatformInfo::new(&tables).expect("platform info");
    if let Some(processors) = info.processor_info {
        sprintln!("boot processor: {:?}", processors.boot_processor);
        sprintln!(
            "application processors: {:?}",
            processors.application_processors
        );
    }
}
