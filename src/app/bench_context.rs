use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread::ThreadId,
    time::{Duration, Instant},
};

use crate::{
    AnyWindowHandle, App, AppCell, AppContext, BackgroundExecutor, Context, Empty, Entity,
    ForegroundExecutor, PlatformDispatcher, Priority, Render, RunnableVariant, Task, TestPlatform,
    ThreadTaskTimings, Window, WindowOptions, platform::priority_pool::ThreadPool,
};
use parking_lot::{Condvar, Mutex};

struct BenchmarkDispatcher {
    thread: ThreadId,
    foreground: Mutex<VecDeque<RunnableVariant>>,
    ready: Condvar,
    background: Arc<AtomicUsize>,
    workers: ThreadPool<Priority>,
}

impl BenchmarkDispatcher {
    fn new() -> Self {
        Self {
            thread: std::thread::current().id(),
            foreground: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
            background: Arc::new(AtomicUsize::new(0)),
            workers: ThreadPool::new(num_cpus::get().max(1)),
        }
    }

    fn drain(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let runnable = self.foreground.lock().pop_front();
            if let Some(runnable) = runnable {
                runnable.run();
                continue;
            }
            let mut queue = self.foreground.lock();
            if queue.is_empty() && self.background.load(Ordering::Acquire) == 0 {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "benchmark background work did not settle within 30 seconds"
            );
            self.ready.wait_for(&mut queue, Duration::from_millis(1));
        }
    }
}

impl PlatformDispatcher for BenchmarkDispatcher {
    fn get_all_timings(&self) -> Vec<ThreadTaskTimings> {
        ThreadTaskTimings::convert(&crate::GLOBAL_THREAD_TIMINGS.lock())
    }
    fn get_current_thread_timings(&self) -> Vec<crate::TaskTiming> {
        crate::THREAD_TIMINGS.with(|timings| timings.lock().timings.iter().cloned().collect())
    }
    fn is_main_thread(&self) -> bool {
        std::thread::current().id() == self.thread
    }
    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        self.background.fetch_add(1, Ordering::Release);
        let background = self.background.clone();
        self.workers.queue_job(&priority, move || {
            let completed = util::defer(move || {
                background.fetch_sub(1, Ordering::Release);
            });
            runnable.run();
            drop(completed);
        });
    }
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _priority: Priority) {
        self.foreground.lock().push_back(runnable);
        self.ready.notify_one();
    }
    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        self.workers
            .queue_delayed(&Priority::Low, duration, runnable);
    }
    fn spawn_realtime(&self, callback: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(callback);
    }
    fn wakeup(&self) {
        self.ready.notify_one();
    }
}

/// A Criterion benchmark application with production text shaping and real worker threads.
///
/// The portable window performs actual layout, painting and offscreen GPU submission.
/// Measurements include CPU scene preparation and submission; they exclude GPU completion
/// and physical display latency.
/// Timers use wall time; future timers do not prevent [`Self::run_until_idle`] from returning.
pub struct BenchAppContext<'context, 'bencher> {
    app: Rc<AppCell>,
    background: BackgroundExecutor,
    foreground: ForegroundExecutor,
    dispatcher: Arc<BenchmarkDispatcher>,
    bencher: Option<&'context mut criterion::Bencher<'bencher>>,
}

impl<'context, 'bencher> BenchAppContext<'context, 'bencher> {
    /// Creates an independent benchmark app. Font discovery and setup occur outside measurement.
    pub fn new(bencher: &'context mut criterion::Bencher<'bencher>) -> Self {
        let dispatcher = Arc::new(BenchmarkDispatcher::new());
        let background = BackgroundExecutor::new(dispatcher.clone());
        let foreground = ForegroundExecutor::new(dispatcher.clone());
        let platform = TestPlatform::with_platform(
            background.clone(),
            foreground.clone(),
            crate::platform::text_system::new(),
            Some(Box::new(crate::current_headless_renderer)),
        );
        let app = App::new_app(
            platform,
            Arc::new(()),
            http_client::FakeHttpClient::with_404_response(),
        );
        Self {
            app,
            background,
            foreground,
            dispatcher,
            bencher: Some(bencher),
        }
    }

    /// Updates application state and flushes synchronous effects.
    pub fn update<R>(&mut self, callback: impl FnOnce(&mut App) -> R) -> R {
        self.app.borrow_mut().update(callback)
    }

