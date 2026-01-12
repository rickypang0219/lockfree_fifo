use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

/// Wrapper to force alignment to 128 bytes.
#[repr(align(128))]
struct CachePadded<T>(T);

struct Slot<T> {
    turn: AtomicUsize,
    data: UnsafeCell<MaybeUninit<T>>,
}

pub struct Fifo6<T> {
    capacity: usize,
    // The ring buffer of slots.
    ring: Box<[Slot<T>]>,
    // Head: Consumer index.
    head: CachePadded<AtomicUsize>,
    // Tail: Producer index.
    tail: CachePadded<AtomicUsize>,
}

unsafe impl<T: Send> Sync for Fifo6<T> {}
unsafe impl<T: Send> Send for Fifo6<T> {}

impl<T> Fifo6<T> {
    pub fn new(capacity: usize) -> Fifo6<T> {
        // Prepare slots

        assert!(capacity.is_power_of_two(), "Size must be power of 2!");
        let mut ring = Vec::with_capacity(capacity);
        for i in 0..capacity {
            ring.push(Slot {
                turn: AtomicUsize::new(i),
                data: UnsafeCell::new(MaybeUninit::uninit()),
            });
        }
        let ring = ring.into_boxed_slice();

        Fifo6 {
            capacity,
            ring,
            head: CachePadded(AtomicUsize::new(0)),
            tail: CachePadded(AtomicUsize::new(0)),
        }
    }

    pub fn pop(&self) -> Option<T> {
        loop {
            let head = self.head.0.load(Ordering::Relaxed);
            let index = head & (self.capacity - 1);
            let slot = &self.ring[index];

            // let slot = &self.ring[head % self.capacity];
            let turn = slot.turn.load(Ordering::Acquire);

            // Calculate the difference between the turn and the head + 1.
            // If turn == head + 1: The slot has data for this lap.
            // If turn == head: The slot is empty (producer hasn't filled it yet).
            let diff = turn.wrapping_sub(head.wrapping_add(1));

            if diff == 0 {
                // Try to claim this slot
                if self
                    .head
                    .0
                    .compare_exchange(
                        head,
                        head.wrapping_add(1),
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    // Success! Read the data.
                    let data = unsafe { slot.data.get().read().assume_init() };
                    // Update turn to next lap for producer
                    // Current head was H. Turn becomes H + Capacity.
                    slot.turn
                        .store(head.wrapping_add(self.capacity), Ordering::Release);
                    return Some(data);
                }
            } else if (diff as isize) < 0 {
                // Slot is empty. If calculating for MPMC, we might retry.
                // For SPSC, if head catches up to tail logic (via turn), it means empty.
                // diff < 0 means turn < head + 1.
                // e.g. turn = head (0 vs 1). Empty.
                return None;
            } else {
                // diff > 0. Head fell behind? (MPMC race) or logic error.
                // In SPSC buffer this shouldn't happen unless lap wrapped?
                // Just retry.
            }
        }
    }

    pub fn push(&self, item: T) -> bool {
        loop {
            let tail = self.tail.0.load(Ordering::Relaxed);
            let index = tail & (self.capacity - 1);
            let slot = &self.ring[index];

            // let slot = &self.ring[tail % self.capacity];
            let turn = slot.turn.load(Ordering::Acquire);

            // If turn == tail: The slot is free for this lap.
            // If turn == tail + 1: The slot is full (consumer hasn't taken it).
            let diff = turn.wrapping_sub(tail);

            if diff == 0 {
                // Try to claim
                if self
                    .tail
                    .0
                    .compare_exchange(
                        tail,
                        tail.wrapping_add(1),
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    // Success! Write data.
                    unsafe { slot.data.get().write(MaybeUninit::new(item)) };
                    // Update turn for consumer: becomes tail + 1
                    slot.turn.store(tail.wrapping_add(1), Ordering::Release);
                    return true;
                }
            } else if (diff as isize) < 0 {
                // Slot is full.
                // turn < tail. e.g. turn 0, tail 0 (ok). turn 1, tail 0 (diff 1).
                // Wait, turn is tail + 1 when full.
                // So diff would be 1.
                // If diff < 0? This means tail Wrapped?
                // Actually:
                // Push: turn starts at `i`. we write to `i`. tail is `i`. turn==tail.
                // Set turn to `i+1`.
                // Next push (lap 2): tail `i + cap`. turn needs to be `i + cap`.
                // If turn is still `i + 1`, then `i+1 - (i+cap)` is negative.
                // So diff < 0 implies "Slot Full / Turn lagged".

                // Correction:
                // Full state: Producer wants to write to `tail`.
                // Slot turn is from previous lap's write `tail_prev + 1`.
                // `tail` is `tail_prev + capacity`.
                // `turn` is way behind `tail`.
                // So `turn - tail` is negative.
                return false;
            } else {
                // diff > 0. Tail fell behind. Retry.
            }
        }
    }
}

pub fn run_benchmark(iters: usize, capacity: usize) -> f64 {
    let queue = Arc::new(Fifo6::<usize>::new(capacity));
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
    println!("Fifo6 Time: {:.4}s, Iters: {}", secs, iters);

    (iters as f64) / secs
}
