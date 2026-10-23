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

| Operation                                        | Latency    | Throughput | 1 GiB  |
| ------------------------------------------------ | ---------- | ---------- | ------ |
| CPU Hash, not crypto-safe (64 bytes)             | 10 ns      | 5 GiB/s    | 200 ms |
| CPU Hash, crypto-safe (64 bytes)                 | 100 ns     | 1 GiB/s    | 1 s    |
| CPU System Call                                  | 100 ns     |            |        |
| CPU Context Switch `[1]` `[2]`                   | 1 μs       |            |        |
| CPU Serialize, fast `[8]` `[9]` †                |            | 1 GiB/s    | 1 s    |
| CPU Serialize, slow `[8]` `[9]` †                |            | 100 MiB/s  | 10 s   |
| CPU Compress `[11]`                              |            | 500 MiB/s  | 2 s    |
| CPU Decompress `[11]`                            |            | 1 GiB/s    | 1 s    |
| CPU Sort (64-bit integers)                       |            | 500 MiB/s  | 3 s    |
| Memory Sequential Read, 1 thread                 |            | 20 GiB/s   | 50 ms  |
| Memory Sequential Read, all cores                |            | 200 GiB/s  | 5 ms   |
| Memory Random Read (64 bytes)                    | 100 ns     | 3 GiB/s    | 300 ms |
| SSD Sequential Read                              |            | 4 GiB/s    | 250 ms |
| SSD Sequential Write                             |            | 2 GiB/s    | 500 ms |
| SSD Random Read (8 KiB)                          | 100 μs     | 2 GiB/s    | 500 ms |
| SSD Sequential Write + fsync (8 KiB)             | 300 μs     | 30 MiB/s   | 30 s   |
| Network Disk Sequential Read                     |            | 1 GiB/s    | 1 s    |
| Network Disk Sequential Write                    |            | 1 GiB/s    | 1 s    |
| Network Disk Random Read (8 KiB)                 | 500 μs     | 250 MiB/s  | 4 s    |
| Network Disk Sequential Write + fsync (8 KiB)    | 1 ms       | 8 MiB/s    | 2 min  |
| HDD Sequential Read                              |            | 200 MiB/s  | 5 s    |
| HDD Sequential Write                             |            | 200 MiB/s  | 5 s    |
| HDD Random Read (8 KiB) `[7]`                    | 10 ms      | 2 MiB/s    | 10 min |
| HDD Sequential Write + fsync (8 KiB)             | 1 ms       | 8 MiB/s    | 2 min  |
| Network Same-Zone                                | 100 μs     | 3 GiB/s    | 300 ms |
| Network Cross-Zone                               | 500 μs     | 2 GiB/s    | 500 ms |
| Network Same-Continent `[6]`                     | 25-60 ms   | 50 MiB/s   | 20 s   |
| Network Cross-Atlantic                           | 80-100 ms  | 25 MiB/s   | 40 s   |
| Network Cross-Pacific                            | 160-180 ms | 25 MiB/s   | 40 s   |
| Proxy Request (Envoy, HAProxy, Nginx, ProxySQL)  | 50 μs      |            |        |
| Database Query (MySQL, Memcached, Redis, ...)    | 500 μs     |            |        |
| Blob Storage GET, 1 connection                   | 80 ms      | 100 MiB/s  | 10 s   |
| Blob Storage PUT, 1 connection                   | 200 ms     | 100 MiB/s  | 10 s   |
| Blob Storage LIST (1000 keys)                    | 100 ms     |            |        |