    /// Reads application state without dispatching pending work.
    pub fn read<R>(&self, callback: impl FnOnce(&App) -> R) -> R {
        callback(&self.app.borrow())
    }

    /// Returns the executor running production background tasks.
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background
    }

    /// Returns the benchmark's foreground executor.
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground
    }

    /// Drains ready foreground work and in-flight worker tasks, then draws pending windows once.
    pub fn run_until_idle(&self) {
        self.dispatcher.drain();
        self.draw_pending_frames();
        self.dispatcher.drain();
    }

    /// Finishes setup work before entering a measured iteration loop.
    pub fn settle(&mut self) {
        self.run_until_idle();
    }

    /// Opens a portable benchmark window containing an empty view.
    pub fn add_empty_window(&mut self) -> BenchWindowContext {
        let window = self
            .update(|cx| cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty)))
            .expect("benchmark window must open");
        BenchWindowContext {
            app: self.app.clone(),
            window: window.into(),
        }
    }

    fn draw_pending_frames(&self) {
        let handles = self.app.borrow().windows();
        for handle in handles {
            self.app
                .borrow_mut()
                .update_window(handle, |_, window, cx| window.benchmark_frame(cx))
                .expect("benchmark window must remain open while drawing");
        }
    }

    /// Measures a foreground operation and all rendering it schedules once per Criterion iteration.
    pub fn bench_iter(&mut self, mut benchmark: impl FnMut(&mut Self)) {
        self.run_until_idle();
        let bencher = self
            .bencher
            .take()
            .expect("a benchmark measurement cannot be nested");
        bencher.iter(|| {
            benchmark(self);
            self.run_until_idle();
        });
        self.bencher = Some(bencher);
    }

    /// Measures a task to completion, including its foreground and background work.
    pub fn bench_task<Output: 'static>(
        &mut self,
        mut benchmark: impl FnMut(&mut Self) -> Task<Output>,
    ) {
        self.bench_iter(|cx| {
            let output = Rc::new(RefCell::new(None));
            let task = benchmark(cx);
            cx.foreground
                .spawn({
                    let output = output.clone();
                    async move {
                        *output.borrow_mut() = Some(task.await);
                    }
                })
                .detach();
            let deadline = Instant::now() + Duration::from_secs(30);
            while output.borrow().is_none() {
                cx.run_until_idle();
                assert!(
                    Instant::now() < deadline,
                    "benchmark task did not complete within 30 seconds"
                );
                std::thread::yield_now();
            }
            std::hint::black_box(output.borrow_mut().take());
        });
    }

    /// Measures an entity update followed by actual CPU layout and painting of its window.
    pub fn bench_renderer<V: Render + 'static>(
        &mut self,
        view: Entity<V>,
        mut update: impl FnMut(&mut V, &mut Window, &mut Context<V>),
    ) {
        self.run_until_idle();
        let entity_id = view.entity_id();
        let handles = self.app.borrow().windows();
        let handle = handles
            .into_iter()
            .find(|handle| {
                self.app
                    .borrow_mut()
                    .update_window(*handle, |root, window, _| {
                        root.entity_id() == entity_id
                            || window.rendered_frame.dispatch_tree.contains_view(entity_id)
                    })
                    .expect("benchmark window must remain open")
            })
            .expect("render benchmark entity must be attached to a window");
        self.bench_iter(|cx| {
            cx.app
                .borrow_mut()
                .update_window(handle, |_, window, cx| {
                    view.update(cx, |view, cx| update(view, window, cx));
                })
                .expect("render benchmark window must remain open");
        });
    }

    /// Runs application shutdown after a benchmark has finished.
    pub fn teardown(mut self) {
        self.update(App::shutdown);
        self.dispatcher.drain();
    }
}

/// A window used during Criterion benchmark setup.
#[derive(Clone)]
pub struct BenchWindowContext {
    app: Rc<AppCell>,
    window: AnyWindowHandle,
}

impl BenchWindowContext {
    /// Returns the portable window handle.
    pub fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    /// Updates the benchmark window and application state.
    pub fn update<R>(&mut self, callback: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        self.app
            .borrow_mut()
            .update_window(self.window, |_, window, cx| callback(window, cx))
            .expect("benchmark window must remain open during setup")
    }
}
