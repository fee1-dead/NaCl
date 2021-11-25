use core::cell::UnsafeCell;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Poll, Waker};

pub struct Mutex<T> {
    locked: AtomicBool,
    inner: UnsafeCell<T>,
}

unsafe impl<T: Sync> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}

pub struct MutexGuard<'a, T> {
    waker: Option<Waker>,
    locked: &'a AtomicBool,
    inner: &'a mut T,
}

pub struct MutexLockFuture<'a, T> {
    locked: &'a AtomicBool,
    inner: &'a UnsafeCell<T>,
}

impl<'a, T> Future for MutexLockFuture<'a, T> {
    type Output = MutexGuard<'a, T>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let Self { locked, inner } = self.get_mut();
        if let Ok(_) = locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) {
            Poll::Ready(MutexGuard {
                waker: Some(cx.waker().clone()),
                locked: *locked,
                inner: unsafe { &mut *inner.get() },
            })
        } else {
            Poll::Pending
        }
    }
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MutexLockFuture<'_, T> {
        MutexLockFuture {
            locked: &self.locked,
            inner: &self.inner,
        }
    }

    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if let Ok(_) =
            self.locked
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        {
            Some(MutexGuard {
                waker: None,
                locked: &self.locked,
                inner: unsafe { &mut *self.inner.get() },
            })
        } else {
            None
        }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.locked.store(false, Ordering::Release);
        if let Some(w) = self.waker.take() {
            w.wake()
        }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}
