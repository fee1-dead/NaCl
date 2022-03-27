use alloc::boxed::Box;
use x86_64::instructions::interrupts::without_interrupts;
use core::array;
use core::cell::{Cell, RefCell};
use core::marker::PhantomData;
use core::num::Wrapping;
use core::sync::atomic::{AtomicBool, Ordering};

use crossbeam_epoch::LocalHandle;
use stivale_boot::v2::StivaleSmpInfo;

use crate::task::TaskId;
use crate::task::crossbeam::{Worker, Stealer};
use crate::task::executor::Executor;

pub const MAX_NUM_CPUS: usize = 64;

/// a large structure containing core-local state.
pub struct Cpu {
    pub timer: Cell<Wrapping<usize>>,
    pub local_handle: LocalHandle,
    pub executor: RefCell<Executor>,
    pub worker: Worker<TaskId>,
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            timer: Cell::new(Wrapping(0)),
            local_handle: crate::task::gc::default_collector().register(),
            executor: RefCell::default(),
            worker: Worker::new_fifo(),
        }
    }
}

/// A key that is only accessible from a particular CPU.
#[derive(PartialEq, Eq)]
#[repr(transparent)]
pub struct CpuKey<'a> {
    /// the assigned number of this cpu.
    num: u32,

    /// cannot send this
    _notsend: PhantomData<*const ()>,

    /// lifetime is invariant
    _lt_invariant: PhantomData<fn(&'a ()) -> &'a ()>,
}

/// Returns a unique identifying number of this processor.
pub fn id() -> u32 {
    crate::arch::apic::lapic().id()
}

#[allow(clippy::declare_interior_mutable_const)]
const CPU_SCOPE_INIT: AtomicBool = AtomicBool::new(false);
static CPU_ENTERS: [AtomicBool; MAX_NUM_CPUS] = [CPU_SCOPE_INIT; MAX_NUM_CPUS];

impl CpuKey<'static> {
    pub fn scope<F, T>(callback: F) -> T
    where
        F: for<'a> FnOnce(&CpuKey<'a>) -> T,
    {
        let num = id();
        let key = CpuKey {
            num,
            _notsend: PhantomData,
            _lt_invariant: PhantomData,
        };

        callback(&key)
    }
}

const CPU_INIT: Option<&'static Cpu> = None;
static mut CPUS: [Option<&'static Cpu>; MAX_NUM_CPUS] = [CPU_INIT; MAX_NUM_CPUS];

const STEALER_INIT: Option<Stealer<TaskId>> = None;
pub static mut STEALERS: [Option<Stealer<TaskId>>; MAX_NUM_CPUS] = [STEALER_INIT; MAX_NUM_CPUS];

pub fn stealers<'a>() -> impl Iterator<Item = &'a Stealer<TaskId>> {
    unsafe {
        STEALERS.iter().filter_map(|st| st.as_ref())
    }
}

pub fn cpu<'a>() -> &'a Cpu {
    // SAFETY: the key guarantees unique access of the CPU.
    CpuKey::scope(move |k| {
        unsafe { &mut CPUS[k.num as usize] }
            .get_or_insert_with(|| {
                let cpu = Box::leak(Box::new(Cpu::new()));
                unsafe {
                    STEALERS[k.num as usize] = Some(cpu.worker.stealer());
                }
                cpu
            })
    })
}
