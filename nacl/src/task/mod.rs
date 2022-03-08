pub mod executor;
pub mod lock;

use alloc::boxed::Box;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicU64, Ordering};
use core::task::{Context, Poll};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub struct Task {
    id: TaskId,
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            id: TaskId::new(),
            future: Box::pin(future),
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}

#[derive(Default)]
pub struct Yield(bool);

impl Future for Yield {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let b = &mut self.get_mut().0;
        if *b {
            Poll::Ready(())
        } else {
            *b = true;
            Poll::Pending
        }
    }
}

/// Call in an async function to yield execution back to the executor.
pub macro ayield() {{
    <$crate::task::Yield as ::core::default::Default>::default().await;
}}

/*
pub fn block_on<F: Future>(mut fut: F) -> F::Output {
    let pin = unsafe { Pin::new_unchecked(&mut fut) };
    loop {
        pin.poll(cx)
    }
}*/
