### release, client with TCP_QUICKACK

| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| rr | 1 | rust-release | 37.0 k | 2.81 | 22 µs | 50 µs | 88 µs | 4.48 | 11.85 | 3.8 | 21 | 3 | 2/3 contended (load1/runnable: 23.6/15.2, 15.9/5.0, 21.9/29.8) |
| rr | 1 | llvm-release | 42.2 k | 3.21 | 22 µs | 45 µs | 80 µs | 3.66 | 8.81 | 2.9 | 2 | 3 | 2/3 contended (load1/runnable: 22.0/16.7, 14.5/7.7, 22.3/20.1) |
| rr | 16 | rust-release | 201.8 k | 15.34 | 62 µs | 229 µs | 930 µs | 21.39 | 10.82 | 4.9 | 21 | 3 | 1/3 contended (load1/runnable: 16.3/5.0, 25.2/33.5, 13.9/0.9) |
| rr | 16 | llvm-release | 437.1 k | 33.22 | 33 µs | 72 µs | 306 µs | 52.34 | 12.23 | 4.4 | 17 | 3 | no (load1/runnable: 17.6/8.0, 27.1/5.8, 15.2/0.5) |
| rr | 256 | rust-release | 450.3 k | 34.22 | 512 µs | 2.20 ms | 4.72 ms | 62.32 | 14.54 | 9.1 | 21 | 3 | no (load1/runnable: 22.3/4.3, 26.2/2.5, 18.2/1.5) |
| rr | 256 | llvm-release | 532.0 k | 40.43 | 473 µs | 965 µs | 3.48 ms | 60.05 | 11.29 | 23.8 | 257 | 3 | no (load1/runnable: 30.0/3.1, 34.3/0.5, 22.0/2.8) |
| pipe | 1 | rust-release | 288.2 k | 21.90 | 220 µs | 369 µs | 396 µs | 10.90 | 3.78 | 3.9 | 21 | 3 | 1/3 contended (load1/runnable: 120.2/16.6, 121.4/6.0, 123.2/0.8) |
| pipe | 1 | llvm-release | 298.1 k | 22.66 | 212 µs | 323 µs | 347 µs | 9.23 | 3.10 | 3.2 | 2 | 3 | 1/3 contended (load1/runnable: 96.7/1.3, 95.8/21.8, 104.5/1.1) |
| pipe | 16 | rust-release | 1.61 M | 122.52 | 398 µs | 3.14 ms | 4.55 ms | 85.47 | 5.30 | 5.6 | 21 | 3 | no (load1/runnable: 64.3/7.8, 55.4/1.6, 59.6/0.5) |
| pipe | 16 | llvm-release | 878.7 k | 66.78 | 390 µs | 4.49 ms | 6.26 ms | 83.22 | 9.26 | 5.5 | 17 | 3 | no (load1/runnable: 59.0/6.3, 51.0/3.6, 52.4/2.0) |
| pipe | 256 | rust-release | 642.7 k | 48.84 | 20.74 ms | 92.15 ms | 128.79 ms | 65.71 | 10.22 | 10.9 | 21 | 3 | 2/3 contended (load1/runnable: 64.8/48.2, 51.9/31.1, 40.7/0.8) |
| pipe | 256 | llvm-release | 2.52 M | 191.26 | 5.84 ms | 20.73 ms | 31.53 ms | 121.24 | 4.82 | 42.7 | 257 | 3 | 2/3 contended (load1/runnable: 67.4/39.2, 54.0/32.0, 40.3/0.8) |
| connect | 1 | rust-release | 6.5 k | 0.10 | 142 µs | 403 µs | 3.62 ms | 4.04 | 61.47 | 3.9 | 21 | 3 | 2/3 contended (load1/runnable: 137.0/41.0, 132.2/81.2, 100.1/1.6) |
| connect | 1 | llvm-release | 10.6 k | 0.16 | 68 µs | 681 µs | 3.50 ms | 4.79 | 44.99 | 5.1 | 2 | 3 | 1/3 contended (load1/runnable: 114.7/26.1, 118.0/3.0, 85.1/4.8) |
| connect | 16 | rust-release | 82.0 k | 1.23 | 175 µs | 481 µs | 1.19 ms | 28.40 | 34.28 | 5.2 | 21 | 3 | no (load1/runnable: 72.2/4.8, 68.0/3.5, 51.1/1.1) |
| connect | 16 | llvm-release | 23.6 k | 0.35 | 362 µs | 15.01 ms | 29.08 ms | 12.16 | 51.42 | 7.1 | 6 | 3 | 2/3 contended (load1/runnable: 57.9/14.2, 62.6/52.6, 45.8/2.5) |

