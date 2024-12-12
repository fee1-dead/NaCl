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

use self::memory::mapper::Mapper;
use crate::hlt_loop;

pub extern "C" fn start(boot_info: &'static mut StivaleStruct) -> ! {
    init(boot_info);
    hlt_loop();
}

pub fn init(boot_info: &'static mut StivaleStruct) {
    gdt::init();
    interrupts::init_idt();

    let mapper = Mapper::new(boot_info);
    let tables = acpi::get_acpi_tables(boot_info, mapper);
    let platform_info = acpi::get_platform_info(&tables);
    apic::init_and_disable_old_pic();
    apic::init_lapic(&platform_info, &mapper);
    let (ioapic, pitreg) = apic::init_ioapic(&platform_info, &mapper);
    time::init(ioapic, pitreg);
    // smp::init(&platform_info, boot_info);

    x86_64::instructions::interrupts::enable();
}
