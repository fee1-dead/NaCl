#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(bigint_helper_methods)]
#![feature(cell_update)]
#![feature(decl_macro)]
#![feature(format_args_nl)]
#![feature(fn_align)]
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

use core::panic::PanicInfo;

use limine::paging::Mode;

use log::Log;
use x86_64::VirtAddr;

use crate::cores::cpu;
use crate::font::FrameBufferManager;

#[repr(C, align(4096))]
pub struct PageAligned<T>(pub T);

// 32 KiB of stack
const STACK_SIZE: u64 = 32 * 1024;

use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, PagingModeRequest, RequestsEndMarker, RequestsStartMarker, RsdpRequest, StackSizeRequest
};
use limine::BaseRevision;

/// Sets the base revision to the latest revision supported by the crate.
/// See specification for further info.
/// Be sure to mark all limine requests with #[used], otherwise they may be removed by the compiler.
#[used]
// The .requests section allows limine to find the requests faster and more safely.
#[link_section = ".requests"]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[link_section = ".requests"]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[link_section = ".requests"]
static PAGING_MODE_REQUEST: PagingModeRequest =
    PagingModeRequest::new().with_mode(Mode::FIVE_LEVEL);

#[used]
#[link_section = ".requests"]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(STACK_SIZE);

#[used]
#[link_section = ".requests"]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[link_section = ".requests"]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[link_section = ".requests"]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

/// Define the stand and end markers for Limine requests.
#[used]
#[link_section = ".requests_start_marker"]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[link_section = ".requests_end_marker"]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[no_mangle]
pub extern "C" fn kernel_start() -> ! {
    assert!(BASE_REVISION.is_supported());

    init();

    sprintln!("ok");

    println!("NaCl v{}", env!("CARGO_PKG_VERSION"));
    println!("Ayo");

    /*for i in 1..1000 {
        cpu().executor.borrow_mut().spawn(task::Task::new(async move {
            FBMAN.lock().await.as_mut().unwrap().write_fmt(format_args_nl!("{i}")).unwrap();
        }))
    }*/
    cpu().executor.borrow_mut().run();
}

pub struct Logger;

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        sprintln!("{} - {}", record.level(), record.args())
    }
    fn flush(&self) {
        
    }
}

fn init() {
    let physical_memory_offset = HHDM_REQUEST.get_response().unwrap().offset();
    // SAFETY: provided via boot_info, so it is correct
    unsafe {
        crate::arch::memory_init(
            VirtAddr::new(physical_memory_offset),
            MEMORY_MAP_REQUEST.get_response().unwrap().entries(),
        );
    }
    log::set_logger(&Logger).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    // log::info!("hi");

    let frame_buffer = FRAMEBUFFER_REQUEST.get_response().unwrap().framebuffers().next().unwrap();
    let mut fb = FrameBufferManager::new(&frame_buffer);
    sprintln!("{fb:?}");
    fb.putchar('F', 0, 0, 0xFFFFFF, 0);
    font::insert_fbman(fb);

    // initialize per-core memory access.
    crate::arch::init(physical_memory_offset as usize, RSDP_REQUEST.get_response().unwrap().address() as usize - physical_memory_offset as usize);
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
