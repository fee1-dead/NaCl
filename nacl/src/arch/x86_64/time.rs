use core::num::Wrapping;
use core::sync::atomic::AtomicU32;
use core::sync::atomic::Ordering::Relaxed;
use core::time::Duration;

use x86_64::instructions::port::Port;
use x86_64::instructions::{hlt, interrupts};

use super::apic::{
    lapic, IoApic, APIC_MASKED, LAPIC_LVT_TIMER_REG, LAPIC_TIMER_CURRCNT_REG,
    LAPIC_TIMER_INITCNT_REG,
};
use super::interrupts::InterruptIndex;
use crate::arch::x86_64::apic::{APIC_TIMER_PERIODIC, LAPIC_TIMER_DIV_REG};
use crate::cores::cpu;
use crate::sprintln;

/// number of APIC ticks in 10ms, used by AP init sequence.
///
/// Note that this is NOT the number of IRQs per 10ms.
static APIC_TICKS_IN_10MS: AtomicU32 = AtomicU32::new(0);

fn get_irq_cnt() -> Wrapping<usize> {
    cpu().timer.get()
}

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
        lapic.write_register(LAPIC_LVT_TIMER_REG, InterruptIndex::ScratchTimer as u32);
        // set the divide value to 16.
        lapic.write_register(LAPIC_TIMER_DIV_REG, 3);
    }

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
    unsafe {
        lapic.write_register(LAPIC_TIMER_INITCNT_REG, u32::MAX);
    }

    // wait for another IRQ from the PIT.
    while get_irq_cnt() - curr_pit_cnt < Wrapping(1) {}

    // Stop the APIC timer
    unsafe {
        lapic.write_register(LAPIC_LVT_TIMER_REG, APIC_MASKED);
    }

    // we've now measured the number of LAPIC ticks in 10ms.
    let apic_ticks_in_10ms = u32::MAX - unsafe { lapic.read_register(LAPIC_TIMER_CURRCNT_REG) };

    interrupts::disable();

    sprintln!("apic ticks in 10ms = {apic_ticks_in_10ms}");

    APIC_TICKS_IN_10MS.store(apic_ticks_in_10ms, Relaxed);

    // mask the PIT I/O APIC entry.
    unsafe { ioapic.write_register(pitreg, APIC_MASKED) }

    // configure the lapic timer to send an IRQ per 10ms periodically.
    unsafe {
        // use the `Timer` IRQ instead of `ScratchTimer`. Enable periodic mode.
        lapic.write_register(
            LAPIC_LVT_TIMER_REG,
            InterruptIndex::Timer as u32 | APIC_TIMER_PERIODIC,
        );
        lapic.write_register(LAPIC_TIMER_INITCNT_REG, apic_ticks_in_10ms);
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

/// precision microsecond delay, `micros` should not be larger than 1000.
pub fn udelay(micros: usize) {
    // instead of using the IRQ counter, we need to read the current count
    // register directly.
    let ticks_per_10ms = APIC_TICKS_IN_10MS.load(Relaxed) as usize;
    let delay_ticks = ticks_per_10ms * micros / 10000;

    let get_tick = || unsafe { lapic().read_register(LAPIC_TIMER_CURRCNT_REG) } as usize;

    let current_tick = get_tick();

    // not hlt-ing here, as the duration is smaller than 1ms
    //
    // N.B. the orders are swapped here because the LAPIC timer decrements
    // the tick counter while our kernel IRQ handler increments the counter.
    while current_tick - get_tick() < delay_ticks {}
}

/// precision millis delay
fn mdelay(mut millis: usize) {
    while millis >= 10 {
        let curr_irq_cnt = get_irq_cnt();

        while get_irq_cnt() - curr_irq_cnt < Wrapping(1) {
            hlt();
        }

        millis -= 10;
    }
}

/// Simple spin delay
pub fn delay(dur: Duration) {
    if dur.as_micros() < 1000 {
        udelay(dur.as_micros() as usize);
    } else {
        mdelay(
            dur.as_millis()
                .try_into()
                .expect("delay duration is too long"),
        );
    }
}
