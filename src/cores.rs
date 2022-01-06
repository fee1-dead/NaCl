use core::array;
use core::marker::PhantomData;
use core::num::Wrapping;

pub const MAX_NUM_CPUS: usize = 64;

#[derive(Default)]
pub struct Cpu {
    pub timer: Wrapping<usize>,
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

impl CpuKey<'static> {
    #[inline]
    pub fn scope<F, T>(callback: F) -> T
    where
        F: for<'a> FnOnce(&CpuKey<'a>) -> T,
    {
        let key = CpuKey {
            num: id(),
            _notsend: PhantomData,
            _lt_invariant: PhantomData,
        };
        callback(&key)
    }
}

static mut CPUS: Option<[Cpu; MAX_NUM_CPUS]> = None;

pub fn init() {
    unsafe {
        crate::cores::CPUS = Some(array::from_fn(|_| Cpu::default()));
    }
}

pub fn cpu_enter<F: FnOnce(&mut Cpu) -> T, T>(f: F) -> T {
    CpuKey::scope(move |k| f(get_cpu(k)))
}

pub fn get_cpu<'a>(key: &CpuKey<'a>) -> &'a mut Cpu {
    // SAFETY: the key guarantees unique access of the CPU.
    unsafe {
        &mut CPUS.as_mut().unwrap()[key.num as usize]
    }
}

