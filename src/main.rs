#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(bigint_helper_methods)]
#![feature(decl_macro)]
#![feature(format_args_nl)]
#![feature(once_cell)]
#![feature(type_alias_impl_trait)]
#![no_std]
#![no_main]
#![allow(clippy::missing_safety_doc)] // TODO remove this later

extern crate alloc;

pub mod acpi;
pub mod apic;
pub mod font;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod serial;
pub mod task;

use core::mem;
use core::panic::PanicInfo;

use bootloader::boot_info::Optional;
use bootloader::{entry_point, BootInfo};
use x86_64::VirtAddr;

use crate::font::FrameBufferManager;
use crate::memory::mapper::Mapper;

entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    init(boot_info);

    println!("sfOS v{}", env!("CARGO_PKG_VERSION"));
    println!("Ayo");

    hlt_loop()
}

fn init(boot_info: &'static mut BootInfo) {
    gdt::init();
    interrupts::init_idt();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    // SAFETY: provided via boot_info, so it is correct
    unsafe {
        memory::init(phys_mem_offset, &boot_info.memory_regions);
    }

    let mut frame_buffer = Optional::None;
    mem::swap(&mut boot_info.framebuffer, &mut frame_buffer);
    let fb = FrameBufferManager::new(frame_buffer.into_option().unwrap());
    font::insert_fbman(fb);

    let mapper = Mapper::new(boot_info);
    let tables = acpi::get_acpi_tables(boot_info, mapper);
    let platform_info = acpi::get_platform_info(&tables);
    apic::init_and_disable_old_pic();
    apic::init_lapic(&platform_info, &mapper);
    apic::init_ioapic(&platform_info, &mapper);

    x86_64::instructions::interrupts::enable();
}

#[panic_handler]
pub fn panic_handler(info: &PanicInfo) -> ! {
    sprintln!("{}", info);
    hlt_loop()
}

/// Do not execute until the next interrupt. Makes CPU work less harder.
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt()
    }
}
