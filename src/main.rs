mod fifo1;
mod fifo2;
mod fifo3;
mod fifo4;
mod fifo5;
mod fifo6;
mod fifo6a;
mod fifo_crossbeam;

fn main() {
    let iters = 100_000_000;
    let capacity = 131_072;

    println!("Running Fifo1 Benchmark...");
    let ops_per_sec = fifo1::run_benchmark(iters, capacity);
    println!(
        "Fifo1 Throughput: {:.2} million ops/sec",
        ops_per_sec / 1_000_000.0
    );

    println!("\nRunning Fifo2 (Lock-Free) Benchmark...");
    let ops_per_sec2 = fifo2::run_benchmark(iters, capacity);
    println!(
        "Fifo2 Throughput: {:.2} million ops/sec",
        ops_per_sec2 / 1_000_000.0
    );

    println!("\nRunning Fifo3 (Cache Padded) Benchmark...");
    let ops_per_sec3 = fifo3::run_benchmark(iters, capacity);
    println!(
        "Fifo3 Throughput: {:.2} million ops/sec",
        ops_per_sec3 / 1_000_000.0
    );

    println!("\nRunning Fifo4 (Shadow Cursors + Padding) Benchmark...");
    let ops_per_sec4 = fifo4::run_benchmark(iters, capacity);
    println!(
        "Fifo4 Throughput: {:.2} million ops/sec",
        ops_per_sec4 / 1_000_000.0
    );

    println!("\nRunning Fifo5 (MaybeUninit + Shadow) Benchmark...");
    let ops_per_sec5 = fifo5::run_benchmark(iters, capacity);
    println!(
        "Fifo5 Throughput: {:.2} million ops/sec",
        ops_per_sec5 / 1_000_000.0
    );

    println!("\nRunning Fifo6 (Vyukov MPMC Prototype) Benchmark...");
    let ops_per_sec6_proto = fifo6::run_benchmark(iters, capacity);
    println!(
        "Fifo6 Throughput: {:.2} million ops/sec",
        ops_per_sec6_proto / 1_000_000.0
    );

    println!("\nRunning Fifo6a (Vyukov MPMC Prototype with bit mask) Benchmark...");
    let ops_per_sec6_proto = fifo6a::run_benchmark(iters, capacity);
    println!(
        "Fifo6 Throughput: {:.2} million ops/sec",
        ops_per_sec6_proto / 1_000_000.0
    );

    println!("\nRunning Crossbeam ArrayQueue Benchmark...");
    let ops_per_sec6 = fifo_crossbeam::run_benchmark(iters, capacity);
    println!(
        "Crossbeam Throughput: {:.2} million ops/sec",
        ops_per_sec6 / 1_000_000.0
    );
}
