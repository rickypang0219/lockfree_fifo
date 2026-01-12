use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

pub struct Fifo1<T> {
    capacity: usize,
    ring: Vec<Option<T>>,
    push_cursor: usize,
    pop_cursor: usize,
}

impl<T> Fifo1<T> {
    pub fn new(capacity: usize) -> Fifo1<T> {
        let mut ring = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            ring.push(None);
        }
        Fifo1 {
            capacity,
            ring,
            push_cursor: 0,
            pop_cursor: 0,
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.size() == 0 {
            return None;
        }
        let loc = self.pop_cursor % self.capacity;
        let value = self.ring[loc].take();
        self.pop_cursor += 1;
        value
    }

    pub fn push(&mut self, item: T) -> bool {
        if self.is_full() {
            return false;
        };
        let loc = self.push_cursor % self.capacity;
        self.ring[loc] = Some(item);
        self.push_cursor += 1;
        return true;
    }

    pub fn size(&self) -> usize {
        // In a circular buffer where push and pop are monotonic, push >= pop is invariant.
        assert!(self.push_cursor >= self.pop_cursor);
        self.push_cursor - self.pop_cursor
    }

    pub fn is_full(&self) -> bool {
        self.capacity == self.size()
    }

    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }
}

pub fn run_benchmark(iters: usize, capacity: usize) -> f64 {
    let queue = Arc::new(Mutex::new(Fifo1::<usize>::new(capacity)));
    let done = Arc::new(AtomicBool::new(false));
    let queue_consumer = queue.clone();
    let done_consumer = done.clone();

    // Consumer Thread
    let consumer = thread::spawn(move || {
        let mut expected = 0;
        while !done_consumer.load(Ordering::Acquire) {
            loop {
                // Try to pop; if empty, we spin (or yield)
                let mut guard = queue_consumer.lock().unwrap();
                if let Some(val) = guard.pop() {
                    assert_eq!(val, expected, "Consumer received out-of-order value");
                    expected += 1;
                    break;
                }
                drop(guard);
                // In a real lock-free queue, we might cpu_relax here.
                // With a Mutex, we rely on unlock to yield.
            }
        }

        // Final drain after producer is done
        loop {
            let mut guard = queue_consumer.lock().unwrap();
            if let Some(val) = guard.pop() {
                assert_eq!(val, expected, "Drain received out-of-order value");
                expected += 1;
            } else {
                break;
            }
        }
    });

    let start = Instant::now();

    // Producer (Main Thread)
    for i in 0..iters {
        loop {
            let mut guard = queue.lock().unwrap();
            if guard.push(i) {
                break;
            }
            drop(guard);
            // Spin if full
        }
    }

    // Signal consumer to stop waiting for new data once empty
    // But we need to make sure the consumer knows we are done purely producing.
    // The consumer loop condition !done might exit early if we set it here and the queue is not empty yet?
    // No, the consumer loop checks !done. If done is true, it exits the main loop.
    // However, the queue might still have items. The "Final drain" handles that.
    // Crucially, we must ensure all items pushed are consumed.
    // The current logic:
    // 1. Producer pushes all items.
    // 2. Producer sets `done = true`.
    // 3. Consumer sees `done = true`, exits main loop.
    // 4. Consumer enters drain loop.
    // Race condition: Producer sets done=true. Consumer checks done=true BEFORE popping the last item in the main loop.
    // Consumer exits main loop. Consumer enters drain loop. Drain loop pops remaining items.
    // This seems correct.

    done.store(true, Ordering::Release);

    consumer.join().unwrap();

    let duration = start.elapsed();
    let secs = duration.as_secs_f64();
    println!("Time: {:.4}s, Iters: {}", secs, iters);

    (iters as f64) / secs
}
