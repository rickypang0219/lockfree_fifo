# Lock Free FIFO Queue for low-latency computing


# Benchmark

| Benchmark Variant                          | Time (s)   | Iterations   | Throughput (million ops/sec) |
|--------------------------------------------|------------|--------------|------------------------------|
| Fifo1                                      | 3.6715     | 100,000,000  | 27.24                        |
| Fifo2 (Lock-Free)                          | 6.3392     | 100,000,000  | 15.77                        |
| Fifo3 (Cache Padded)                       | 7.0732     | 100,000,000  | 14.14                        |
| Fifo4 (Shadow Cursors + Padding)           | 6.0229     | 100,000,000  | 16.60                        |
| Fifo5 (MaybeUninit + Shadow)               | 6.0836     | 100,000,000  | 16.44                        |
| Fifo6 (Vyukov MPMC Prototype)              | 0.7316     | 100,000,000  | 136.68                       |
| Fifo6a (Vyukov MPMC with bit mask)         | 0.7316     | 100,000,000  | 136.69                       |
| Crossbeam ArrayQueue                       | 0.6853     | 100,000,000  | 145.93                       |
