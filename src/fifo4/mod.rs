use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

/// Wrapper to force alignment to 128 bytes (Apple Silicon / standard cache line).
#[repr(align(64))]
struct CachePadded<T>(T);

/// Fields exclusive to the Producer thread.
struct ProducerFields {
    push_cursor: AtomicUsize,
    // A local copy of the consumer's pop cursor.
    // This allows the producer to check for space *without* reading the shared atomic
    // pop_cursor variables (which causes cache coherence traffic) until necessary.
    cached_pop: UnsafeCell<usize>,
}

/// Fields exclusive to the Consumer thread.
struct ConsumerFields {
    pop_cursor: AtomicUsize,
    // A local copy of the producer's push cursor.
    cached_push: UnsafeCell<usize>,
}

pub struct Fifo4<T> {
    capacity: usize,
    ring: Vec<UnsafeCell<Option<T>>>,
    // Grouping mutable fields that are accessed together to maximize cache locality
    // and minimize False Sharing between producer and consumer.
    producer: CachePadded<ProducerFields>,
    consumer: CachePadded<ConsumerFields>,
}

// SAFETY: SPSC only.
unsafe impl<T: Send> Sync for Fifo4<T> {}
unsafe impl<T: Send> Send for Fifo4<T> {}

impl<T> Fifo4<T> {
    pub fn new(capacity: usize) -> Fifo4<T> {
        let mut ring = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            ring.push(UnsafeCell::new(None));
        }
        Fifo4 {
            capacity,
            ring,
            producer: CachePadded(ProducerFields {
                push_cursor: AtomicUsize::new(0),
                cached_pop: UnsafeCell::new(0),
            }),
            consumer: CachePadded(ConsumerFields {
                pop_cursor: AtomicUsize::new(0),
                cached_push: UnsafeCell::new(0),
            }),
        }
    }

    pub fn pop(&self) -> Option<T> {
        let consumer = &self.consumer.0;
        let pop_val = consumer.pop_cursor.load(Ordering::Relaxed);

        // Read our cached view of the producer
        // Safe because only Consumer calls pop, so only Consumer mutates cached_push
        let mut cached_push = unsafe { *consumer.cached_push.get() };

        // If it looks empty, check the REAL push cursor
        if pop_val >= cached_push {
            let actual_push = self.producer.0.push_cursor.load(Ordering::Acquire);
            // Update our cache
            unsafe { *consumer.cached_push.get() = actual_push };
            cached_push = actual_push;

            if pop_val >= cached_push {
                return None; // Really empty
            }
        }

        let loc = pop_val % self.capacity;
        let value = unsafe { (*self.ring[loc].get()).take() };

        consumer.pop_cursor.store(pop_val + 1, Ordering::Release);
        value
    }

    pub fn push(&self, item: T) -> bool {
        let producer = &self.producer.0;
        let push_val = producer.push_cursor.load(Ordering::Relaxed);

        // Read our cached view of the consumer
        let mut cached_pop = unsafe { *producer.cached_pop.get() };

        // If it looks full, check the REAL pop cursor
        if push_val >= cached_pop + self.capacity {
            let actual_pop = self.consumer.0.pop_cursor.load(Ordering::Acquire);
            unsafe { *producer.cached_pop.get() = actual_pop };
            cached_pop = actual_pop;

            if push_val >= cached_pop + self.capacity {
                return false; // Really full
            }
        }

        let loc = push_val % self.capacity;
        unsafe { *self.ring[loc].get() = Some(item) };

        producer.push_cursor.store(push_val + 1, Ordering::Release);
        return true;
    }
}

pub fn run_benchmark(iters: usize, capacity: usize) -> f64 {
    let queue = Arc::new(Fifo4::<usize>::new(capacity));
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
    println!("Fifo4 Time: {:.4}s, Iters: {}", secs, iters);

    (iters as f64) / secs
}