### debug, client with TCP_QUICKACK

| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| rr | 1 | rust-debug | 21.3 k | 1.62 | 46 µs | 90 µs | 122 µs | 8.35 | 39.19 | 4.4 | 21 | 3 | 1/3 contended (load1/runnable: 20.2/5.5, 16.6/44.9, 19.4/2.2) |
| rr | 1 | llvm-debug | 34.0 k | 2.58 | 29 µs | 51 µs | 79 µs | 5.79 | 16.64 | 3.0 | 2 | 3 | 1/3 contended (load1/runnable: 18.6/6.0, 21.2/35.9, 15.9/1.2) |
| rr | 16 | rust-debug | 190.2 k | 14.46 | 76 µs | 260 µs | 466 µs | 72.33 | 38.02 | 5.6 | 21 | 3 | 1/3 contended (load1/runnable: 22.0/6.2, 30.4/23.9, 15.6/1.2) |
| rr | 16 | llvm-debug | 284.1 k | 21.59 | 56 µs | 113 µs | 218 µs | 89.64 | 31.87 | 4.4 | 17 | 3 | no (load1/runnable: 20.8/4.3, 26.0/3.2, 16.0/1.2) |
| rr | 256 | rust-debug | 251.5 k | 19.11 | 985 µs | 2.20 ms | 4.18 ms | 126.68 | 50.37 | 8.4 | 21 | 3 | no (load1/runnable: 84.0/3.1, 90.7/0.7, 104.6/1.6) |
| rr | 256 | llvm-debug | 367.4 k | 27.92 | 682 µs | 1.50 ms | 4.48 ms | 95.68 | 26.17 | 24.8 | 257 | 3 | 1/3 contended (load1/runnable: 72.2/19.2, 79.1/0.3, 94.2/1.5) |
| pipe | 1 | rust-debug | 81.4 k | 6.19 | 782 µs | 1.79 ms | 2.99 ms | 11.41 | 14.02 | 4.4 | 21 | 3 | 1/3 contended (load1/runnable: 85.5/11.2, 81.9/4.2, 88.7/0.6) |
| pipe | 1 | llvm-debug | 87.8 k | 6.67 | 725 µs | 1.17 ms | 1.24 ms | 8.82 | 10.04 | 3.2 | 2 | 3 | no (load1/runnable: 71.9/3.1, 70.6/3.7, 69.6/1.4) |
| pipe | 16 | rust-debug | 194.7 k | 14.80 | 4.31 ms | 17.92 ms | 25.46 ms | 45.95 | 23.60 | 5.9 | 21 | 3 | 2/3 contended (load1/runnable: 55.4/38.8, 48.0/35.4, 48.8/1.1) |
| pipe | 16 | llvm-debug | 224.3 k | 17.05 | 3.94 ms | 13.42 ms | 18.26 ms | 50.44 | 20.76 | 5.8 | 17 | 3 | 2/3 contended (load1/runnable: 59.5/80.2, 50.0/47.1, 45.6/0.2) |
| pipe | 256 | rust-debug | 259.0 k | 19.68 | 54.13 ms | 193.62 ms | 259.67 ms | 70.20 | 28.75 | 11.1 | 21 | 3 | 2/3 contended (load1/runnable: 108.7/29.7, 95.0/86.7, 77.7/1.2) |
| pipe | 256 | llvm-debug | 818.2 k | 62.18 | 18.65 ms | 52.56 ms | 71.71 ms | 135.16 | 16.52 | 43.8 | 257 | 3 | 2/3 contended (load1/runnable: 98.3/27.9, 99.9/85.0, 66.2/1.4) |
| connect | 1 | rust-debug | 5.5 k | 0.08 | 136 µs | 747 µs | 3.50 ms | 8.34 | 136.49 | 4.5 | 21 | 3 | 1/3 contended (load1/runnable: 100.6/23.6, 100.8/2.2, 68.6/6.8) |
| connect | 1 | llvm-debug | 11.8 k | 0.18 | 81 µs | 142 µs | 263 µs | 7.08 | 59.86 | 5.3 | 2 | 3 | 1/3 contended (load1/runnable: 83.6/4.8, 79.7/6.8, 62.7/25.2) |
| connect | 16 | rust-debug | 67.4 k | 1.01 | 214 µs | 656 µs | 1.14 ms | 59.08 | 87.51 | 5.6 | 21 | 3 | 1/3 contended (load1/runnable: 52.0/22.7, 57.4/2.4, 40.3/0.8) |
| connect | 16 | llvm-debug | 37.4 k | 0.56 | 416 µs | 620 µs | 2.38 ms | 23.86 | 63.72 | 9.9 | 6 | 3 | 1/3 contended (load1/runnable: 47.9/23.9, 51.3/3.4, 34.0/1.2) |

