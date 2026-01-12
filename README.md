# Lock Free FIFO Queue for low-latency computing

This repository aims to learn lock-free FIFO queue and atomic operations(i.e. CAS, Memory Ordering) based in Rust. The FIFO models we discussed here are covered in [CppCon23 presented by Charles Frasch](https://youtu.be/K3P_Lmq6pw0?si=tXXvZMqwCip_-0sT).


# Machine Spec
- CPU: Apple Silicon M1 Max
- RAM: 32GB Unified Memory
- OS: MacOS Sequoia


# Benchmark

| Benchmark Variant                          | Time (s)   | Iterations   | Throughput (million ops/sec) |
|--------------------------------------------|------------|--------------|------------------------------|
| Fifo1 (Mutex)                              | 3.6715     | 100,000,000  | 27.24                        |
| Fifo2 (Lock-Free)                          | 6.3392     | 100,000,000  | 15.77                        |
| Fifo3 (Cache Padded)                       | 7.0732     | 100,000,000  | 14.14                        |
| Fifo4 (Shadow Cursors + Padding)           | 6.0229     | 100,000,000  | 16.60                        |
| Fifo5 (MaybeUninit + Shadow)               | 6.0836     | 100,000,000  | 16.44                        |
| Fifo6 (Vyukov MPMC Prototype)              | 0.7316     | 100,000,000  | 136.68                       |
| Fifo6a (Vyukov MPMC with bit mask)         | 0.7316     | 100,000,000  | 136.69                       |
| Crossbeam ArrayQueue                       | 0.6853     | 100,000,000  | 145.93                       |


# Remark
I downloaded the source code of Frasch C++ implementation of FIFO queue and do the benchtest of my Mac, the performance/throughput is 10x SLOWER compared to his result listed in his presentation. Since I am not experienced in C++, I cannot explain why I got 10x SLOWER using his code. Gemini3 pro claimed that the discrepancy may likely relate to how the OS scheduling tasks between Linux and MacOS.


# References
- [Frasch's source code of his talk](https://github.com/CharlesFrasch/cppcon2023)
- [Frasch's talk in CppCon23](https://youtu.be/K3P_Lmq6pw0?si=tXXvZMqwCip_-0sT)
- [Rust Atomics and Locks: Low-Level Concurrency in Practice](https://marabos.nl/atomics/)
