---
id: performance
title: Performance
---

!!! note
    * **Memory SIMD-Vector processing performance only**
    * Dataset: 100,000,000,000 (100 Billion)
    * Hardware: AMD Ryzen 7 PRO 4750U, 8 CPU Cores, 16 Threads
    * Rust: rustc 1.55.0-nightly (868c702d0 2021-06-30)
    * Build with Link-time Optimization and Using CPU Specific Instructions
    * ClickHouse server version 21.4.6 revision 54447

| Query                                                        | FuseQuery (v0.4.48-nightly)                                  | ClickHouse (v21.4.6)                                         |
| ------------------------------------------------------------ | --------------------------------------------------- | ------------------------------------------------------------ |
| SELECT avg(number) FROM numbers_mt(100000000000)             | 4.35 s.<br /> (22.97 billion rows/s., 183.91 GB/s.) | **×1.4 slow, (6.04 s.)** <br /> (16.57 billion rows/s., 132.52 GB/s.) |
| SELECT sum(number) FROM numbers_mt(100000000000)             | 4.20 s.<br />(23.79 billion rows/s., 190.50 GB/s.)  | **×1.4 slow, (5.90 s.)** <br />(16.95 billion rows/s., 135.62 GB/s.) |
| SELECT min(number) FROM numbers_mt(100000000000)             | 4.92 s.<br />(20.31 billion rows/s., 162.64 GB/s.)  | **×2.7 slow, (13.05 s.)** <br /> (7.66 billion rows/s., 61.26 GB/s.) |
| SELECT max(number) FROM numbers_mt(100000000000)             | 4.77 s.<br />(20.95 billion rows/s., 167.78 GB/s.)  | **×3.0 slow, (14.07 s.)** <br /> (7.11 billion rows/s., 56.86 GB/s.) |
| SELECT count(number) FROM numbers_mt(100000000000)           | 2.91 s.<br />(34.33 billion rows/s., 274.90 GB/s.)  | **×1.3 slow, (3.71 s.)** <br /> (26.93 billion rows/s., 215.43 GB/s.) |
| SELECT sum(number+number+number) FROM numbers_mt(100000000000) | 19.83 s.<br />(5.04 billion rows/s., 40.37 GB/s.)   | **×12.1 slow, (233.71 s.)** <br /> (427.87 million rows/s., 3.42 GB/s.) |
| SELECT sum(number) / count(number) FROM numbers_mt(100000000000) | 3.90 s.<br />(25.62 billion rows/s., 205.13 GB/s.)  | **×2.5 slow, (9.70 s.)** <br /> (10.31 billion rows/s., 82.52 GB/s.) |
| SELECT sum(number) / count(number), max(number), min(number) FROM numbers_mt(100000000000) | 8.28 s.<br />(12.07 billion rows/s., 96.66 GB/s.)   | **×4.0 slow, (32.87 s.)** <br /> (3.04 billion rows/s., 24.34 GB/s.) |
| SELECT number FROM numbers_mt(10000000000) ORDER BY number DESC LIMIT 100 | 4.80 s.<br />(2.08 billion rows/s., 16.67 GB/s.)    | **×2.9 slow, (13.95 s.)** <br /> (716.62 million rows/s., 5.73 GB/s.) |
| SELECT max(number), sum(number) FROM numbers_mt(1000000000) GROUP BY number % 3, number % 4, number % 5 | 6.31 s.<br />(158.49 million rows/s., 1.27 GB/s.) | **×1.02 fast, (6.18 s.)** <br /> (161.84 million rows/s., 1.29 GB/s.) |

!!! note "Notes"
    ClickHouse system.numbers_mt is <b>16-way</b> parallelism processing, [gist](https://gist.github.com/BohuTANG/bba7ec2c23da8017eced7118b59fc7d5) 

    FuseQuery system.numbers_mt is <b>16-way</b> parallelism processing, [gist](https://gist.github.com/BohuTANG/8c37f5390e129cfc9d648ff930d9ef03)

<figure>
  <img src="https://datafuse-1253727613.cos.ap-hongkong.myqcloud.com/datafuse-avg-100b.gif"/>
  <figcaption>100,000,000,000 records on laptop show</figcaption>
</figure>

Experience 100 billion performance on your laptop, [talk is cheap just bench it](building-and-running.md)
