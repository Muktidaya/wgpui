use async_task::Runnable;
use std::{
    collections::VecDeque,
    marker::PhantomData,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::Duration,
};

/// Implement this for your priority enum to use with [`ThreadPool`].
pub trait Priority {
    /// Number of distinct priority levels (index 0 = highest).
    const COUNT: usize;
    /// Map this priority value to a queue index in `[0, COUNT)`.
    fn index(&self) -> usize;
}

type Task = Box<dyn FnOnce() + Send + 'static>;

struct Shared {
    /// One queue per priority level; index 0 is highest priority.
    queues: Vec<VecDeque<Task>>,
    stopped: bool,
}

struct Inner {
    shared: Mutex<Shared>,
    condvar: Condvar,
}

impl Inner {
    fn push(&self, index: usize, task: Task) {
        let mut guard = self.shared.lock().unwrap();
        guard.queues[index].push_back(task);
        drop(guard);
        self.condvar.notify_one();
    }
}

pub struct ThreadPool<P: Priority> {
    inner: Arc<Inner>,
    _threads: Vec<JoinHandle<()>>,
    _phantom: PhantomData<P>,
}

impl<P: Priority> ThreadPool<P> {
    pub fn new(num_threads: usize) -> Self {
        let inner = Arc::new(Inner {
            shared: Mutex::new(Shared {
                queues: (0..P::COUNT).map(|_| VecDeque::new()).collect(),
                stopped: false,
            }),
            condvar: Condvar::new(),
        });

        let threads = (0..num_threads)
            .map(|ix| {
                let inner = Arc::clone(&inner);
                std::thread::Builder::new()
                    .name(format!("wgpui-worker-{ix}"))
                    .spawn(move || {
                        loop {
                            let task = {
                                let mut guard = inner.shared.lock().unwrap();
                                loop {
                                    if guard.stopped {
                                        return;
                                    }
                                    if let Some(task) =
                                        guard.queues.iter_mut().find_map(|q| q.pop_front())
                                    {
                                        break task;
                                    }
                                    guard = inner.condvar.wait(guard).unwrap();
                                }
                            };
                            task();
                        }
                    })
                    .expect("failed to spawn worker thread")
            })
            .collect();

        Self {
            inner,
            _threads: threads,
            _phantom: PhantomData,
        }
    }

    pub fn queue<M: Send + Sync + 'static>(&self, priority: &P, runnable: Runnable<M>) {
        let index = priority.index();
        debug_assert!(index < P::COUNT, "priority index out of bounds");
        self.inner.push(
            index,
            Box::new(move || {
                let _ = runnable.run();
            }),
        );
    }

    pub fn queue_delayed<M: Send + Sync + 'static>(
        &self,
        priority: &P,
        duration: Duration,
        runnable: Runnable<M>,
    ) {
        let index = priority.index();
        debug_assert!(index < P::COUNT, "priority index out of bounds");
        let inner = Arc::clone(&self.inner);
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            inner.push(
                index,
                Box::new(move || {
                    let _ = runnable.run();
                }),
            );
        });
    }
}

impl<P: Priority> Drop for ThreadPool<P> {
    fn drop(&mut self) {
        let mut guard = self.inner.shared.lock().unwrap();
        guard.stopped = true;
        drop(guard);
        self.inner.condvar.notify_all();
        // Threads are joined implicitly when JoinHandle drops, but since we
        // don't call join() here the threads will finish naturally after waking.
    }
}
