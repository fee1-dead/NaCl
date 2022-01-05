use acpi::{AcpiTables, PlatformInfo};
use bootloader::BootInfo;

use crate::memory::mapper::Mapper;
use crate::sprintln;

pub type Tables = AcpiTables<Mapper>;

pub fn get_acpi_tables(boot_info: &BootInfo, mapper: Mapper) -> Tables {
    let rsdp = boot_info.rsdp_addr.into_option().unwrap();
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
