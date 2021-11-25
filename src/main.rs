#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(bigint_helper_methods)]
#![feature(once_cell)]
#![feature(type_alias_impl_trait)]
#![no_std]
#![no_main]

extern crate alloc;

pub mod acpi;
pub mod font;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod serial;
pub mod task;

use core::panic::PanicInfo;

use bootloader::{entry_point, BootInfo};
use x86_64::VirtAddr;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    init(boot_info);

    sprintln!("sfOS v{}", env!("CARGO_PKG_VERSION"));

    acpi::print_acpi_tables(boot_info.physical_memory_offset.into_option().unwrap());

    let cpuid = raw_cpuid::CpuId::new();

    sprintln!("{:#?}", cpuid);

    hlt_loop()
}

fn init(boot_info: &'static BootInfo) {
    interrupts::init_idt();
    gdt::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    // SAFETY: provided via boot_info, so it is correct
    unsafe {
        memory::init(phys_mem_offset, &boot_info.memory_regions);
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    sprintln!("{}", info);
    hlt_loop()
}

/// Do not execute until the next interrupt. Makes CPU work less harder.
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt()
    }
}
