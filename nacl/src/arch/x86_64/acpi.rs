use acpi::{AcpiTables, PlatformInfo};

use super::memory::mapper::Mapper;
use crate::sprintln;

pub type Tables = AcpiTables<Mapper>;

pub fn get_acpi_tables(rsdp_addr: usize, mapper: Mapper) -> Tables {
    unsafe { acpi::AcpiTables::from_rsdp(mapper, rsdp_addr) }.expect("ACPI tables")
}

pub fn get_platform_info(tables: &Tables) -> PlatformInfo {
    let info = PlatformInfo::new(tables).expect("platform info");
    if let Some(processors) = &info.processor_info {
        sprintln!("boot processor: {:#?}", processors.boot_processor);
        sprintln!(
            "application processors: {:#?}",
            processors.application_processors
        );
    }
    info
}
