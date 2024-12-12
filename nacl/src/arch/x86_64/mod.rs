use stivale_boot::v2::StivaleStruct;

mod acpi;
pub mod apic;
mod boot;
mod gdt;
mod interrupts;
mod memory;
// mod smp;
mod time;

pub use memory::init as memory_init;
pub use time::delay;

use crate::sprintln;

use self::memory::mapper::Mapper;


pub fn init(physical_memory_offset: usize, rsdp_addr: usize) {
    gdt::init();
    interrupts::init_idt();

    let mapper = Mapper::new(physical_memory_offset);
    let tables = acpi::get_acpi_tables(rsdp_addr, mapper);
    let platform_info = acpi::get_platform_info(&tables);
    apic::init_and_disable_old_pic();
    apic::init_lapic(&platform_info, &mapper);

    let (ioapic, pitreg) = apic::init_ioapic(&platform_info, &mapper);
    time::init(ioapic, pitreg);
    // smp::init(&platform_info, boot_info);

    x86_64::instructions::interrupts::enable();
}
