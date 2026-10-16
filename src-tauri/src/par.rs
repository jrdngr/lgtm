//! Running work over many items on every core.

use std::sync::atomic::{AtomicUsize, Ordering};

/// `f` of each item, in order, spread over the machine's threads.
pub fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    if threads < 2 || items.len() < 2 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut out: Vec<(usize, R)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads.min(items.len()))
            .map(|_| {
                s.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        done.push((i, f(item)));
                    }
                    done
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    out.sort_unstable_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_the_items_order() {
        let items: Vec<u32> = (0..1000).collect();
        assert_eq!(
            super::map(&items, |n| n * 2),
            items.iter().map(|n| n * 2).collect::<Vec<_>>()
        );
    }
}
