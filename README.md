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

| Operation                                                 | Latency    | Throughput | 1 GiB  |
| --------------------------------------------------------- | ---------- | ---------- | ------ |
| Memory Sequential Read, all cores                         |            | 200 GiB/s  | 5 ms   |
| L1 Cache Read                                             | 1 ns       | 200 GiB/s  | 5 ms   |
| Memory Sequential Write, all cores                        |            | 150 GiB/s  | 7 ms   |
| L2 Cache Read                                             | 5 ns       | 150 GiB/s  | 7 ms   |
| L3 Cache Read                                             | 30 ns      | 30 GiB/s   | 30 ms  |
| Memory Sequential Read, 1 thread                          |            | 20 GiB/s   | 50 ms  |
| Memory Sequential Write, 1 thread                         |            | 10 GiB/s   | 100 ms |
| CPU Encrypt / Decrypt (AES-256-GCM)                       |            | 10 GiB/s   | 100 ms |
| CPU Encode / Decode Bitpacked Integers                    |            | 10 GiB/s   | 100 ms |
| CPU Hash, not crypto-safe (64 bytes)                      | 10 ns      | 5 GiB/s    | 200 ms |
| SSD Sequential Read                                       |            | 4 GiB/s    | 250 ms |
| Network Bandwidth                                         |            | 3 GiB/s    | 300 ms |
| Network Same-Zone                                         | 100 μs     | NW Limit   |        |
| Network Cross-Zone                                        | 500 μs     | NW Limit   |        |
| Network Same-Continent                                    | 25-60 ms   | NW Limit\* |        |
| Network Cross-Atlantic                                    | 80-100 ms  | NW Limit\* |        |
| Network Cross-Pacific                                     | 160-180 ms | NW Limit\* |        |
| Blob Storage, Zonal Read (128 KiB)                        | 50 ms      | NW Limit   |        |
| Blob Storage, Regional Read (128 KiB)                     | 100 ms     | NW Limit   |        |
| Blob Storage, Regional Write (128 KiB)                    | 100 ms     | NW Limit   |        |
| Blob Storage, Zonal Write (128 KiB)                       | 100 ms     | NW Limit   |        |
| Memory Random Write (64 bytes)                            |            | 3 GiB/s    | 300 ms |
| Memory Random Read (64 bytes)                             | 100 ns     | 3 GiB/s    | 300 ms |
| SSD Sequential Write                                      |            | 2 GiB/s    | 500 ms |
| CPU Decode Parquet (dictionary + zstd)                    |            | 2 GiB/s    | 500 ms |
| SSD Random Write (8 KiB)                                  | 30 μs      | 2 GiB/s    | 500 ms |
| SSD Sequential Durable Write (64 KiB)†                    | 50 μs      | 2 GiB/s    | 500 ms |
| SSD Random Read (8 KiB)                                   | 100 μs     | 2 GiB/s    | 500 ms |
| CPU Serialize (JSON, Protobuf)                            |            | 1 GiB/s    | 1 s    |
| CPU Decompress (lz4, zstd -1)                             |            | 1 GiB/s    | 1 s    |
| Network Disk, Zonal Sequential Read                       |            | 1 GiB/s    | 1 s    |
| Network Disk, Regional Sequential Read                    |            | 1 GiB/s    | 1 s    |
| Network Disk, Zonal Sequential Write                      |            | 1 GiB/s    | 1 s    |
| Network Disk, Regional Sequential Write                   |            | 1 GiB/s    | 1 s    |
| CPU Hash, crypto-safe (64 bytes)                          | 100 ns     | 1 GiB/s    | 1 s    |
| Network Disk, Zonal Sequential Durable Write (64 KiB)†    | 1 ms       | 1 GiB/s    | 1 s    |
| Network Disk, Regional Sequential Durable Write (64 KiB)† | 2 ms       | 1 GiB/s    | 1 s    |
| CPU Compress (lz4, zstd -1)                               |            | 500 MiB/s  | 2 s    |
| CPU Sort (64-bit integers)                                |            | 500 MiB/s  | 3 s    |
| CPU Deserialize (JSON, Protobuf)                          |            | 300 MiB/s  | 3 s    |
| CPU Encode Parquet (dictionary + zstd)                    |            | 300 MiB/s  | 3 s    |
| Network Disk, Zonal Random Read (8 KiB)                   | 500 μs     | 250 MiB/s  | 4 s    |
| Network Disk, Regional Random Read (8 KiB)                | 500 μs     | 250 MiB/s  | 4 s    |
| Network Disk, Zonal Random Write (8 KiB)                  | 500 μs     | 250 MiB/s  | 4 s    |
| Network Disk, Regional Random Write (8 KiB)               | 1 ms       | 250 MiB/s  | 4 s    |
| HDD Sequential Read                                       |            | 200 MiB/s  | 5 s    |
| HDD Sequential Write                                      |            | 200 MiB/s  | 5 s    |
| HDD Sequential Durable Write (8 KiB)†                     | 1 ms       | 8 MiB/s    | 2 min  |
| HDD Random Write (8 KiB)                                  | 5 ms       | 2 MiB/s    | 10 min |
| HDD Random Read (8 KiB)                                   | 10 ms      | 2 MiB/s    | 10 min |
| CPU Atomic Increment                                      | 5 ns       |            |        |
| CPU Mutex Lock + Unlock                                   | 10 ns      |            |        |
| CPU System Call                                           | 100 ns     |            |        |
| CPU Atomic Increment, contended                           | 100 ns     |            |        |
| CPU Mutex Lock + Unlock, contended                        | 500 ns     |            |        |
| CPU Context Switch                                        | 1 μs       |            |        |
| CPU TCP Connect (+1 RTT)                                  | 10 μs      |            |        |
| Proxy Request (Envoy, HAProxy, Nginx, ProxySQL)           | 50 μs      |            |        |
| CPU Thread Spawn                                          | 50 μs      |            |        |
| CPU TLS 1.3 Handshake (+1 RTT)                            | 250 μs     |            |        |
| CPU Process Spawn                                         | 300 μs     |            |        |
| Database Query (MySQL, Memcached, Redis, ...)             | 500 μs     |            |        |
| Blob Storage, Regional Conditional Write (128 KiB)        | 100 ms     |            |        |
| Blob Storage, Zonal List (1000 keys)                      | 100 ms     |            |        |
| Blob Storage, Regional List (1000 keys)                   | 150 ms     |            |        |
| Blob Storage, Zonal Conditional Write (128 KiB)           | 500 ms     |            |        |

