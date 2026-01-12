use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

/// Wrapper to force alignment to 128 bytes.
#[repr(align(128))]
struct CachePadded<T>(T);

struct ProducerFields {
    push_cursor: AtomicUsize,
    cached_pop: UnsafeCell<usize>,
}

struct ConsumerFields {
    pop_cursor: AtomicUsize,
    cached_push: UnsafeCell<usize>,
}

pub struct Fifo5<T> {
    capacity: usize,
    // Raw uninitialized memory. No Option<T> overhead.
    // We treat this as a circular buffer of T.
    ring: Box<[MaybeUninit<T>]>,
    producer: CachePadded<ProducerFields>,
    consumer: CachePadded<ConsumerFields>,
}

unsafe impl<T: Send> Sync for Fifo5<T> {}
unsafe impl<T: Send> Send for Fifo5<T> {}

impl<T> Fifo5<T> {
    pub fn new(capacity: usize) -> Fifo5<T> {
        // Allocate raw memory.
        let mut ring = Vec::with_capacity(capacity);
        ring.resize_with(capacity, MaybeUninit::uninit);
        let ring = ring.into_boxed_slice();

        Fifo5 {
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

        let mut cached_push = unsafe { *consumer.cached_push.get() };

        if pop_val >= cached_push {
            let actual_push = self.producer.0.push_cursor.load(Ordering::Acquire);
            unsafe { *consumer.cached_push.get() = actual_push };
            cached_push = actual_push;

            if pop_val >= cached_push {
                return None;
            }
        }

        let loc = pop_val % self.capacity;
        // SAFETY:
        // 1. We checked push > pop, so data exists.
        // 2. We are the only consumer.
        // 3. We read using ptr::read (memcpy effectively)
        // 4. We do NOT write back to the slot (saving a write vs Option::take).
        // 5. The slot is logically "uninit" for us now, but physically contains old bytes.
        let value = unsafe { self.ring[loc].as_ptr().read() };

        consumer.pop_cursor.store(pop_val + 1, Ordering::Release);
        Some(value)
    }

    pub fn push(&self, item: T) -> bool {
        let producer = &self.producer.0;
        let push_val = producer.push_cursor.load(Ordering::Relaxed);

        let mut cached_pop = unsafe { *producer.cached_pop.get() };

        if push_val >= cached_pop + self.capacity {
            let actual_pop = self.consumer.0.pop_cursor.load(Ordering::Acquire);
            unsafe { *producer.cached_pop.get() = actual_pop };
            cached_pop = actual_pop;

            if push_val >= cached_pop + self.capacity {
                return false;
            }
        }

        let loc = push_val % self.capacity;
        // SAFETY: Slot is free. Write data content directly.
        // We cast the const pointer to mutable because we know we own this slot via SPSC logic.
        unsafe {
            let slot_ptr = self.ring.as_ptr().add(loc) as *mut MaybeUninit<T>;
            slot_ptr.write(MaybeUninit::new(item));
        }

        producer.push_cursor.store(push_val + 1, Ordering::Release);
        return true;
    }
}

// Drop glue: We must drop elements strictly remaining in the queue.
impl<T> Drop for Fifo5<T> {
    fn drop(&mut self) {
        let pop = self.consumer.0.pop_cursor.load(Ordering::Relaxed);
        let push = self.producer.0.push_cursor.load(Ordering::Relaxed);

        // In a real implementation we would drop items from pop..push
        // For benchmarking usize, it's a no-op, but for correctness with T it is required.
        if std::mem::needs_drop::<T>() {
            for i in pop..push {
                let loc = i % self.capacity;
                unsafe { self.ring[loc].as_mut_ptr().drop_in_place() };
            }
        }
    }
}

pub fn run_benchmark(iters: usize, capacity: usize) -> f64 {
    let queue = Arc::new(Fifo5::<usize>::new(capacity));
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
    println!("Fifo5 Time: {:.4}s, Iters: {}", secs, iters);

    (iters as f64) / secs
}
