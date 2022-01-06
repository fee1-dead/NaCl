#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(array_from_fn)]
#![feature(bigint_helper_methods)]
#![feature(decl_macro)]
#![feature(format_args_nl)]
#![feature(once_cell)]
#![feature(type_alias_impl_trait)]
#![no_std]
#![no_main]
#![allow(clippy::missing_safety_doc)] // TODO remove this later

extern crate alloc;

pub mod arch;
pub mod cores;
pub mod font;
pub mod memory;
pub mod serial;
pub mod task;

use core::mem;
use core::panic::PanicInfo;
use core::time::Duration;

use bootloader::boot_info::Optional;
use bootloader::{entry_point, BootInfo};
use x86_64::VirtAddr;

use crate::arch::delay;
use crate::font::FrameBufferManager;

entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    init(boot_info);

    println!("sfOS v{}", env!("CARGO_PKG_VERSION"));
    println!("Ayo");

    loop {
        delay(Duration::from_secs(1));
        println!("SEC!");
    }
}

fn init(boot_info: &'static mut BootInfo) {
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    // SAFETY: provided via boot_info, so it is correct
    unsafe {
        memory::init(phys_mem_offset, &boot_info.memory_regions);
    }

    let mut frame_buffer = Optional::None;
    mem::swap(&mut boot_info.framebuffer, &mut frame_buffer);
    let fb = FrameBufferManager::new(frame_buffer.into_option().unwrap());
    font::insert_fbman(fb);

    // initialize per-core memory access.
    crate::cores::init();
    crate::arch::init(boot_info);
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
