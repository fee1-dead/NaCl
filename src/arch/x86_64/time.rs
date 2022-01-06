
use core::num::Wrapping;
use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

use x86_64::instructions::interrupts;
use x86_64::instructions::port::Port;

use crate::arch::x86_64::apic::{APIC_TIMER_PERIODIC, LAPIC_TIMER_DIV_REG};
use crate::cores::cpu_enter;
use crate::sprintln;

use super::apic::{lapic, LAPIC_TIMER_LVT_REG, LAPIC_TIMER_INITCNT_REG, APIC_MASKED, LAPIC_TIMER_CURRCNT_REG, IoApic};
use super::interrupts::InterruptIndex;

static APIC_TICKS_IN_10MS: AtomicU32 = AtomicU32::new(0); 

/// Configure the programmable interval timer for transition to
/// the Local APIC timer. Interrupts must not be enabled.
fn calibrate_apic_timer(mut ioapic: IoApic, pitreg: u8) {
    // set a divider for 100Hz which is 10ms per IRQ from the PIT.
    let divider = 11932u16;

    // configure the PIT to send an IRQ every 10ms.
    let mut channel0 = Port::new(0x40);
    unsafe {
        // select channel 0, access mode lobyte/hibyte, mode 2 rate generator
        Port::new(0x43).write(0b00110100u8);

        // send the lo/hi bytes to set the reload value.
        channel0.write(divider as u8);
        channel0.write((divider >> 8) as u8);
    }

    let mut lapic = lapic();

    // prepare LAPIC timer
    unsafe {
        // there are other flags the lvt register allows configuring.
        // for now set the interrrupt vector, all other flags are irrelevant.
        //
        // the timer will be in one-shot mode, meaning it will start decrementing
        // the count value when we set an init count.
        lapic.write_register(LAPIC_TIMER_LVT_REG, InterruptIndex::ScratchTimer as u32);
        // set the divide value to 16.
        lapic.write_register(LAPIC_TIMER_DIV_REG, 3);
    }

    let get_irq_cnt = || cpu_enter(|c| c.timer);


    sprintln!("right before trigger");
    // enable interrupts
    interrupts::enable();

    // we need to wait until PIT interrupts so the delay is as accurate as possible
    let saved_pit_cnt = get_irq_cnt();
    let mut curr_pit_cnt;

    loop {
        curr_pit_cnt = get_irq_cnt();

        if curr_pit_cnt - saved_pit_cnt >= Wrapping(1) {
            break;
        }
    }

    // PIT just emitted IRQ, start LAPIC timer.
    unsafe { lapic.write_register(LAPIC_TIMER_INITCNT_REG, u32::MAX); }

    // wait for another IRQ from the PIT.
    while get_irq_cnt() - curr_pit_cnt < Wrapping(1) {}

    // Stop the APIC timer
    unsafe { lapic.write_register(LAPIC_TIMER_LVT_REG, APIC_MASKED); }

    // we've now measured the number of LAPIC ticks in 10ms.
    let apic_ticks_in_10ms = u32::MAX - unsafe { lapic.read_register(LAPIC_TIMER_CURRCNT_REG) };

    interrupts::disable();

    sprintln!("apic ticks in 10ms = {apic_ticks_in_10ms}");

    APIC_TICKS_IN_10MS.store(apic_ticks_in_10ms, Relaxed);

    // mask the PIT I/O APIC entry.
    unsafe { ioapic.write_register(pitreg, APIC_MASKED) }

    // configure the lapic timer to send an IRQ per 10ms periodically.
    unsafe {
        lapic.write_register(LAPIC_TIMER_INITCNT_REG, apic_ticks_in_10ms);
        // use the `Timer` IRQ instead of `ScratchTimer`. Enable periodic mode.
        lapic.write_register(LAPIC_TIMER_LVT_REG, InterruptIndex::Timer as u32 | APIC_TIMER_PERIODIC);
        // set the divide value to 16 again. This is not required by the manuals.
        // but according to OSDev wiki there are buggy hardware out there that needs this.
        lapic.write_register(LAPIC_TIMER_DIV_REG, 3);
    }

}

/// Configure to have 100 Timer IRQs per second, i.e. 1 IRQ per 10ms.
///
/// Interrupts should not be enabled but should be properly configured
/// before calling this. Interrupts will not be enabled when this function
/// returns.
/// 
/// The programmable interval timer (PIT) should be configured to IRQ at
/// `InterruptIndex::Timer`. We currently use it to calibrate the APIC timer.
pub fn init(ioapic: IoApic, pitreg: u8) {
    calibrate_apic_timer(ioapic, pitreg);
}