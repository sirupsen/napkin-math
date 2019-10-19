# Base Rates

This project contains a series of programs to evaluate various 'base rates' that
are useful to know when designing systems. For example, how fast is reading 1
GiB of memory?

Base rates are useful for determining the expected performance of a system.

Run with `cargo run --release`. You won't get the right numbers when you're
compiling in debug mode.

[Slides from a talk on the
subject](https://speakerdeck.com/sirupsen/advanced-napkin-math-estimating-systems-performance-from-first-principles).
Video will appear in the README when it's available.

If you are interested in contributing, write a benchmark for one of the
question-marks below or take a look at the issues!

## Numbers

Here are the numbers from the program, run on my 2017 Macbook. The goal is to
run this on more platform. Note that all numbers don't line up as they've been
rounded to make them more memorable.

| Operation                              | Latency | Throughput | 1 MiB  | 1 GiB  |
|----------------------------------------|---------|------------|--------|--------|
| Sequential Memory Read (64 bit)        | 1 ns    | 6 GiB/s    | 100 us | 100 ms |
| Sequential Memory Writes (64 bit)      | 5 ns    | 1.5 GiB/s  | 500 us | 500 ms |
| Random Memory Read (64 bit)            | 25 ns   | 300 MiB/s  | 3.5 ms | 3.5 s  |
| Mutex Lock/Unlock                      | ?       | ?          | ?      | ?      |
| Random Memory Write (64 bit)           | ?       | ?          | ?      | ?      |
| Sequential SSD Read (8kb)              | 1 us    | 4 GiB/s    | 200 us | 200 ms |
| TCP Echo (TCP overhead) (64 bytes)     | 15 us   | ?          | ?      | ?      |
| Sequential SSD write, -fsync (16KiB)   | 30 us   | 2 MiB/s    | 500 ms | 10 min |
| {Snappy, Gzip, ..} Compression (? KiB) | ?       | ?          | ?      | ?      |
| Hashing (? bytes)                      | ?       | ?          | ?      | ?      |
| Random SSD Seek (64 bytes)             | 100 us  | 500 KiB/s  | 1.5 s  | 30 min |
| Cloud us-east1 to us-east2             | 250 us  | ?          | ?      | ?      |
| {MySQL, Memcached, Redis, ..} Query    | ?       | ?          | ?      | ?      |
| Envoy/Nginx Overhead                   | ?       | ?          | ?      | ?      |
| {JSON, Protobuf, ..} Serializee (?)    | ?       | ?          | ?      | ?      |
| Cloud us-east to us-central            | ?       | ?          | ?      | ?      |


## Cost Numbers

Approximate numbers that should be consistent between Cloud providers.

| What        | Amount | $ / Month |
|-------------|--------|-----------|
| CPU         | 1      | $10       |
| Memory      | 1 GB   | $1        |
| SSD         | 1 GB   | $0.1      |
| Disk        | 1 GB   | $0.01     |
| S3, GCS, .. | 1 GB   | $0.01     |
| Network     | 1 GB   | $0.01     |
