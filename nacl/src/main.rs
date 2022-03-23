#![feature(abi_x86_interrupt)]
#![feature(abi_efiapi)]
#![feature(alloc_error_handler)]
#![feature(array_from_fn)]
#![feature(asm_sym, asm_const)]
#![feature(bigint_helper_methods)]
#![feature(cell_update)]
#![feature(const_mut_refs)]
#![feature(decl_macro)]
#![feature(format_args_nl)]
#![feature(fn_align)]
#![feature(once_cell)]
#![feature(type_alias_impl_trait)]
#![feature(naked_functions)]
#![no_std]
#![no_main]
#![allow(clippy::missing_safety_doc)] // TODO remove this later

extern crate alloc;

pub mod arch;
pub mod cores;
pub mod font;
pub mod serial;
pub mod task;

use core::arch::asm;
use core::mem;
use core::panic::PanicInfo;
use core::time::Duration;

use stivale_boot::v2::{
    Stivale5LevelPagingHeaderTag, StivaleFramebufferHeaderTag, StivaleHeader, StivaleSmpHeaderTag,
    StivaleStruct, StivaleUnmapNullHeaderTag,
};
use x86_64::structures::gdt::DescriptorFlags;
use x86_64::VirtAddr;

use crate::arch::delay;
use crate::cores::id;
use crate::font::FrameBufferManager;

#[repr(C, align(4096))]
pub struct PageAligned<T>(pub T);

// 32 KiB of stack
const STACK_SIZE: usize = 32 * 1024;

static STACK: PageAligned<[u8; STACK_SIZE]> = PageAligned([0; STACK_SIZE]);

#[link_section = ".stivale2hdr"]
#[used]
pub static HEADER: StivaleHeader = StivaleHeader::new()
    //    .entry_point(kernel_start)
    // "How to write a library that only works with your own intended usage"
    // TODO fork the library and make it work with this
    .stack(STACK.0.as_ptr())
    .tags({
        macro t($NAME:ident) {{
            &$NAME as *const _ as *const ()
        }}
        static SMP: StivaleSmpHeaderTag = StivaleSmpHeaderTag::new();
        static UNMAP_NULL: StivaleUnmapNullHeaderTag =
            StivaleUnmapNullHeaderTag::new().next(t!(SMP));
        static LEVEL_5_PAGING: Stivale5LevelPagingHeaderTag =
            Stivale5LevelPagingHeaderTag::new().next(t!(UNMAP_NULL));
        static FRAMEBUFFER: StivaleFramebufferHeaderTag =
            StivaleFramebufferHeaderTag::new().next(t!(LEVEL_5_PAGING));

        t!(FRAMEBUFFER)
    });

#[no_mangle]
pub extern "C" fn kernel_start(boot_info: &'static mut StivaleStruct) -> ! {
    init(boot_info);

    sprintln!("ok");

    println!("NaCl v{}", env!("CARGO_PKG_VERSION"));
    println!("Ayo");

    loop {
        delay(Duration::from_secs(1));
        println!("SEC!");
    }
}

fn init(boot_info: &'static mut StivaleStruct) {
    let phys_mem_offset = VirtAddr::new(boot_info.vmap().expect("expected vmap tag").address);
    // SAFETY: provided via boot_info, so it is correct
    unsafe {
        crate::arch::memory_init(phys_mem_offset, boot_info.memory_map().unwrap());
    }

    let frame_buffer = boot_info.framebuffer().unwrap();
    let mut fb = FrameBufferManager::new(frame_buffer);
    sprintln!("{fb:?}");
    fb.putchar('F', 0, 0, 0xFFFFFF, 0);
    font::insert_fbman(fb);

    // initialize per-core memory access.
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