### release, stock client

| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| rr | 1 | rust-release | 24.1 | 0.00 | 41.01 ms | 42.18 ms | 43.00 ms | 0.00 | 0.00 | 3.8 | 21 | 3 | 1/3 contended (load1/runnable: 43.3/28.5, 41.5/3.0, 29.8/2.4) |
| rr | 1 | llvm-release | 24.0 | 0.00 | 41.01 ms | 42.09 ms | 42.25 ms | 0.00 | 0.00 | 3.0 | 2 | 3 | 1/3 contended (load1/runnable: 40.4/30.0, 35.7/4.5, 25.3/3.1) |
| rr | 16 | rust-release | 385.6 | 0.03 | 41.02 ms | 42.14 ms | 43.35 ms | 0.09 | 23.38 | 4.4 | 21 | 3 | 2/3 contended (load1/runnable: 39.5/30.4, 21.2/11.6, 17.0/1.2) |
| rr | 16 | llvm-release | 386.6 | 0.03 | 41.01 ms | 42.97 ms | 45.01 ms | 0.09 | 23.28 | 4.3 | 17 | 3 | no (load1/runnable: 36.9/3.2, 19.3/8.0, 14.6/1.9) |
| rr | 256 | rust-release | 6.2 k | 0.47 | 41.03 ms | 42.18 ms | 42.89 ms | 0.94 | 15.23 | 8.6 | 21 | 3 | no (load1/runnable: 23.7/8.2, 16.3/6.5, 9.1/0.2) |
| rr | 256 | llvm-release | 6.2 k | 0.47 | 41.02 ms | 42.18 ms | 42.98 ms | 1.00 | 16.21 | 23.6 | 257 | 3 | 1/3 contended (load1/runnable: 19.6/17.4, 14.3/6.8, 7.1/0.5) |
| pipe | 1 | rust-release | 1.5 k | 0.12 | 41.02 ms | 42.64 ms | 42.99 ms | 0.06 | 3.91 | 3.8 | 21 | 3 | 1/3 contended (load1/runnable: 11.4/36.2, 11.7/6.2, 4.0/1.2) |
| pipe | 1 | llvm-release | 1.5 k | 0.12 | 41.01 ms | 42.14 ms | 42.53 ms | 0.04 | 2.59 | 2.9 | 2 | 3 | 1/3 contended (load1/runnable: 14.1/1.6, 10.8/5.8, 4.2/12.2) |
| pipe | 16 | rust-release | 24.7 k | 1.88 | 41.02 ms | 42.27 ms | 42.83 ms | 0.92 | 3.72 | 4.3 | 21 | 3 | no (load1/runnable: 8.8/1.6, 10.8/8.6, 4.0/3.5) |
| pipe | 16 | llvm-release | 24.7 k | 1.87 | 41.01 ms | 42.15 ms | 43.89 ms | 0.77 | 3.12 | 5.3 | 17 | 3 | no (load1/runnable: 7.6/3.5, 10.7/8.4, 3.7/0.4) |
| pipe | 256 | rust-release | 394.0 k | 29.95 | 41.02 ms | 42.71 ms | 47.08 ms | 12.52 | 3.17 | 8.6 | 21 | 3 | no (load1/runnable: 6.2/2.8, 10.0/2.1, 4.7/0.7) |
| pipe | 256 | llvm-release | 394.8 k | 30.00 | 41.02 ms | 43.03 ms | 46.01 ms | 14.11 | 3.57 | 39.6 | 257 | 3 | 1/3 contended (load1/runnable: 8.9/36.6, 8.7/5.3, 4.0/1.1) |
| connect | 1 | rust-release | 15.8 k | 0.24 | 60 µs | 130 µs | 178 µs | 5.73 | 36.55 | 3.8 | 21 | 3 | 1/3 contended (load1/runnable: 27.0/29.4, 6.6/1.4, 24.1/2.8) |
| connect | 1 | llvm-release | 14.0 k | 0.21 | 63 µs | 139 µs | 569 µs | 5.96 | 42.66 | 5.4 | 2 | 3 | no (load1/runnable: 23.3/1.5, 6.2/4.0, 21.6/3.9) |
| connect | 16 | rust-release | 82.9 k | 1.24 | 148 µs | 415 µs | 1.97 ms | 26.04 | 31.40 | 5.1 | 21 | 3 | 1/3 contended (load1/runnable: 14.4/27.5, 11.2/4.3, 14.8/3.0) |
| connect | 16 | llvm-release | 41.0 k | 0.61 | 351 µs | 1.46 ms | 8.01 ms | 19.39 | 47.39 | 10.9 | 6 | 3 | 1/3 contended (load1/runnable: 18.4/6.0, 11.6/13.3, 14.3/0.7) |

