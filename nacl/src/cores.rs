use core::array;
use core::marker::PhantomData;
use core::num::Wrapping;
use core::sync::atomic::{AtomicBool, Ordering};

pub const MAX_NUM_CPUS: usize = 64;

pub struct Cpu {
    pub timer: Wrapping<usize>,
}

impl Cpu {
    pub const fn new() -> Cpu {
        Cpu { timer: Wrapping(0) }
    }
}

/// A key that is only accessible from a particular CPU.
#[derive(PartialEq, Eq)]
#[repr(transparent)]
pub struct CpuKey<'a> {
    /// the assigned number of this cpu.
    num: u8,

    /// cannot send this
    _notsend: PhantomData<*const ()>,

    /// lifetime is invariant
    _lt_invariant: PhantomData<fn(&'a ()) -> &'a ()>,
}

/// Returns a unique identifying number of this processor.
pub fn id() -> u8 {
    crate::arch::id()
}

#[allow(clippy::declare_interior_mutable_const)]
const CPU_SCOPE_INIT: AtomicBool = AtomicBool::new(false);
static CPU_ENTERS: [AtomicBool; MAX_NUM_CPUS] = [CPU_SCOPE_INIT; MAX_NUM_CPUS];

impl CpuKey<'static> {
    #[inline]
    pub fn scope<F, T>(callback: F) -> Option<T>
    where
        F: for<'a> FnOnce(&CpuKey<'a>) -> T,
    {
        let num = id();
        if CPU_ENTERS[num as usize]
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let key = CpuKey {
                num,
                _notsend: PhantomData,
                _lt_invariant: PhantomData,
            };
            let val = callback(&key);

            let _ = CPU_ENTERS[num as usize].compare_exchange(
                true,
                false,
                Ordering::Release,
                Ordering::Relaxed,
            );

            Some(val)
        } else {
            None
        }
    }
}

const CPU_INIT: Cpu = Cpu::new();
static mut CPUS: [Cpu; MAX_NUM_CPUS] = [CPU_INIT; MAX_NUM_CPUS];

pub fn cpu_enter<F: FnOnce(&mut Cpu) -> T, T>(f: F) -> Option<T> {
    // SAFETY: the key guarantees unique access of the CPU.
    CpuKey::scope(move |k| f(unsafe { &mut CPUS[k.num as usize] }))
}