Throughput is the most you can get, e.g. many requests in flight for random
reads. Network Disk is EBS gp3 / GCP Hyperdisk Balanced provisioned to the max.
Blob storage and network throughput scale with connections up to the VM's
network limit. Region-to-region latencies: [cloudping](https://www.cloudping.co/).

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

| What                | Amount | \$ / Month | Spot \$ /month | Hourly Spot \$ |
| --------------------| ------ | ---------  | ------------- | ------------- |
| CPU                 | 1 vCPU | \$15       | \$7            |  \$0.01        |
| GPU (H100)          | 1      | \$5000     | \$1500         |  \$2           |
| Memory              | 1 GB   | \$2        | \$1            |  \$0.0015      |
| Storage             |        |            |               |               |
| ├ Warehouse Storage | 1 GB   | \$0.02     |               |               |
| ├ Blob (S3, GCS)    | 1 GB   | \$0.02     |               |               |
| ├ Zonal HDD         | 1 GB   | \$0.05     |               |               |
| ├ Ephemeral SSD     | 1 GB   | \$0.08     | \$0.05         |  \$0.00007     |
| ├ Regional HDD      | 1 GB   | \$0.1      |               |               |
| ├ Zonal SSD         | 1 GB   | \$0.2      |               |               |
| ├ Regional SSD      | 1 GB   | \$0.35     |               |               |
| Networking          |        |            |               |               |
| ├ Same Zone         | 1 GB   | \$0        |               |               |
| ├ Blob              | 1 GB   | \$0        |               |               |
| ├ Ingress           | 1 GB   | \$0        |               |               |
| ├ L4 LB             | 1 GB   | \$0.008    |               |               |
| ├ Inter-Zone        | 1 GB   | \$0.01     |               |               |
| ├ Inter-Region      | 1 GB   | \$0.02     |               |               |
| ├ Internet Egress ††| 1 GB   | \$0.1      |               |               |
| CDN Egress          | 1 GB   | \$0.05     |               |               |
| CDN Fill ‡          | 1 GB   | \$0.01     |               |               |
| Warehouse Query     | 1 GB   | \$0.005    |               |               |
| Logs/Traces ♣       | 1 GB   | \$0.5      |               |               |
| Metrics (series)    | 1000   | \$20       |               |               |
| KMS Keys            | 1      | \$1        |               |               |

†† This refers to network leaving your cloud provider, e.g. sending data to S3
from GCP or egress network for sending HTML from AWS to a client.

‡ CDNs also charge ~\$1 per 1M requests, cache hits included.

♣ [Datadog](https://www.datadoghq.com/pricing/?product=log-management#products)
charges \$0.1/GB ingested plus ~\$1.5 per 1M indexed events for 7d retention.

Furthermore, for blob storage (S3/GCS/R2/...), you're charged per read/write
operation (fewer, large files is cheaper):

 |                | 1M      | 1000     |
 |----------------|---------|----------|
 | Reads          | \$0.4   | \$0.0004 |
 | Writes         | \$5     | \$0.005  |
 | KMS Encryption | \$3     | \$0.003  |

## Compression Ratios

This is sourced from a few sources. `[3]` `[4]` `[5]` Note that compression
speeds (but generally not ratios) vary by orders of magnitude depending on the
algorithm and the level of compression (which trades speed for compression).

I typically ballpark that another _x in compression ratio decreases performance
by 10x_. E.g. on the Silesia corpus, lz4 gets 2x at ~500 MiB/s, zstd -9 3.5x at
~50 MiB/s and zstd -19 4x at ~3 MiB/s.

| What                   | Compression Ratio |
| ---------------------- | ----------------- |
| HTML                   | 3-8x              |
| English                | 2-4x              |
| Source Code            | 3-7x              |
| Executables            | 2-3x              |
| RPC (JSON)             | 5-20x             |
| Encrypted / compressed | 1x `[10]`         |

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
* [Systems Performance (book)](https://www.amazon.com/Systems-Performance-Enterprise-Brendan-Gregg/dp/0133390098/ref=sr_1_1?keywords=systems+performance&qid=1580733419&sr=8-1). Fantastic book about analyzing system performance, finding bottlenecks, and understanding operating systems.
* [Files are hard](https://danluu.com/file-consistency/)

[fio]: https://github.com/axboe/fio
