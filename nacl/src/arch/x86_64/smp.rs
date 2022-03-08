//! Simultanous multi processing.

use core::arch::{asm, global_asm};
use core::ptr::addr_of;
use core::sync::atomic::{AtomicU16, AtomicUsize};

use acpi::platform::ProcessorState;
use acpi::PlatformInfo;
use x86_64::structures::gdt::{Descriptor, DescriptorFlags, GlobalDescriptorTable};
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::structures::paging::mapper::PageTableFrameMapping;
use x86_64::structures::paging::PageTable;
use x86_64::structures::DescriptorTablePointer;
use x86_64::VirtAddr;

use super::apic::lapic;
use crate::arch::id;
use crate::arch::x86_64::apic::Lapic;
use crate::{hlt_loop, println};

// variables for AP to read and use. Because AP's are not the BSP,
// therefore does not have access to memory *yet*. BSP acts as a caretaker
// and sets up the stack for each AP to use.

extern "C" {
    /// start of the stack for AP
    static mut AP_STACK_START: usize;

    /// the BSP takes a snapshot of the page tables during boot, each AP can decide
    /// to use their own page tables later.
    static mut AP_LEVEL_FOUR_PAGE_TABLE: usize;
}

global_asm!(
    "
    .comm AP_STACK_START, 8, 8
    .comm AP_LEVEL_FOUR_PAGE_TABLE, 8, 8
"
);

// Constants that we prepare for the AP.

/// can't use the x86_64 version as that requires long mode
#[repr(C, packed)]
pub struct DtPointer {
    /// Size of the DT.
    pub limit: u16,
    /// Pointer to the memory region containing the DT.
    pub base: u32,
}

#[no_mangle]
static AP_STARTED: AtomicU16 = AtomicU16::new(0);

/// Empty IDT
static IDT: DtPointer = DtPointer { limit: 0, base: 0 };

/// gdt starts with a null entry, we add kernel code segment and kernel data segments.
static GDT: [u64; 3] = [
    0,
    DescriptorFlags::KERNEL_CODE64.bits(),
    DescriptorFlags::KERNEL_DATA.bits(),
];

static mut GDTPTR: DtPointer = DtPointer {
    limit: 3,
    // bogus value. gets overwritten in assembly.
    // TODO replace this with solution from https://github.com/rust-lang/rust/issues/51910#issuecomment-1013921561.
    base: 0,
};

/*
global_asm!(include_str!("ap_init.asm"));
global_asm!(include_str!("ap_init_long.asm"));
*/

/// initialize application processor.
///
/// There is a lot to be done here: the AP boots on real mode,
/// we need to set up the stack, make and load global descriptor table,
/// setting up and enabling paging, and switch to long mode to be able
/// to call Rust code. Fortunately we do not need to set up a framebuffer
/// and all that kinds of stuff.
#[naked]
#[repr(align(0x8000))]
pub unsafe extern "C" fn ap_init_trampoline() -> ! {
    asm!("5: jmp 5b", options(noreturn));
}

#[no_mangle]
pub extern "C" fn ap_init() -> ! {
    println!("Hello from AP {}!", id());
    hlt_loop()
}

pub fn init(platform_info: &PlatformInfo) {
    let procinfo = platform_info
        .processor_info
        .as_ref()
        .expect("processor info");
    let mut lapic = lapic();
    for ap in &procinfo.application_processors {
        assert!(ap.is_ap);

        if let ProcessorState::Disabled | ProcessorState::Running = ap.state {
            continue;
        }

        // send INIT IPI

        // TODO: Support x2APIC
        //
        // TODO(COMPAT): delay between INIT and INIT Deassert?
        // https://forum.osdev.org/viewtopic.php?t=40408&p=316625
        //
        // https://web.archive.org/web/20121002210153/http://download.intel.com/design/archives/processors/pro/docs/24201606.pdf
        unsafe {
            let select_ap = |lapic: &mut Lapic| {
                lapic.update_register(0x310, |v| (v & 0x00FFFFFF) | ap.local_apic_id << 24);
            };
            // TODO: Example on OSDev wiki sets this to zero, while another article shows
            // this register as read only
            lapic.write_register(0x280, 0); // clear errors

            // Trigger INIT IPI
            select_ap(&mut lapic);
            lapic.update_register(0x300, |v| (v & 0xFFF00000) | 0x00C500);
            lapic.icr_wait_for_delivery();

            // INIT Level De-assert
            select_ap(&mut lapic);
            lapic.update_register(0x300, |v| (v & 0xFFF00000) | 0x008500);
            lapic.icr_wait_for_delivery();

            // TODO(COMPAT): send SIPI twice?
            // https://forum.osdev.org/viewtopic.php?t=11564
            lapic.write_register(0x280, 0);

            select_ap(&mut lapic);

            // "These local APICs recognize the STARTUP IPI, which is an APIC Interprocessor Interrupt
            // with trigger mode set to edge and delivery mode set to “110” (bits 8 through 10 of the ICR)."
            let mode = 0b110;

            let trampoline_addr = ap_init_trampoline as usize;
            // "The STARTUP IPI causes the target processor to start executing in Real Mode from address
            // 000VV000h, where VV is an 8-bit vector that is part of the IPI message."
            assert!(trampoline_addr & (!0xFF000) != 0); // panic if trampoline has other bits set

            // Startup IPI
            // TODO(COMPAT): OSDev wiki updates the register instead of writing
            lapic.write_register(0x300, mode << 8 | trampoline_addr as u32 >> 12);
        }
    }
}
