//! The default garbage collector.
//!
//! For each thread, a participant is lazily initialized on its first use, when the current thread
//! is registered in the default collector.  If initialized, the thread's participant will get
//! destructed on thread exit, which in turn unregisters the thread.

use crossbeam_epoch::{Collector, Guard, LocalHandle};

use crate::cores::cpu_enter;

lazy_static::lazy_static! {
    /// The global data for the default garbage collector.
    static ref COLLECTOR: Collector = Collector::new();
}

/// Pins the current thread.
#[inline]
pub fn pin() -> Guard {
    with_handle(|handle| handle.pin())
}

/// Returns `true` if the current thread is pinned.
#[inline]
pub fn is_pinned() -> bool {
    with_handle(|handle| handle.is_pinned())
}

/// Returns the default global collector.
pub fn default_collector() -> &'static Collector {
    &COLLECTOR
}

#[inline]
fn with_handle<F, R>(mut f: F) -> R
where
    F: FnMut(&LocalHandle) -> R,
{
    cpu_enter(|cpu| f(&cpu.local_handle))
}
