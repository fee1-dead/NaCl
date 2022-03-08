use raw_cpuid::CpuId;
use stivale_boot::v2::StivaleStruct;

mod acpi;
mod apic;
mod boot;
mod gdt;
mod interrupts;
mod memory;
mod smp;
mod time;

pub use memory::init as memory_init;
pub use time::delay;

use self::memory::mapper::Mapper;
use crate::hlt_loop;

pub extern "C" fn start(boot_info: &'static StivaleStruct) -> ! {
    init(boot_info);
    hlt_loop();
}

pub fn init(boot_info: &'static StivaleStruct) {
    gdt::init();
    interrupts::init_idt();

    let mapper = Mapper::new(boot_info);
    let tables = acpi::get_acpi_tables(boot_info, mapper);
    let platform_info = acpi::get_platform_info(&tables);
    apic::init_and_disable_old_pic();
    apic::init_lapic(&platform_info, &mapper);
    let (ioapic, pitreg) = apic::init_ioapic(&platform_info, &mapper);
    time::init(ioapic, pitreg);
    // smp::init(&platform_info);

    x86_64::instructions::interrupts::enable();
}

/// Returns a unique identifying number of this processor.
pub fn id() -> u8 {
    // N.B. some processor specific models allow changin the LAPIC ID.
    // initial lapic id is immutable and thus suitable for a unique identifier
    // for processors.
    CpuId::new()
        .get_feature_info()
        .unwrap()
        .initial_local_apic_id()
}
