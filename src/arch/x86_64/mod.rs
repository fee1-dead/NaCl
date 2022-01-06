use bootloader::BootInfo;
use raw_cpuid::CpuId;

use crate::memory::mapper::Mapper;

mod acpi;
mod apic;
mod gdt;
mod interrupts;
mod time;

pub fn init(boot_info: &'static BootInfo) {
    gdt::init();
    interrupts::init_idt();

    let mapper = Mapper::new(boot_info);
    let tables = acpi::get_acpi_tables(boot_info, mapper);
    let platform_info = acpi::get_platform_info(&tables);
    apic::init_and_disable_old_pic();
    apic::init_lapic(&platform_info, &mapper);
    let (ioapic, pitreg) = apic::init_ioapic(&platform_info, &mapper);
    time::init(ioapic, pitreg);

    x86_64::instructions::interrupts::enable();
}

/// Returns a unique identifying number of this processor.
pub fn id() -> u8 {
    // N.B. some processor specific models allow changin the LAPIC ID.
    // initial lapic id is immutable and thus suitable for a unique identifier
    // for processors.
    CpuId::new().get_feature_info().unwrap().initial_local_apic_id()
}