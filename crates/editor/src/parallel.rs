//! Work shared out among the processor's cores.

use std::{
    num::NonZero,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

/// What every worker does before its work, if the app says: on Windows, Kata has
/// them give way to its windows, which this crate knows nothing of.
static FIRST: OnceLock<fn()> = OnceLock::new();

/// Has every worker do `first` before its work, from now on. Only the first call
/// counts.
pub fn before_work(first: fn()) {
    let _ = FIRST.set(first);
}

/// How many pieces of work run at once.
pub fn width() -> usize {
    thread::available_parallelism().map_or(1, NonZero::get)
}

/// `work(0)` to `work(count - 1)`, in that order, each done on whichever core is
/// free.
pub fn each<T: Send>(count: usize, work: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = width().min(count);
    if threads <= 1 {
        return (0..count).map(work).collect();
    }
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, T)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    if let Some(first) = FIRST.get() {
                        first();
                    }
                    let mut mine = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= count {
                            break mine;
                        }
                        mine.push((index, work(index)));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a worker finishes"))
            .collect()
    });
    done.sort_unstable_by_key(|&(index, _)| index);
    done.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_keep_their_order() {
        assert_eq!(each(0, |index| index), []);
        assert_eq!(each(1, |index| index + 5), [5]);
        let squares = each(1000, |index| index * index);
        assert!(
            squares
                .iter()
                .enumerate()
                .all(|(i, &square)| square == i * i)
        );
    }
}
