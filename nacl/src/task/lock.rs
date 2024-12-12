use core::cell::UnsafeCell;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;

use futures_util::task::AtomicWaker;

pub struct Mutex<T> {
    inner: UnsafeCell<T>,
    locked: AtomicBool,
    waker: AtomicWaker,
}

unsafe impl<T: Sync> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}

pub struct MutexGuard<'a, T> {
    inner: &'a mut T,
    locked: &'a AtomicBool,
    waker: &'a AtomicWaker,
}

pub struct MutexLockFuture<'a, T> {
    inner: &'a UnsafeCell<T>,
    locked: &'a AtomicBool,
    waker: &'a AtomicWaker,
}

/// Check whether the lock is currently locked. Returns `Ok` on success and the lock is locked
fn check(locked: &AtomicBool) -> Result<bool, bool> {
    locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
}

impl<'a, T> Future for MutexLockFuture<'a, T> {
    type Output = MutexGuard<'a, T>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let Self {
            locked,
            inner,
            waker,
        } = self.get_mut();

        // Fast path. Avoid registering this task's waker.
        if check(locked).is_ok() {
            return Poll::Ready(MutexGuard {
                waker,
                locked,
                inner: unsafe { &mut *inner.get() },
            });
        }

        waker.register(cx.waker());
        if check(locked).is_ok() {
            Poll::Ready(MutexGuard {
                waker,
                locked,
                inner: unsafe { &mut *inner.get() },
            })
        } else {
            Poll::Pending
        }
    }
}

static NEW_WAKER: AtomicWaker = AtomicWaker::new();

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(value),
            waker: AtomicWaker::new(),
        }
    }

    pub fn lock(&self) -> MutexLockFuture<'_, T> {
        MutexLockFuture {
            locked: &self.locked,
            inner: &self.inner,
            waker: &self.waker,
        }
    }

    pub fn lock_or_spin(&self) -> MutexGuard<'_, T> {
        while check(&self.locked).is_err() {}

        MutexGuard {
            waker: &NEW_WAKER,
            locked: &self.locked,
            inner: unsafe { &mut *self.inner.get() },
        }
    }

    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if check(&self.locked).is_ok() {
            Some(MutexGuard {
                waker: &NEW_WAKER,
                locked: &self.locked,
                inner: unsafe { &mut *self.inner.get() },
            })
        } else {
            None
        }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.locked.store(false, Ordering::Release);
        self.waker.wake();
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}
