//! The animal step's share of the process's compute pool: the `bevy_tasks`
//! `ComputeTaskPool` the plant step and the schedule already run on (no second pool).
//! Independent jobs run side by side when the host steps with more than one thread, and
//! their results come back **in job order** whichever finished first, so a caller that
//! folds them in that order gets what one serial pass would have built.

/// One job: borrows what it reads, returns what it built.
pub(crate) type Job<'a, T> = Box<dyn FnOnce() -> T + Send + 'a>;

/// Run `jobs`, on the compute pool when `threads > 1` and there is more than one.
pub(crate) fn run<T: Send + 'static>(threads: usize, jobs: Vec<Job<'_, T>>) -> Vec<T> {
    #[cfg(feature = "parallel")]
    if threads > 1 && jobs.len() > 1 {
        let pool = bevy_tasks::ComputeTaskPool::get_or_init(|| {
            bevy_tasks::TaskPoolBuilder::new().num_threads(threads).build()
        });
        let mut parts: Vec<(usize, T)> = pool.scope(|scope| {
            for (k, job) in jobs.into_iter().enumerate() {
                scope.spawn(async move { (k, job()) });
            }
        });
        parts.sort_by_key(|&(k, _)| k);
        return parts.into_iter().map(|(_, t)| t).collect();
    }
    let _ = threads;
    jobs.into_iter().map(|job| job()).collect()
}

/// `0..n` split into at most `threads` contiguous ranges of at least `min` items (one
/// range when the split would not pay, or without the `parallel` feature).
pub(crate) fn ranges(n: usize, threads: usize, min: usize) -> Vec<std::ops::Range<usize>> {
    let chunks = if cfg!(feature = "parallel") {
        threads.max(1).min(n / min.max(1)).max(1)
    } else {
        1
    };
    let per = n.div_ceil(chunks).max(1);
    (0..chunks)
        .map(|k| (k * per).min(n)..((k + 1) * per).min(n))
        .filter(|r| !r.is_empty() || n == 0)
        .collect()
}
