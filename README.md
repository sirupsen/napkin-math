# Napkin Math

The goal of this project is to collect software, numbers, and techniques to
quickly estimate the expected performance of systems from first-principles. For
example, how quickly can you read 1 GB of memory? By composing these resources
you should be able to answer interesting questions like: how much storage cost
should you expect to pay for logging for an application with 100,000 RPS?

The best introduction to this skill is [through my talk at
SRECON](https://www.youtube.com/watch?v=IxkSlnrRFqc).

The best way to practise napkin math in the grand domain of computers is to work
on your own problems. The second-best is to **subscribe to [this
newsletter](http://sirupsen.com/napkin) where you'll get a problem every few
weeks to practise on**. It should only take you a few minutes to solve each one as your
facility with these techniques improve.

The archive of problems to practise with are
[here](https://sirupsen.com/napkin/). The solution will be in the following
newsletter.

## Numbers

Below are numbers rounded for memorization, not faux precision. Last measured
in October 2026 on GCP `c4-standard-48-lssd` and AWS `m8id.12xlarge` /
`m8gd.12xlarge` (Graviton4). See [BENCHMARKING.md](BENCHMARKING.md) to run them
yourself.

**Note 1:** Some throughput and latency numbers don't line up, this is
intentional for ease of calculations.

**Note 2:** Take the numbers with a grain of salt. E.g. for I/O, [`fio`][fio] is
the state-of-the-art. I am continuously updating these numbers as I learn more
to improve accuracy and as hardware improves.

| Operation                           | Latency     | Throughput | 1 MiB  | 1 GiB  |
| ----------------------------------- | -------     | ---------- | ------ | ------ |
| Sequential Memory R/W (64 bytes)    | 3 ns        |            |        |        |
| ├ Single Thread                     |             | 20 GiB/s   | 50 μs  | 50 ms  |
| ├ Threaded                          |             | 200 GiB/s  | 5 μs   | 5 ms   |
| Network Same-Zone                   | 100 μs      | 3 GiB/s    | 300 μs | 300 ms |
| ├ Inside VPC                        | 100 μs      | 3 GiB/s    | 300 μs | 300 ms |
| ├ Outside VPC                       |             | 1 GiB/s    | 1 ms   | 1s     |
| Hashing, not crypto-safe (64 bytes) | 10 ns       | 5 GiB/s    | 200 μs | 200 ms |
| Random Memory R/W (64 bytes)        | 100 ns      | 3 GiB/s    | 300 μs | 300 ms |
| Fast Serialization `[8]` `[9]` †    | N/A         | 1 GiB/s    | 1 ms   | 1s     |
| Fast Deserialization `[8]` `[9]` †  | N/A         | 1 GiB/s    | 1 ms   | 1s     |
| System Call                         | 100 ns      | N/A        | N/A    | N/A    |
| ├ stat()                            | 300 ns      | N/A        | N/A    | N/A    |
| Hashing, crypto-safe (64 bytes)     | 100 ns      | 1 GiB/s    | 1 ms   | 1s     |
| Sequential SSD read (8 KiB)         | 2 μs        | 4 GiB/s    | 250 μs | 250 ms |
| Context Switch, same core `[1] [2]` | 1 μs        | N/A        | N/A    | N/A    |
| ├ Cross-core thread wake-up         | 2 μs        | N/A        | N/A    | N/A    |
| Sequential SSD write, -fsync (8KiB) | 4 μs        | 2 GiB/s    | 500 μs | 500 ms |
| TCP Echo, loopback (32 KiB)         | 50 μs       | 500 MiB/s  | 2 ms   | 2s     |
| Random SSD Read, QD1 (8 KiB)        | 100 μs      | 70 MiB/s   | 15 ms  | 15s    |
| ├ Parallel, QD32+ (1 NVMe)          |             | 2 GiB/s    | 500 μs | 500 ms |
| Decompression `[11]`                | N/A         | 1 GiB/s    | 1 ms   | 1s     |
| Compression, fast `[11]`            | N/A         | 500 MiB/s  | 2 ms   | 2s     |
| Sorting (64-bit integers)           | N/A         | 500 MiB/s  | 2 ms   | 3s     |
| Proxy: Envoy/ProxySQL/Nginx/HAProxy | 50 μs       | ?          | ?      | ?      |
| Network cross-zone, same region     | 500 μs      | 2 GiB/s    | 500 μs | 500 ms |
| Premium network within zone/VPC     | 50 μs       | 25 GiB/s   | 40 μs  | 40 ms  |
| Sequential SSD write, +fsync (8KiB) | 300 μs      | 30 MiB/s   | 30 ms  | 30s    |
| {MySQL, Memcached, Redis, ..} Query | 500 μs      | ?          | ?      | ?      |
| Serialization `[8]` `[9]` †         | N/A         | 100 MiB/s  | 10 ms  | 10s    |
| Deserialization `[8]` `[9]` †       | N/A         | 100 MiB/s  | 10 ms  | 10s    |
| Sequential HDD Read (8 KiB) `[7]`   | 10 ms       | 250 MiB/s  | 4 ms   | 4s     |
| Random HDD Read (8 KiB) `[7]`       | 10 ms       | 0.7 MiB/s  | 2 s    | 30 min |
| Blob Storage GET, If-None-Match 304 | 30 ms       |            |        |        |
| Blob Storage GET, 1 conn (128KiB)   | 80 ms       | 100 MiB/s  | 10 ms  | 10s    |
| Blob Storage GET, n conn (offsets)  | 80 ms       | NW limit   |        |        |
| Blob Storage LIST (1000 keys)       | 100 ms      |            |        |        |
| Blob Storage PUT, 1 conn (128KiB)   | 200 ms      | 100 MiB/s  | 10 ms  | 10s    |
| Blob Storage PUT, n conn (multipart)| 200 ms      | NW limit   |        |        |
| Network between regions `[6]`       | [Varies][i] | 25 MiB/s   | 40 ms  | 40s    |
| Network NA Central <-> East         | 25 ms       | 100 MiB/s  | 10 ms  | 10s    |
| Network NA Central <-> West         | 40 ms       | 50 MiB/s   | 20 ms  | 20s    |
| Network NA East <-> West            | 60 ms       | 50 MiB/s   | 20 ms  | 20s    |
| Network EU West <-> NA East         | 80 ms       | 25 MiB/s   | 40 ms  | 40s    |
| Network EU West <-> NA Central      | 100 ms      | 25 MiB/s   | 40 ms  | 40s    |
| Network NA West <-> Singapore       | 180 ms      | 25 MiB/s   | 40 ms  | 40s    |
| Network EU West <-> Singapore       | 160 ms      | 25 MiB/s   | 40 ms  | 40s    |

[i]: https://www.cloudping.co/

**†:** "Fast serialization/deserialization" is typically a simple wire-protocol
that just dumps bytes, or a very efficient environment. Typically standard
serialization such as e.g. JSON will be of the slower kind. We include both here
as serialization/deserialization is a very, very broad topic with extremely
different performance characteristics depending on data and implementation.

I am aware of some inefficiencies in this suite. I intend to improve my skills
in this area, in order to ensure the numbers are the upper-bound of performance
you may be able to squeeze out in production. I find it highly unlikely any of
them will be more than 2-3x off, which shouldn't be a problem for most users.

## Cost Numbers

Approximate numbers that should be consistent between Cloud providers.

| What                | Amount | \$ / Month | 1y commit \$ /month | Spot \$ /month | Hourly Spot \$ |
| --------------------| ------ | ---------  | ------------------ | ------------- | ------------- |
| CPU ♥               | 1 vCPU | \$15       | \$10                | \$7            |  \$0.01        |
| GPU (H100)          | 1      | \$5000     | \$3000              | \$1500         |  \$2           |
| Memory ♥            | 1 GB   | \$2        | \$1                 | \$1            |  \$0.0015      |
| Storage             |        |            |                    |               |               |
| ├ Warehouse Storage | 1 GB   | \$0.02     |                    |               |               |
| ├ Blob (S3, GCS)    | 1 GB   | \$0.02     |                    |               |               |
| ├ Zonal HDD         | 1 GB   | \$0.05     |                    |               |               |
| ├ Ephemeral SSD     | 1 GB   | \$0.08     | \$0.05             | \$0.05        |  \$0.00007     |
| ├ Regional HDD      | 1 GB   | \$0.1      |                    |               |               |
| ├ Zonal SSD ♠       | 1 GB   | \$0.2      |                    |               |               |
| ├ Regional SSD ♠    | 1 GB   | \$0.35     |                    |               |               |
| Networking          |        |            |                    |               |               |
| ├ Same Zone         | 1 GB   | \$0        |                    |               |               |
| ├ Blob              | 1 GB   | \$0        |                    |               |               |
| ├ Ingress           | 1 GB   | \$0        |                    |               |               |
| ├ L4 LB             | 1 GB   | \$0.008    |                    |               |               |
| ├ Inter-Zone ◆      | 1 GB   | \$0.01     |                    |               |               |
| ├ Inter-Region      | 1 GB   | \$0.02     |                    |               |               |
| ├ Internet Egress ††| 1 GB   | \$0.1      |                    |               |               |
| CDN Egress          | 1 GB   | \$0.05     |                    |               |               |
| CDN Fill ‡          | 1 GB   | \$0.01     |                    |               |               |
| Warehouse Query     | 1 GB   | \$0.005    |                    |               |               |
| Logs/Traces    ♣    | 1 GB   | \$0.5      |                    |               |               |
| Metrics (series)    | 1000   | \$20       |                    |               |               |
| KMS Keys            | 1      | \$1        |                    |               |               |

♥ Per vCPU, which on x86 is one hyperthread. The on-demand and 1y prices match
GCP E2; current general-purpose families (AWS m7i, GCP N2/C4, Azure Dsv5) are
~1.5x that (≈\$25/vCPU and \$3/GB on-demand). Spot discounts vary from 40% to
80% by provider, region and month (October 2026: GCP ~40% off, AWS ~55-65%,
Azure ~80%), so on GCP spot now costs about the same as a 1y commit.

♠ Performance-tier network SSD (GCP `pd-ssd` and regional `pd-ssd`). Baseline
network SSD (AWS gp3, GCP Hyperdisk Balanced, Azure Premium SSD v2) is
~\$0.08/GB with ~3k IOPS / ~125 MB/s included; provisioning `pd-ssd`-class IOPS
on those brings it back to ~\$0.2.

◆ Per side: AWS bills \$0.01/GB to both sender and receiver (\$0.02 per GB
moved, the same as US inter-region), GCP bills the sender only, and Azure
doesn't charge between availability zones.

†† This refers to network leaving your cloud provider, e.g. sending data to S3
from GCP or egress network for sending HTML from AWS to a client.

‡ CDNs also charge ~\$1 per 1M requests on every request, cache hits included
(~2x the blob read cost, see just below); each cache miss also pays the
origin's blob GET.

♣ This is standard pricing among a few logging providers, but e.g. [Datadog
pricing](https://www.datadoghq.com/pricing/?product=log-management#products) is
different and charges \$0.1 per ingested GB plus ~\$1.5 per 1M indexed events
for 7d retention (≈\$1.5-2/GB at ~1 KB/event).

KMS keys are \$1/month on AWS KMS (and GCP Cloud HSM); GCP Cloud EKM keys in an
external key manager are ~\$3/month per key version.

Furthermore, for blob storage (S3/GCS/R2/...), you're charged per read/write
operation (fewer, large files is cheaper):

 |                | 1M      | 1000     |
 |----------------|---------|----------|
 | Reads          | \$0.4   | \$0.0004 |
 | Writes         | \$5     | \$0.005  |
 | KMS Encryption | \$3     | \$0.003  |

## Compression Ratios

This is sourced from a few sources. `[3]` `[4]` `[5]` Note that compression
speeds (but generally not ratios) vary by two to three orders of magnitude
depending on the algorithm and the level of compression (which trades speed for
compression).

I typically ballpark that once you're past the fast codecs, _another x in
compression ratio decreases compression speed by ~10x_. On the Silesia corpus
(single thread, measured October 2026 with `src/bin/compression_corpus.rs`) we
get 2x at ~500 MiB/s (lz4, snappy), 3x at ~250 MiB/s (zstd -3), 3.5x at ~50
MiB/s (zstd -9) and 4x at ~3 MiB/s (zstd -19). Decompression stays at ~1 GiB/s
for zstd at every level.

The ranges below go from fast codecs (lz4/snappy) at the low end to zstd -19 /
brotli -11 at the high end; gzip -6 and zstd -3 land in the middle.

| What                           | Compression Ratio |
| ------------------------------ | ----------------- |
| HTML                           | 3-8x              |
| English                        | 2-4x              |
| Source Code                    | 3-7x              |
| Executables                    | 2-3x              |
| RPC (JSON)                     | 5-20x             |
| Encrypted or already compressed | ~1x, TLS adds ~2% `[10]` |

These were re-checked in October 2026: modern web pages compressed a median
6.3x with gzip -6 (2.4-9.5x per page), C source 2.8-5.6x (Silesia `samba`) and
3.1-7.1x (`/usr/include`), English 1.6-3.6x (Silesia `dickens`), executables
1.4-3.9x (Silesia `mozilla`/`ooffice`, Ubuntu `/usr/bin`), and paged GitHub API
JSON 5-26x (10.6x with gzip -6, 17x with zstd -3).

## Techniques

* **Don't overcomplicate.** If you are basing your calculation on more than 6
    assumptions, you're likely making it harder than it should be.
* **Keep the units.** They're good checksumming.
    [Wolframalpha](https://wolframalpha.com) has terrific support if you need a
    hand in converting e.g. KiB to TiB.
* **Calculate with exponents.** A lot of back-of-the-envelope calculations are
    done with just coefficients and exponents, e.g. `c * 10^e`. Your goal is to
    get within an order of magnitude right--that's just `e`. `c` matters a lot
    less. Only worrying about single-digit coefficients and exponents makes it
    much easier on a napkin (not to speak of all the zeros you avoid writing).
* **Perform Fermi decomposition.** Write down things you can guess at until you
    can start to hint at an answer. When you want to know the cost of storage
    for logging, you're going to want to know how big a log line is, how many of
    those you have per second, what that costs, and so on.

## Resources

* `[1]`: https://eli.thegreenplace.net/2018/measuring-context-switching-and-memory-overheads-for-linux-threads/
* `[2]`: https://blog.tsunanet.net/2010/11/how-long-does-it-take-to-make-context.html
* `[3]`: https://cran.r-project.org/web/packages/brotli/vignettes/brotli-2015-09-22.pdf
* `[4]`: https://github.com/google/snappy
* `[5]`: https://quixdb.github.io/squash-benchmark/
* `[6]`: https://dl.acm.org/doi/10.1145/1879141.1879143
* `[7]`: https://en.wikipedia.org/wiki/Hard_disk_drive_performance_characteristics#Seek_times_&_characteristics
* `[8]`: https://github.com/simdjson/simdjson#performance-results
* `[9]`: https://github.com/protocolbuffers/protobuf/blob/d20e9a92/docs/performance.md
* `[10]`: https://www.imperialviolet.org/2010/06/25/overclocking-ssl.html
* `[11]`: https://github.com/inikep/lzbench
* ["How to get consistent results when benchmarking on
  Linux?"](https://easyperf.net/blog/2019/08/02/Perf-measurement-environment-on-Linux#2-disable-hyper-threading).
  Great compilation of various Kernel and CPU features to toggle for reliable
  bench-marking, e.g. CPU affinity, disabling turbo boost, etc. It also has
  resources on proper statistical methods for benchmarking.
* [LLVM benchmarking tips](https://www.llvm.org/docs/Benchmarking.html). Similar
  to the above in terms of dedicating CPUs, disabling address space
  randomization, etc.
* [Top-Down performance analysis
  methodology](https://easyperf.net/blog/2019/02/09/Top-Down-performance-analysis-methodology).
  Useful post about using `toplev` to find the bottlenecks. This is particularly
  useful for the benchmarking suite we have here, to ensure the programs are
  correctly written (I have not taken them through this yet, but plan to).
* [Godbolt's compiler explorer](https://gcc.godbolt.org/#). Fantastic resource
  for comparing assembly between Rust and e.g. C with Clang/GCC.
* [cargo-show-asm](https://github.com/pacak/cargo-show-asm). Cargo extension to allow
  disassembling functions. Unfortunately the support for closure is a bit
  lacking, which requires some refactoring.
* [Agner's Assembly
  Guide](https://www.agner.org/optimize/optimizing_assembly.pdf). An excellent
  resource on writing optimum assembly, which will be useful to inspect the
  various functions for inefficiencies in our suite.
* [Agner's Instruction
  Tables](https://www.agner.org/optimize/instruction_tables.pdf). Thorough
  resource on the expected throughput for various instructions which is helpful
  to inspect the assembly.
* [halobates.de](http://halobates.de/). Useful resource for low-level
  performance by the author of `toplev`.
* [Systems Performance (book)](https://www.amazon.com/Systems-Performance-Enterprise-Brendan-Gregg/dp/0133390098/ref=sr_1_1?keywords=systems+performance&qid=1580733419&sr=8-1). Fantastic book about analyzing system performance, finding bottlenecks, and understanding operating systems.
* [io_uring](https://lwn.net/Articles/776703/). Best summary, it links to many
  resources.
* [Integer Compression Comparisons](https://github.com/powturbo/TurboPFor-Integer-Compression)
* [Files are hard](https://danluu.com/file-consistency/)

[fio]: https://github.com/axboe/fio