### debug, stock client

| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| rr | 1 | rust-debug | 24.1 | 0.00 | 41.01 ms | 43.90 ms | 44.00 ms | 0.02 | 83.33 | 4.3 | 21 | 3 | 1/3 contended (load1/runnable: 42.4/48.6, 28.7/0.8, 22.8/3.0) |
| rr | 1 | llvm-debug | 24.1 | 0.00 | 41.01 ms | 42.09 ms | 43.91 ms | 0.01 | 41.49 | 3.0 | 2 | 3 | 1/3 contended (load1/runnable: 41.4/33.1, 25.0/8.2, 20.1/4.5) |
| rr | 16 | rust-debug | 386.2 | 0.03 | 41.01 ms | 42.43 ms | 43.05 ms | 0.26 | 67.43 | 5.0 | 21 | 3 | no (load1/runnable: 33.7/7.2, 19.5/4.6, 12.7/1.1) |
| rr | 16 | llvm-debug | 387.2 | 0.03 | 41.01 ms | 42.09 ms | 42.20 ms | 0.15 | 38.75 | 4.5 | 17 | 3 | no (load1/runnable: 27.7/9.5, 18.1/5.8, 10.8/0.1) |
| rr | 256 | rust-debug | 6.2 k | 0.47 | 41.03 ms | 42.58 ms | 43.39 ms | 2.63 | 42.61 | 9.0 | 21 | 3 | no (load1/runnable: 16.9/1.1, 13.0/6.5, 6.0/0.7) |
| rr | 256 | llvm-debug | 6.2 k | 0.47 | 41.03 ms | 42.42 ms | 43.11 ms | 1.90 | 30.79 | 24.7 | 257 | 3 | no (load1/runnable: 14.4/0.9, 12.2/6.8, 4.7/0.1) |
| pipe | 1 | rust-debug | 1.5 k | 0.12 | 41.01 ms | 42.99 ms | 43.48 ms | 0.17 | 11.02 | 4.4 | 21 | 3 | no (load1/runnable: 11.2/0.3, 10.1/4.7, 4.8/2.0) |
| pipe | 1 | llvm-debug | 1.5 k | 0.12 | 41.00 ms | 42.08 ms | 42.14 ms | 0.15 | 9.69 | 3.1 | 2 | 3 | no (load1/runnable: 10.6/1.4, 11.8/7.0, 4.4/2.5) |
| pipe | 16 | rust-debug | 24.7 k | 1.88 | 41.02 ms | 42.97 ms | 44.43 ms | 2.98 | 12.12 | 5.1 | 21 | 3 | 1/3 contended (load1/runnable: 6.9/1.4, 10.6/13.1, 6.0/6.2) |
| pipe | 16 | llvm-debug | 24.7 k | 1.88 | 41.01 ms | 42.08 ms | 42.17 ms | 2.31 | 9.32 | 5.5 | 17 | 3 | 1/3 contended (load1/runnable: 6.9/8.7, 11.3/16.6, 5.3/1.2) |
| pipe | 256 | rust-debug | 393.5 k | 29.91 | 41.02 ms | 44.62 ms | 49.57 ms | 44.23 | 11.22 | 9.6 | 21 | 3 | 2/3 contended (load1/runnable: 12.5/33.5, 7.8/3.0, 3.5/46.5) |
| pipe | 256 | llvm-debug | 394.0 k | 29.95 | 41.01 ms | 44.04 ms | 47.96 ms | 48.05 | 12.27 | 40.9 | 257 | 3 | 1/3 contended (load1/runnable: 19.1/31.1, 6.7/2.6, 6.4/0.6) |
| connect | 1 | rust-debug | 10.8 k | 0.16 | 89 µs | 148 µs | 416 µs | 10.68 | 99.92 | 4.4 | 21 | 3 | 1/3 contended (load1/runnable: 20.0/0.8, 7.9/26.0, 20.4/1.1) |
| connect | 1 | llvm-debug | 10.4 k | 0.16 | 72 µs | 224 µs | 959 µs | 6.80 | 60.82 | 5.0 | 2 | 3 | 1/3 contended (load1/runnable: 16.6/0.6, 10.4/30.2, 18.1/4.0) |
| connect | 16 | rust-debug | 56.7 k | 0.85 | 213 µs | 1.48 ms | 3.62 ms | 48.76 | 86.06 | 5.7 | 21 | 3 | 1/3 contended (load1/runnable: 16.9/3.5, 14.7/38.9, 14.2/6.0) |
| connect | 16 | llvm-debug | 29.4 k | 0.44 | 460 µs | 3.25 ms | 10.02 ms | 20.40 | 69.50 | 8.4 | 7 | 3 | 1/3 contended (load1/runnable: 18.1/8.7, 19.5/32.6, 15.3/2.4) |

