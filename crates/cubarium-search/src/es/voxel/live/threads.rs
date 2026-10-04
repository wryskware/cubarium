//! **Search runs fill idle cores at the tail** (`design/handoffs/tail-threads-2026-10-03.md`).
//!
//! A runner that pushes worlds through [`parallel_shared`] keeps one simulation thread per
//! world while jobs are queued. Once the queue is empty, the worlds still running share the
//! machine: each re-reads its [`ThreadBudget`] every simulated minute
//! ([`super::search::EvalRun::share_threads`]) and steps on about `cores / running`
//! workers, so as runs finish the survivors spread onto the freed cores.
//!
//! Thread count is execution only: it changes no rule, number or result's meaning.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A world's thread count: one while any job is queued (every worker already holds a
/// world), else the `cores` shared evenly among the `running` worlds, never below one.
pub fn budget(cores: usize, running: usize, queued: usize) -> usize {
    if queued > 0 || running == 0 {
        return 1;
    }
    (cores / running).max(1)
}

/// The machine's share-out: its cores, the worlds running and the jobs still queued.
#[derive(Debug)]
pub struct ThreadBudget {
    cores: usize,
    running: AtomicUsize,
    queued: AtomicUsize,
}

impl ThreadBudget {
    /// `cores` to share (`1` is today's pinned run: every world one thread throughout).
    pub fn new(cores: usize) -> Arc<ThreadBudget> {
        Arc::new(ThreadBudget {
            cores: cores.max(1),
            running: AtomicUsize::new(0),
            queued: AtomicUsize::new(0),
        })
    }

    pub fn cores(&self) -> usize {
        self.cores
    }

    /// [`budget`] as things stand.
    pub fn current(&self) -> usize {
        budget(
            self.cores,
            self.running.load(Ordering::Relaxed),
            self.queued.load(Ordering::Relaxed),
        )
    }

    /// Before the first world steps: the process's compute pool at full size, since it is
    /// sized once by its first user (`cubarium_voxel_sim::prepare_compute_pool`).
    pub fn prepare(&self) {
        if self.cores > 1 {
            cubarium_voxel_sim::prepare_compute_pool(self.cores);
        }
    }
}

/// [`super::trainer::parallel`] that keeps `budget`'s running and queued counts: `f` gets
/// the budget to hand its world ([`super::search::EvalRun::share_threads`]). Results in
/// item order.
pub(crate) fn parallel_shared<T: Send, R: Send>(
    items: Vec<T>,
    threads: usize,
    budget: &Arc<ThreadBudget>,
    f: impl Fn(T, &Arc<ThreadBudget>) -> R + Sync,
) -> Vec<R> {
    let n = items.len();
    budget.prepare();
    let queue: Mutex<Vec<(usize, T)>> = Mutex::new(items.into_iter().enumerate().rev().collect());
    budget.queued.store(n, Ordering::Relaxed);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1).min(n.max(1)) {
            scope.spawn(|| {
                loop {
                    let next = {
                        let mut q = queue.lock().expect("queue");
                        let next = q.pop();
                        if next.is_some() {
                            budget.running.fetch_add(1, Ordering::Relaxed);
                        }
                        budget.queued.store(q.len(), Ordering::Relaxed);
                        next
                    };
                    let Some((i, item)) = next else {
                        return;
                    };
                    let r = f(item, budget);
                    budget.running.fetch_sub(1, Ordering::Relaxed);
                    out.lock().expect("out")[i] = Some(r);
                }
            });
        }
    });
    out.into_inner()
        .expect("out")
        .into_iter()
        .map(|r| r.expect("every item ran"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_thread_while_anything_is_queued() {
        assert_eq!(budget(64, 64, 1), 1);
        assert_eq!(budget(64, 3, 10), 1);
        assert_eq!(budget(16, 1, 1), 1);
    }

    #[test]
    fn the_tail_shares_the_cores() {
        assert_eq!(budget(64, 64, 0), 1);
        assert_eq!(budget(64, 32, 0), 2);
        assert_eq!(budget(64, 3, 0), 21);
        assert_eq!(budget(16, 1, 0), 16);
        assert_eq!(budget(16, 5, 0), 3);
    }

    #[test]
    fn never_below_one() {
        assert_eq!(budget(4, 8, 0), 1);
        assert_eq!(budget(0, 1, 0), 1);
        assert_eq!(budget(16, 0, 0), 1);
    }

    #[test]
    fn one_core_is_the_pinned_run() {
        for running in 0..10 {
            for queued in 0..3 {
                assert_eq!(budget(1, running, queued), 1);
            }
        }
    }

    #[test]
    fn the_shared_runner_counts_the_tail() {
        // Three jobs on three workers: none queued once all have started, so each sees
        // the cores split three ways, then wider as the others finish.
        let b = ThreadBudget::new(12);
        let seen = parallel_shared(vec![40u64, 100, 200], 3, &b, |ms, b| {
            std::thread::sleep(std::time::Duration::from_millis(20));
            let first = b.current();
            std::thread::sleep(std::time::Duration::from_millis(ms));
            (first, b.current())
        });
        assert_eq!(seen.iter().map(|s| s.0).collect::<Vec<_>>(), vec![4, 4, 4]);
        assert_eq!(seen[2].1, 12, "the last one running gets every core");
        assert_eq!(b.current(), 1, "nothing running");
    }

    #[test]
    fn queued_jobs_keep_one_thread_each() {
        let b = ThreadBudget::new(16);
        let seen = parallel_shared((0..6).collect::<Vec<u32>>(), 2, &b, |_, b| {
            let t = b.current();
            std::thread::sleep(std::time::Duration::from_millis(10));
            t
        });
        // The first four start with jobs still queued.
        assert!(seen[..4].iter().all(|&t| t == 1), "{seen:?}");
    }
}
