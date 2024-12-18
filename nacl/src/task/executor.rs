use alloc::sync::Arc;
use core::task::{Context, Poll, Waker};

use hashbrown::HashMap;

use super::{Task, TaskId};
use crate::cores::{cpu, stealers};

pub struct Executor {
    tasks: HashMap<TaskId, Task>,
    waker_cache: HashMap<TaskId, Waker>,
}

impl Default for Executor {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    #[inline]
    pub fn new() -> Self {
        Executor {
            tasks: HashMap::new(),
            waker_cache: HashMap::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        cpu().worker.push(task_id);
    }

    fn run_ready_tasks(&mut self) {
        let Self { tasks, waker_cache } = self;
        while let Some(task_id) = cpu().worker.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // task no longer exists
            };
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new_waker(task_id));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};

        interrupts::disable();
        if cpu().worker.is_empty() {
            if false {
                panic!();
            }
            if !stealers().any(|stealer| {
                let mut res = stealer.steal_batch(&cpu().worker);
                while res.is_retry() {
                    res = stealer.steal_batch(&cpu().worker);
                }
                res.is_success()
            }) {
                enable_and_hlt();
            }
        } else {
            interrupts::enable();
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
}

impl TaskWaker {
    fn wake_task(&self) {
        cpu().worker.push(self.task_id);
    }
}

use alloc::task::Wake;

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}

impl TaskWaker {
    fn new_waker(task_id: TaskId) -> Waker {
        Waker::from(Arc::new(TaskWaker { task_id }))
    }
}