### cells with a contended run

- quickack=False llvm-debug connect 1
- quickack=False llvm-debug connect 16
- quickack=False llvm-debug pipe 16
- quickack=False llvm-debug pipe 256
- quickack=False llvm-debug rr 1
- quickack=False llvm-release connect 16
- quickack=False llvm-release pipe 1
- quickack=False llvm-release pipe 256
- quickack=False llvm-release rr 1
- quickack=False llvm-release rr 256
- quickack=False rust-debug connect 1
- quickack=False rust-debug connect 16
- quickack=False rust-debug pipe 16
- quickack=False rust-debug pipe 256
- quickack=False rust-debug rr 1
- quickack=False rust-release connect 1
- quickack=False rust-release connect 16
- quickack=False rust-release pipe 1
- quickack=False rust-release rr 1
- quickack=False rust-release rr 16
- quickack=True llvm-debug connect 1
- quickack=True llvm-debug connect 16
- quickack=True llvm-debug pipe 16
- quickack=True llvm-debug pipe 256
- quickack=True llvm-debug rr 1
- quickack=True llvm-debug rr 256
- quickack=True llvm-release connect 1
- quickack=True llvm-release connect 16
- quickack=True llvm-release pipe 1
- quickack=True llvm-release pipe 256
- quickack=True llvm-release rr 1
- quickack=True rust-debug connect 1
- quickack=True rust-debug connect 16
- quickack=True rust-debug pipe 1
- quickack=True rust-debug pipe 16
- quickack=True rust-debug pipe 256
- quickack=True rust-debug rr 1
- quickack=True rust-debug rr 16
- quickack=True rust-release connect 1
- quickack=True rust-release pipe 1
- quickack=True rust-release pipe 256
- quickack=True rust-release rr 1
- quickack=True rust-release rr 16
