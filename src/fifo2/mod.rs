use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

/// A Lock-Free SPSC FIFO queue for `usize` values.
/// This implementation is 100% SAFE Rust (no `unsafe` blocks) because it uses
/// `AtomicUsize` for storage. The tradeoff is that it can only store `usize`
/// and reserves `0` as a sentinel value for "Empty".
pub struct Fifo2<T> {
    capacity: usize,
    // Using UnsafeCell to allow interior mutability (writing without &mut self)
    ring: Vec<UnsafeCell<Option<T>>>,
    push_cursor: AtomicUsize,
    pop_cursor: AtomicUsize,
}

// SAFETY: Fifo2 is safe to share across threads assuming SPSC usage.
// We are building a primitive that *requires* correct usage, but strictly speaking
// if T is Send, Fifo2<T> is Send. Sync is Tricky.
// For SPSC, we need to ensure only one pusher and one popper.
unsafe impl<T: Send> Sync for Fifo2<T> {}
unsafe impl<T: Send> Send for Fifo2<T> {}

impl<T> Fifo2<T> {
    pub fn new(capacity: usize) -> Fifo2<T> {
        let mut ring = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            ring.push(UnsafeCell::new(None));
        }
        Fifo2 {
            capacity,
            ring,
            push_cursor: AtomicUsize::new(0),
            pop_cursor: AtomicUsize::new(0),
        }
    }

    pub fn pop(&self) -> Option<T> {
        // Load push_cursor with Acquire to ensure we see the data writes from the producer
        let push_val = self.push_cursor.load(Ordering::Acquire);
        let pop_val = self.pop_cursor.load(Ordering::Relaxed); // We own pop_cursor

        if push_val == pop_val {
            return None;
        }

        let loc = pop_val % self.capacity;
        // SAFETY: We checked that push_val > pop_val, so data is available.
        // Only one consumer accesses ring[loc] at this time.
        // We take the value out, leaving None.
        let value = unsafe { (*self.ring[loc].get()).take() };

        // Release the slot *after* reading
        self.pop_cursor.store(pop_val + 1, Ordering::Release);
        value
    }

    pub fn push(&self, item: T) -> bool {
        let push_val = self.push_cursor.load(Ordering::Relaxed); // We own push_cursor
        let pop_val = self.pop_cursor.load(Ordering::Acquire); // Read consumer's progress

        // size = push - pop. If size == capacity, full.
        if push_val >= pop_val + self.capacity {
            return false;
        }

        let loc = push_val % self.capacity;
        // SAFETY: We checked space is available. Only one producer accesses this slot.
        unsafe { *self.ring[loc].get() = Some(item) };

        // Commit the push *after* writing data
        self.push_cursor.store(push_val + 1, Ordering::Release);
        return true;
    }

    pub fn size(&self) -> usize {
        let push_val = self.push_cursor.load(Ordering::Relaxed);
        let pop_val = self.pop_cursor.load(Ordering::Relaxed);

        if push_val < pop_val {
            return 0; // Should not happen in strict SPSC
        }
        push_val - pop_val
    }
}

pub fn run_benchmark(iters: usize, capacity: usize) -> f64 {
    let queue = Arc::new(Fifo2::<usize>::new(capacity));
    let done = Arc::new(AtomicBool::new(false));
    let queue_consumer = queue.clone();
    let done_consumer = done.clone();

    // Consumer Thread
    let consumer = thread::spawn(move || {
        let mut expected = 0;

        // Loop until done signal AND queue is empty
        loop {
            if let Some(val) = queue_consumer.pop() {
                assert_eq!(val, expected, "Consumer received out-of-order value");
                expected += 1;
            } else {
                // Queue is empty. Check if we are done.
                if done_consumer.load(Ordering::Acquire) {
                    // Double check queue is empty to avoid race where item was pushed
                    // right before we checked done.
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

    // Producer (Main Thread)
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
    println!("Fifo2 Time: {:.4}s, Iters: {}", secs, iters);

    (iters as f64) / secs
}
