use acpi::{AcpiTables, PlatformInfo};
use stivale_boot::v2::StivaleStruct;

use super::memory::mapper::Mapper;
use crate::sprintln;

pub type Tables = AcpiTables<Mapper>;

pub fn get_acpi_tables(boot_info: &StivaleStruct, mapper: Mapper) -> Tables {
    let rsdp = boot_info.rsdp().unwrap().rsdp;
    unsafe { acpi::AcpiTables::from_rsdp(mapper, rsdp as usize) }.expect("ACPI tables")
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
