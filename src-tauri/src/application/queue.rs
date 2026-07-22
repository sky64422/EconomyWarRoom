use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex, Notify};

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

struct Job {
    key: String,
    priority: u8, // lower = higher priority
    work: BoxFuture,
    done: Option<oneshot::Sender<()>>,
}

/// Minimal rate-limited job queue: max concurrent workers, coalesce by key.
///
/// **Coalesce policy:** if `key` is already in the pending queue (not yet running),
/// a second `enqueue` with the same key is a no-op and returns immediately without
/// waiting for the original job. Keys are cleared from the coalesce set when a job
/// starts running, so a new enqueue while work is in-flight will schedule again.
#[derive(Clone)]
pub struct RateLimitedQueue {
    inner: Arc<Mutex<Inner>>,
    notify: Arc<Notify>,
}

struct Inner {
    max_concurrent: usize,
    running: usize,
    pending: VecDeque<Job>,
    pending_keys: HashMap<String, ()>,
    closed: bool,
}

impl RateLimitedQueue {
    pub fn new(max_concurrent: usize) -> Self {
        assert!(max_concurrent >= 1);
        let q = Self {
            inner: Arc::new(Mutex::new(Inner {
                max_concurrent,
                running: 0,
                pending: VecDeque::new(),
                pending_keys: HashMap::new(),
                closed: false,
            })),
            notify: Arc::new(Notify::new()),
        };
        for _ in 0..max_concurrent {
            q.spawn_worker();
        }
        q
    }

    fn spawn_worker(&self) {
        let inner = self.inner.clone();
        let notify = self.notify.clone();
        tokio::spawn(async move {
            loop {
                let job = {
                    let mut g = inner.lock().await;
                    if g.closed && g.pending.is_empty() {
                        break;
                    }
                    if g.running >= g.max_concurrent {
                        None
                    } else {
                        // pick highest priority (lowest number), FIFO within same priority
                        let pos = g
                            .pending
                            .iter()
                            .enumerate()
                            .min_by_key(|(i, j)| (j.priority, *i))
                            .map(|(i, _)| i);
                        if let Some(i) = pos {
                            let job = g.pending.remove(i).unwrap();
                            g.pending_keys.remove(&job.key);
                            g.running += 1;
                            Some(job)
                        } else {
                            None
                        }
                    }
                };
                if let Some(job) = job {
                    (job.work).await;
                    if let Some(tx) = job.done {
                        let _ = tx.send(());
                    }
                    let mut g = inner.lock().await;
                    g.running -= 1;
                    notify.notify_waiters();
                } else {
                    notify.notified().await;
                }
            }
        });
    }

    /// Enqueue work and wait until it completes.
    ///
    /// If `key` is already pending (queued, not running), this is a no-op that
    /// returns immediately (coalesce). Lower `priority` numbers run first.
    pub async fn enqueue<F, Fut>(&self, key: impl Into<String>, priority: u8, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let key = key.into();
        let (tx, rx) = oneshot::channel();
        {
            let mut g = self.inner.lock().await;
            if g.closed {
                return;
            }
            if g.pending_keys.contains_key(&key) {
                // Coalesced: original pending job will run; return without re-queueing.
                return;
            }
            g.pending_keys.insert(key.clone(), ());
            g.pending.push_back(Job {
                key,
                priority,
                work: Box::pin(f()),
                done: Some(tx),
            });
        }
        self.notify.notify_waiters();
        let _ = rx.await;
    }

    pub async fn pending_len(&self) -> usize {
        self.inner.lock().await.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn respects_max_concurrent() {
        let q = RateLimitedQueue::new(2);
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];
        for i in 0..6 {
            let current = current.clone();
            let peak = peak.clone();
            let q = q.clone();
            handles.push(tokio::spawn(async move {
                q.enqueue(format!("k{i}"), 1, move || {
                    let current = current.clone();
                    let peak = peak.clone();
                    async move {
                        let c = current.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(c, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        current.fetch_sub(1, Ordering::SeqCst);
                    }
                })
                .await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= 2);
        assert!(peak.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn coalesces_while_pending() {
        let q = RateLimitedQueue::new(1);
        // Block the single worker so subsequent jobs stay pending.
        let q_block = q.clone();
        tokio::spawn(async move {
            q_block
                .enqueue("block", 0, || async {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                })
                .await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;

        let runs = Arc::new(AtomicUsize::new(0));
        let r = runs.clone();
        let q1 = q.clone();
        let h1 = tokio::spawn(async move {
            q1.enqueue("sym", 1, move || {
                let runs = r;
                async move {
                    runs.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await;
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(q.pending_len().await, 1);

        let r2 = runs.clone();
        let q2 = q.clone();
        let h2 = tokio::spawn(async move {
            q2.enqueue("sym", 1, move || {
                let runs = r2;
                async move {
                    runs.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await;
        });
        h1.await.unwrap();
        h2.await.unwrap();
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert_eq!(q.pending_len().await, 0);
    }
}
