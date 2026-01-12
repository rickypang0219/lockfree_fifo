use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

/// Wrapper to force alignment to 128 bytes (common cache line size is 64, but 128 is safer).
/// This ensures that the wrapped value sits on its own cache line,
/// preventing "False Sharing" where writes to nearby memory invalidates this cache line.
#[repr(align(128))]
struct CachePadded<T>(T);

pub struct Fifo3<T> {
    capacity: usize,
    ring: Vec<UnsafeCell<Option<T>>>,
    // PADDING HERE:
    // push_cursor and pop_cursor are now wrapped in CachePadded.
    // They will be at least 128 bytes apart.
    push_cursor: CachePadded<AtomicUsize>,
    pop_cursor: CachePadded<AtomicUsize>,
}

unsafe impl<T: Send> Sync for Fifo3<T> {}
unsafe impl<T: Send> Send for Fifo3<T> {}

impl<T> Fifo3<T> {
    pub fn new(capacity: usize) -> Fifo3<T> {
        let mut ring = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            ring.push(UnsafeCell::new(None));
        }
        Fifo3 {
            capacity,
            ring,
            push_cursor: CachePadded(AtomicUsize::new(0)),
            pop_cursor: CachePadded(AtomicUsize::new(0)),
        }
    }

    pub fn pop(&self) -> Option<T> {
        // Access inner via .0
        let push_val = self.push_cursor.0.load(Ordering::Acquire);
        let pop_val = self.pop_cursor.0.load(Ordering::Relaxed);

        if push_val == pop_val {
            return None;
        }

        let loc = pop_val % self.capacity;
        let value = unsafe { (*self.ring[loc].get()).take() };

        self.pop_cursor.0.store(pop_val + 1, Ordering::Release);
        value
    }

    pub fn push(&self, item: T) -> bool {
        let push_val = self.push_cursor.0.load(Ordering::Relaxed);
        let pop_val = self.pop_cursor.0.load(Ordering::Acquire);

        if push_val >= pop_val + self.capacity {
            return false;
        }

        let loc = push_val % self.capacity;
        unsafe { *self.ring[loc].get() = Some(item) };

        self.push_cursor.0.store(push_val + 1, Ordering::Release);
        return true;
    }
}

pub fn run_benchmark(iters: usize, capacity: usize) -> f64 {
    let queue = Arc::new(Fifo3::<usize>::new(capacity));
    let done = Arc::new(AtomicBool::new(false));
    let queue_consumer = queue.clone();
    let done_consumer = done.clone();

    let consumer = thread::spawn(move || {
        let mut expected = 0;
        loop {
            if let Some(val) = queue_consumer.pop() {
                assert_eq!(val, expected);
                expected += 1;
            } else {
                if done_consumer.load(Ordering::Acquire) {
                    if queue_consumer.pop().is_none() {
                        break;
                    }
                } else {
                    std::hint::spin_loop();
                }
            }
        }
    });

    let start = Instant::now();

    for i in 0..iters {
        loop {
            if queue.push(i) {
                break;
            }
            std::hint::spin_loop();
        }
    }

    done.store(true, Ordering::Release);
    consumer.join().unwrap();

    let duration = start.elapsed();
    let secs = duration.as_secs_f64();
    println!("Fifo3 Time: {:.4}s, Iters: {}", secs, iters);

    (iters as f64) / secs
}