† pwrite + fdatasync into a preallocated file. Cloud SSDs and network disks
have no write cache to flush (Azure Premium SSD flushes in ~30 μs), so a durable
write costs a write.

\* One TCP connection moves ~2 MiB per round trip, so 2 MiB / RTT: 20 MiB/s
at 100 ms. Add connections to go faster.

Throughput is with enough requests in flight to max out a 48 vCPU VM (AWS m8,
GCP C4) and its disks (EBS gp3, Hyperdisk Balanced at max provisioning). Regional
disks copy every write to a second zone, so only their write latency differs. Blob
Storage latency is p99 from the same region; Zonal is S3 Express One Zone and
GCS Rapid. Region-to-region latencies:
[cloudping](https://www.cloudping.co/).

## Cost Numbers

Approximate numbers that should be consistent between Cloud providers.

| What                          | Amount | \$ / Month | Spot \$ /month | Hourly Spot \$ |
| ----------------------------- | ------ | ---------- | -------------- | -------------- |
| CPU                           | 1 vCPU | \$15       | \$7            | \$0.01         |
| GPU (H100)                    | 1      | \$5000     | \$1500         | \$2            |
| Memory                        | 1 GB   | \$2        | \$1            | \$0.0015       |
| Storage                       |        |            |                |                |
| ├ Warehouse Storage           | 1 GB   | \$0.02     |                |                |
| ├ Blob Storage, Regional      | 1 GB   | \$0.02     |                |                |
| ├ Blob Storage, Zonal         | 1 GB   | \$0.1      |                |                |
| ├ Network Disk, Zonal, HDD    | 1 GB   | \$0.05     |                |                |
| ├ SSD                         | 1 GB   | \$0.08     | \$0.05         | \$0.00007      |
| ├ Network Disk, Regional, HDD | 1 GB   | \$0.1      |                |                |
| ├ Network Disk, Zonal         | 1 GB   | \$0.2      |                |                |
| ├ Network Disk, Regional      | 1 GB   | \$0.35     |                |                |
| Network                       |        |            |                |                |
| ├ Same-Zone                   | 1 GB   | \$0        |                |                |
| ├ Blob Storage, Same-Region   | 1 GB   | \$0        |                |                |
| ├ Ingress                     | 1 GB   | \$0        |                |                |
| ├ L4 Load Balancer            | 1 GB   | \$0.008    |                |                |
| ├ Cross-Zone                  | 1 GB   | \$0.01     |                |                |
| ├ Cross-Region                | 1 GB   | \$0.02     |                |                |
| ├ Internet Egress ††          | 1 GB   | \$0.1      |                |                |
| CDN Egress                    | 1 GB   | \$0.05     |                |                |
| CDN Fill ‡                    | 1 GB   | \$0.01     |                |                |
| Warehouse Query               | 1 GB   | \$0.005    |                |                |
| Logs/Traces ♣                 | 1 GB   | \$0.5      |                |                |
| Metrics (series)              | 1000   | \$20       |                |                |
| KMS Keys                      | 1      | \$1        |                |                |

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

Measured with `cpu_compress`; each range spans lz4 to zstd -19. Note
that compression speeds (but generally not ratios) vary by orders of magnitude
depending on the algorithm and the level of compression (which trades speed for
compression).

I typically ballpark that another _x in compression ratio decreases performance
by 10x_. E.g. on the Silesia corpus, lz4 gets 2x at ~500 MiB/s, zstd -9 3.5x at
~50 MiB/s and zstd -19 4x at ~3 MiB/s.

| What                   | Compression Ratio |
| ---------------------- | ----------------- |
| HTML                   | 3-8x              |
| English                | 2-4x              |
| Source Code            | 3-9x              |
| Executables            | 2-3x              |
| RPC (JSON)             | 5-15x             |
| Logs                   | 6-30x             |
| Encrypted / compressed | 1x                |

Structured data, against its in-memory size (8 bytes per float64, the type's
width per integer, the bytes plus a 4-byte offset per string), measured on real
datasets with `compression_ratio` and `compression_ratio_columns`:

| What                       | Encoding                        | Ratio or size               |
| -------------------------- | ------------------------------- | --------------------------- |
| Columnar table             | Parquet dictionary + RLE + zstd | 10x                         |
| Floats                     | ALP                             | 4x                          |
| Integers                   | FOR / delta + bitpacking        | 2-4x                        |
| Strings                    | FSST                            | 2x                          |
| Low-cardinality column     | Dictionary                      | log2(distinct) bits / value |
| Sorted or repeating column | RLE                             | 3 bytes / run               |

ALP gets the same ratio as zstd on floats and decodes at ~10 GiB/s on one core.
Floats that use the full mantissa (coordinates, model weights) barely compress
at all. Bitpacked integers take log2 of each 1024-value range in bits and decode
at ~15 GiB/s; sorted or low-cardinality columns need RLE or a dictionary first.
FSST decodes at ~2 GiB/s and keeps every string decodable on its own, where zstd
over a whole column gets 3-10x. Dictionary and RLE columns also pay for the
dictionary itself (each distinct value once); the smaller of the two rules
predicts Parquet's dictionary + RLE size of whole tables within 10%.

Search indexes, against the size of the text they index (documents of ~16 KiB
for n-grams; at one line per document they're as big as the text):

| What                       | Stores                                 | Ratio |
| -------------------------- | -------------------------------------- | ----- |
| Inverted index             | Terms, doc IDs, frequencies            | 4x    |
| Inverted index + positions | Terms, doc IDs, frequencies, positions | 3x    |
| Trigram index (regex)      | Trigrams, doc IDs                      | 10x   |
| Fourgram index (regex)     | Fourgrams, doc IDs                     | 4x    |

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

* [Systems Performance (book)](https://www.amazon.com/Systems-Performance-Enterprise-Brendan-Gregg/dp/0133390098/ref=sr_1_1?keywords=systems+performance&qid=1580733419&sr=8-1). Fantastic book about analyzing system performance, finding bottlenecks, and understanding operating systems.
* [Files are hard](https://danluu.com/file-consistency/)

