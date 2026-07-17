//! Fixed-size worker pool.

use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A simple fixed-size thread pool with `Submit` / `Execute` / `Join` semantics.
pub struct ThreadPool {
    sender: Option<Sender<Job>>,
    workers: Vec<JoinHandle<()>>,
}

impl ThreadPool {
    /// Creates a pool with `num_threads` workers.
    #[must_use]
    pub fn new(num_threads: usize) -> Self {
        assert!(num_threads > 0, "thread pool size must be positive");
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(num_threads);
        for _ in 0..num_threads {
            let receiver = Arc::clone(&receiver);
            workers.push(thread::spawn(move || worker_loop(receiver)));
        }
        Self {
            sender: Some(sender),
            workers,
        }
    }

    /// Submits a job that returns a value and returns a handle to that value.
    pub fn submit<F, T>(
        &self,
        job: F,
    ) -> std::sync::mpsc::Receiver<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        self.execute(move || {
            let _ = tx.send(job());
        });
        rx
    }

    /// Submits a fire-and-forget job.
    pub fn execute<F>(
        &self,
        job: F,
    ) where
        F: FnOnce() + Send + 'static,
    {
        let sender = self.sender.as_ref().expect("thread pool has been joined");
        sender.send(Box::new(job)).expect("thread pool worker disconnected");
    }

    /// Waits for all queued work to finish and joins worker threads.
    pub fn join(&mut self) {
        self.sender.take();
        for worker in self.workers.drain(..) {
            worker.join().expect("thread pool worker panicked");
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        if self.sender.is_some() {
            self.join();
        }
    }
}

fn worker_loop(receiver: Arc<Mutex<Receiver<Job>>>) {
    loop {
        let job = {
            let guard = receiver.lock().expect("thread pool receiver mutex");
            guard.recv()
        };
        match job {
            Ok(job) => job(),
            Err(_) => break,
        }
    }
}
