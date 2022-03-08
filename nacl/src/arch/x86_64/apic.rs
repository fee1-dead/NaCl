use core::hint::spin_loop;
use core::ptr::NonNull;

use acpi::{InterruptModel, PlatformInfo};
use pic8259::ChainedPics;

use super::interrupts::{InterruptIndex, PIC_1_OFFSET, PIC_2_OFFSET};
use super::memory::mapper::Mapper;
use crate::sprintln;

static mut LAPIC: Option<Lapic> = None;

/// TODO avoid concurrent read/writes
pub fn lapic() -> Lapic {
    unsafe { LAPIC.unwrap_unchecked() }
}

macro_rules! common_apic_methods {
    ($offset:ident) => {
        #[inline]
        pub unsafe fn read_register(&mut self, offset: $offset) -> u32 {
            self.register_at(offset).read_volatile()
        }

        #[inline]
        pub unsafe fn write_register(&mut self, offset: $offset, value: u32) {
            self.register_at(offset).write_volatile(value);
        }

        #[inline]
        pub unsafe fn update_register<F>(&mut self, offset: $offset, f: F)
        where
            F: FnOnce(u32) -> u32,
        {
            let reg = self.read_register(offset);
            self.write_register(offset, f(reg));
        }
    };
}
pub const APIC_TIMER_PERIODIC: u32 = 0x20000;
pub const APIC_MASKED: u32 = 0x10000;

////////////////////////////////////
// REGISTERS

/// The local vector table for LAPIC timer.
///
/// See LVT format at https://wiki.osdev.org/APIC#Local_Vector_Table_Registers
pub const LAPIC_LVT_TIMER_REG: usize = 0x320;

pub const LAPIC_LVT_LINT0_REG: usize = 0x350;

pub const LAPIC_LVT_LINT1_REG: usize = 0x360;

/// The initial count of the timer.
pub const LAPIC_TIMER_INITCNT_REG: usize = 0x380;

/// The current count of the timer.
pub const LAPIC_TIMER_CURRCNT_REG: usize = 0x390;

/// The divider of the timer.
pub const LAPIC_TIMER_DIV_REG: usize = 0x3E0;

/// Local APIC.
#[derive(Clone, Copy)]
pub struct Lapic {
    /// pointer (virtual memory) to the start address of this APIC.
    pub start_ptr: NonNull<u8>,
}

impl Lapic {
    #[inline]
    pub unsafe fn end_of_interrupt(&mut self) {
        self.write_register(0xB0, 0);
    }

    #[inline]
    pub unsafe fn register_at(&mut self, offset: usize) -> *mut u32 {
        self.start_ptr.as_ptr().add(offset).cast()
    }

    pub unsafe fn icr_wait_for_delivery(&mut self) {
        while self.read_register(0x300) & (1 << 12) != 0 {
            spin_loop()
        }
    }

    common_apic_methods!(usize);
}

/// I/O APIC.
#[derive(Clone, Copy)]
pub struct IoApic {
    /// virtual memory pointer to IOREGSEL.
    pub start_ptr: NonNull<u8>,
}

impl IoApic {
    pub unsafe fn register_at(&mut self, offset: u8) -> *mut u32 {
        // tell IOREGSEL where we want to write to
        self.start_ptr
            .as_ptr()
            .cast::<u32>()
            .write_volatile(offset as _);

        self.start_ptr.as_ptr().add(0x10).cast()
    }

    common_apic_methods!(u8);
}

pub const IOAPICVER: u8 = 1;

/// Initialize and disable the old PIC.
pub fn init_and_disable_old_pic() {
    let mut chained_pics = unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) };

    unsafe {
        chained_pics.initialize();
        chained_pics.disable();
    }
}

pub fn init_lapic(platform_info: &PlatformInfo, mapper: &Mapper) {
    let apic = match &platform_info.interrupt_model {
        InterruptModel::Apic(apic) => apic,
        _ => panic!("unknown interrupt model"),
    };

    sprintln!("{apic:#?}");

    let lapic_addr = apic.local_apic_address;

    let start_ptr = mapper.phys_to_virt_ptr(lapic_addr as usize);
    let mut lapic = Lapic { start_ptr };

    // Set the Spurious Interrupt Vector Register bit 8 to start receiving interrupts.
    unsafe {
        lapic.update_register(0xF0, |reg| reg | 0x100);
    }

    unsafe {
        for reg in [
            LAPIC_LVT_TIMER_REG,
            LAPIC_LVT_LINT0_REG,
            LAPIC_LVT_LINT1_REG,
        ] {
            lapic.write_register(reg, APIC_MASKED);
        }
    }

    unsafe {
        LAPIC = Some(lapic);
    }
}

/// Initialize the I/O APIC to enable PIT interrupts.
///
/// Returns the I/O APIC wit the PIT register.
pub fn init_ioapic(platform_info: &PlatformInfo, mapper: &Mapper) -> (IoApic, u8) {
    let apic = match &platform_info.interrupt_model {
        InterruptModel::Apic(apic) => apic,
        _ => panic!("unknown interrupt model"),
    };

    // find IRQ0 which is emitted by the PIT.
    let pit = apic
        .interrupt_source_overrides
        .iter()
        .find(|ov| ov.isa_source == 0x0)
        .expect("no PIT");
    let overriden_index = pit.global_system_interrupt;

    // find the I/O APIC that handles IRQ0.
    let mut found_reg = None;
    for io_apic in &apic.io_apics {
        let address = io_apic.address;
        let base = io_apic.global_system_interrupt_base;

        let mut ioapic = IoApic {
            start_ptr: mapper.phys_to_virt_ptr(address as usize),
        };

        let ioapicver = unsafe { ioapic.read_register(IOAPICVER) };

        // https://wiki.osdev.org/IOAPIC#IOAPICVER
        let max_redir_count = (ioapicver >> 16) as u8 + 1;
        sprintln!("redirs: {max_redir_count}");

        let mut bsp_acpi_id = platform_info
            .processor_info
            .as_ref()
            .expect("apic proc info")
            .boot_processor
            .local_apic_id;
        // the correct place for destination.
        bsp_acpi_id <<= 56 - 32;

        for idx in 0..max_redir_count {
            // https://wiki.osdev.org/IOAPIC#IOREDTBL
            let reg = 0x10 + idx * 2;
            if idx as u32 + base == overriden_index {
                let vector = InterruptIndex::Timer.as_u8() as u32;

                // The default flags for the register are all zeros. The only interesting bits are the vector bits.
                unsafe {
                    ioapic.write_register(reg, vector);
                }

                // set the processor to send interrupts to. In this case it is the bootstrap processor.
                unsafe {
                    ioapic.write_register(reg + 1, bsp_acpi_id);
                }

                found_reg = Some((ioapic, reg));
            } else {
                // Per https://wiki.osdev.org/APIC#IO_APIC_Registers, set the "masked" flag
                // of other redir entries
                unsafe { ioapic.update_register(reg, |v| v | 1 << 16) }
            }
        }
    }

    found_reg.expect("could not find redirected PIT IRQ")
}
