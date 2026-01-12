use crossbeam::queue::ArrayQueue;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

pub fn run_benchmark(iters: usize, capacity: usize) -> f64 {
    // Crossbeam's ArrayQueue is MPMC, but works fine for SPSC.
    // It handles dropping items automatically.
    let queue = Arc::new(ArrayQueue::<usize>::new(capacity));
    let done = Arc::new(AtomicBool::new(false));
    let queue_consumer = queue.clone();
    let done_consumer = done.clone();

    let consumer = thread::spawn(move || {
        let mut expected = 0;
        loop {
            // pop() returns Option<T>
            if let Some(val) = queue_consumer.pop() {
                assert_eq!(val, expected);
                expected += 1;
            } else {
                if done_consumer.load(Ordering::Acquire) {
                    if queue_consumer.is_empty() {
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
            if queue.push(i).is_ok() {
                break;
            }
            std::hint::spin_loop();
        }
    }

    done.store(true, Ordering::Release);
    consumer.join().unwrap();

    let duration = start.elapsed();
    let secs = duration.as_secs_f64();
    println!("Crossbeam ArrayQueue Time: {:.4}s, Iters: {}", secs, iters);

    (iters as f64) / secs
}
