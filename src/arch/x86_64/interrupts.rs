use core::num::Wrapping;
use core::ops::{Index, IndexMut};

use lazy_static::lazy_static;
use x86_64::structures::idt::{
    Entry, HandlerFunc, InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode,
};

use super::apic::lapic;
use crate::cores::cpu_enter;
use crate::{hlt_loop, sprintln};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        let options = idt.double_fault.set_handler_fn(double_fault_handler);
        unsafe {
            options.set_stack_index(super::gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt[InterruptIndex::Timer].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::ScratchTimer].set_handler_fn(scratch_timer_interrupt_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::sprintln!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    sprintln!("EXCEPTION: PAGE FAULT");
    sprintln!("Accessed Address: {:?}", Cr2::read());
    sprintln!("Error Code: {:?}", error_code);
    sprintln!("{:#?}", stack_frame);
    hlt_loop();
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    cpu_enter(|cpu| {
        cpu.timer += Wrapping(1);
    });

    unsafe {
        lapic().end_of_interrupt();
    }
}

/// The scratch timer is used when calibrating two clocks that both use IRQs
///
/// It is only available to the bootstrap processor.
pub(super) static mut SCRATCH_TIMER: usize = 0;

extern "x86-interrupt" fn scratch_timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        SCRATCH_TIMER = SCRATCH_TIMER.wrapping_add(1);
        lapic().end_of_interrupt();
    }
}

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum InterruptIndex {
    Timer = 32,
    ScratchTimer = 33,
}

impl InterruptIndex {
    #[inline]
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    #[inline]
    pub fn as_usize(&self) -> usize {
        usize::from(self.as_u8())
    }
}

impl Index<InterruptIndex> for InterruptDescriptorTable {
    type Output = Entry<HandlerFunc>;
    #[inline]
    fn index(&self, index: InterruptIndex) -> &Self::Output {
        &self[index.as_usize()]
    }
}

impl IndexMut<InterruptIndex> for InterruptDescriptorTable {
    #[inline]
    fn index_mut(&mut self, index: InterruptIndex) -> &mut Self::Output {
        &mut self[index.as_usize()]
    }
}
