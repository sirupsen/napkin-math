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
in October-November 2026, averaged over GCP C4 and AWS m8i (x86) and m8g
(Graviton4), 48 vCPU each, and their local-SSD variants; network disks on
Hyperdisk, EBS and pd-standard, HDDs on an AWS d3. Blob storage was measured in
March 2026, and the GPU rows are NVIDIA's specs. See
[BENCHMARKING.md](BENCHMARKING.md) to run them yourself.

| Operation                                                                | Latency  | Throughput |
| ------------------------------------------------------------------------ | -------- | ---------- |
| GPU Memory Sequential Read (B200)                                        |          | 7 TiB/s    |
| GPU ↔ GPU Transfer, NVLink (B200)                                        |          | 800 GiB/s  |
| L1 Cache Read, 1 thread                                                  | 2 ns     | 200 GiB/s  |
| Memory Sequential Read, all cores                                        | 20 ns    | 200 GiB/s  |
| Memory Sequential Write, all cores                                       | 30 ns    | 200 GiB/s  |
| L2 Cache Read, 1 thread                                                  | 5 ns     | 150 GiB/s  |
| Memory Random Read (64 B), all cores                                     | 150 ns   | 100 GiB/s  |
| Memory Random Write (64 B), all cores                                    | 150 ns   | 100 GiB/s  |
| Memory Sequential Read, other socket, all cores‡                         | 100 ns   | 60 GiB/s   |
| CPU Decode Bitpacked Integers (128 at a time), 1 thread                  | 10 ns    | 50 GiB/s   |
| CPU Hash, not crypto-safe (xxh3, 4 KiB), 1 thread                        | 100 ns   | 50 GiB/s   |
| GPU ↔ CPU Transfer, PCIe (B200)                                          | 10 μs    | 50 GiB/s   |
| L3 Cache Read, 1 thread                                                  | 50 ns    | 40 GiB/s   |
| GPU ↔ GPU Network, RDMA (400 Gbit/s)                                     | 5 μs     | 40 GiB/s   |
| Memory Sequential Read, 1 thread                                         | 5 ns     | 30 GiB/s   |
| Memory Sequential Write, 1 thread                                        | 10 ns    | 30 GiB/s   |
| CPU Hash, not crypto-safe (xxh3, 64 B), 1 thread                         | 5 ns     | 20 GiB/s   |
| Memory Sequential Read, other socket, 1 thread‡                          | 5 ns     | 15 GiB/s   |
| CPU Encode Bitpacked Integers (128 at a time), 1 thread                  | 50 ns    | 10 GiB/s   |
| CPU Encrypt / Decrypt (AES-256-GCM, 16 KiB), 1 thread                    | 2 μs     | 10 GiB/s   |
| SSD Sequential Read (8 KiB), 1 thread                                    | 50 μs    | 5 GiB/s    |
| SSD Random Read (8 KiB), 2 threads                                       | 100 μs   | 5 GiB/s    |
| Memory Random Read (64 B), 1 thread                                      | 100 ns   | 3 GiB/s    |
| Memory Random Write (64 B), 1 thread                                     | 150 ns   | 3 GiB/s    |
| SSD Random Write (8 KiB), 1 thread                                       | 30 μs    | 3 GiB/s    |
| SSD Sequential Write (8 KiB), 1 thread                                   | 30 μs    | 3 GiB/s    |
| SSD Sequential Durable Write (64 KiB), 1 thread†                         | 50 μs    | 3 GiB/s    |
| Network Bandwidth                                                        | 100 μs   | 3 GiB/s    |
| Network Same-Zone                                                        | 100 μs   | NW Limit   |
| Network Cross-Zone                                                       | 500 μs   | NW Limit   |
| Network Cross-Region, Same-Continent                                     | 25-60 ms | NW Limit\* |
| Blob Storage, Zonal Read (128 KiB)                                       | 50 ms    | NW Limit   |
| Blob Storage, Regional Read (128 KiB)                                    | 100 ms   | NW Limit   |
| Network Cross-Region, Cross-Atlantic                                     | 100 ms   | NW Limit\* |
| Blob Storage, Zonal Write (128 KiB)                                      | 200 ms   | NW Limit   |
| Blob Storage, Regional Write (128 KiB)                                   | 200 ms   | NW Limit   |
| Network Cross-Region, US ↔ Singapore                                     | 200 ms   | NW Limit\* |
| CPU Hash, crypto-safe (SHA-256, 1 KiB), 1 thread                         | 500 ns   | 2 GiB/s    |
| CPU Decode Parquet (dictionary + zstd, 1 MiB page), 1 thread             | 500 μs   | 2 GiB/s    |
| CPU Serialize (JSON, Protobuf, 200 B), 1 thread                          | 100 ns   | 1 GiB/s    |
| Memory Random Read (64 B), other socket, 1 thread‡                       | 250 ns   | 1 GiB/s    |
| CPU Decompress (lz4, zstd -1, 64 KiB), 1 thread                          | 50 μs    | 1 GiB/s    |
| Network Disk, Zonal Sequential Read (8 KiB), 1 thread                    | 500 μs   | 1 GiB/s    |
| Network Disk, Regional Sequential Read (8 KiB), 1 thread                 | 500 μs   | 1 GiB/s    |
| Network Disk, Zonal Sequential Write (8 KiB), 1 thread                   | 500 μs   | 1 GiB/s    |
| Network Disk, Zonal Sequential Durable Write (64 KiB), 1 thread†         | 1 ms     | 1 GiB/s    |
| Network Disk, Regional Sequential Write (8 KiB), 1 thread                | 1 ms     | 1 GiB/s    |
| Network Disk, Regional Sequential Durable Write (64 KiB), 1 thread†      | 2 ms     | 1 GiB/s    |
| CPU Hash, crypto-safe (SHA-256, 64 B), 1 thread                          | 100 ns   | 500 MiB/s  |
| CPU Deserialize (JSON, Protobuf, 200 B), 1 thread                        | 500 ns   | 500 MiB/s  |
| CPU Compress (lz4, zstd -1, 64 KiB), 1 thread                            | 100 μs   | 500 MiB/s  |
| Network Disk, Zonal Random Read (8 KiB), 2 threads                       | 500 μs   | 500 MiB/s  |
| Network Disk, Regional Random Read (8 KiB), 2 threads                    | 500 μs   | 500 MiB/s  |
| Network Disk, Zonal Random Write (8 KiB), 1 thread                       | 500 μs   | 500 MiB/s  |
| Network Disk, Regional Random Write (8 KiB), 1 thread                    | 1 ms     | 500 MiB/s  |
| CPU Encode Parquet (dictionary + zstd, 1 MiB page), 1 thread             | 3 ms     | 300 MiB/s  |
| CPU Sort (64-bit integers, 1 GiB), 1 thread                              | 3 s      | 300 MiB/s  |
| Network Disk, Zonal, HDD Sequential Read (8 KiB), 1 thread               | 500 μs   | 250 MiB/s  |
| Network Disk, Regional, HDD Sequential Read (8 KiB), 1 thread            | 500 μs   | 250 MiB/s  |
| Network Disk, Zonal, HDD Sequential Write (8 KiB), 1 thread              | 500 μs   | 250 MiB/s  |
| HDD Sequential Read (8 KiB), 1 thread                                    | 100 μs   | 200 MiB/s  |
| HDD Sequential Write (8 KiB), 1 thread                                   | 100 μs   | 200 MiB/s  |
| Network Disk, Zonal, HDD Sequential Durable Write (64 KiB), 1 thread†    | 500 μs   | 200 MiB/s  |
| Network Disk, Regional, HDD Sequential Durable Write (64 KiB), 1 thread† | 1 ms     | 200 MiB/s  |
| Network Disk, Regional, HDD Sequential Write (8 KiB), 1 thread           | 1 ms     | 200 MiB/s  |
| Blob Storage, Regional Read (1 GiB), 1 connection                        | 10 s     | 100 MiB/s  |
| Network Disk, Zonal, HDD Random Write (8 KiB), 1 thread                  | 500 μs   | 20 MiB/s   |
| Network Disk, Regional, HDD Random Write (8 KiB), 1 thread               | 1 ms     | 20 MiB/s   |
| Network Disk, Zonal, HDD Random Read (8 KiB), 2 threads                  | 20 ms    | 10 MiB/s   |
| Network Disk, Regional, HDD Random Read (8 KiB), 2 threads               | 20 ms    | 10 MiB/s   |
| HDD Sequential Durable Write (8 KiB), 1 thread†                          | 1 ms     | 5 MiB/s    |
| HDD Random Write (8 KiB), 1 thread                                       | 5 ms     | 2 MiB/s    |
| HDD Random Read (8 KiB), 2 threads                                       | 10 ms    | 2 MiB/s    |
| CPU Atomic Increment                                                     | 5 ns     |            |
| CPU Mutex Lock + Unlock                                                  | 10 ns    |            |
| CPU Atomic Increment, contended                                          | 30 ns    |            |
| CPU System Call                                                          | 100 ns   |            |
| CPU Mutex Lock + Unlock, contended                                       | 200 ns   |            |
| CPU Context Switch                                                       | 1 μs     |            |
| CPU Cross-core Wake-up                                                   | 2 μs     |            |
| CPU TCP Connect (+1 RTT)                                                 | 10 μs    |            |
| Proxy Request (HAProxy, Nginx) (+1 RTT)                                  | 20 μs    |            |
| Cache Get (Memcached, Redis) (+1 RTT)                                    | 20 μs    |            |
| CPU Thread Spawn                                                         | 30 μs    |            |
| Database Query, primary key (MySQL, Postgres) (+1 RTT)                   | 30 μs    |            |
| CPU TLS 1.3 Handshake (+1 RTT)                                           | 300 μs   |            |
| CPU Process Spawn                                                        | 300 μs   |            |
| Blob Storage, Zonal Conditional Write (128 KiB)                          | 100 ms   |            |
| Blob Storage, Regional Conditional Write (128 KiB)                       | 100 ms   |            |
| Blob Storage, Zonal List (1000 keys)                                     | 100 ms   |            |
| Blob Storage, Regional List (1000 keys)                                  | 150 ms   |            |

† pwrite + fdatasync into a preallocated file; one thread keeps 32 in flight
with io_uring, and the latency is one at a time. Cloud SSDs and most network
disks have no write cache to flush (pd-standard and Azure Premium SSD flush in
30-80 μs), so a durable write costs a write. HDDs flush theirs (~1 ms), which
32 in flight don't speed up.

‡ Only the largest shapes span two sockets. Reading the other socket's memory
costs ~2x the latency, and all cores share the ~60 GiB/s link (Graviton4).
Keep memory on the socket that reads it: Linux allocates on the node of the
thread that first touches a page. Intel's largest VMs also split one socket
into a NUMA node per die; another die costs 1.2x the latency and 0.7x one
thread's bandwidth, but all cores lose only ~10%.

\* One TCP connection moves ~3 MiB per round trip, so 3 MiB / RTT: 30 MiB/s
at 100 ms. Add connections to go faster.

Throughput is with enough requests in flight to max out a 48 vCPU VM and its
disks: local NVMe SSDs striped together, EBS gp3 at 16k IOPS / 1000 MiB/s and
Hyperdisk Balanced at 80k IOPS / 1600 MiB/s, 2 TB pd-standard for network HDDs.
Disk rows use io_uring with 32-128 requests in flight per thread; one core tops
out at 500-850k small random reads per second, hence two threads. Their
latency is the p50 of one 8 KiB request at a time, and one thread with one
request in flight gets 8 KiB / latency: random SSD reads at 100 μs are 80
MiB/s. One thread reading a
file 8 KiB at a time gets 1-5 GiB/s, from kernel readahead. Regional disks copy
every write to a second zone, which adds ~1 ms to every write.

CPU and memory rows marked 1 thread scale with cores until memory bandwidth
runs out (on x86 a vCPU is half a core). x86 memory writes need non-temporal
stores to get there; on one thread, a plain memset first reads every line it
overwrites and gets half. Random memory throughput is with 4 KiB pages, where
the TLB misses; the 100 ns latency is with 2 MiB huge pages (~150 ns with 4
KiB). The +1 RTT rows are measured over loopback, so add the network's round
trip; a proxy also adds one to its backend.

Memory latency is one access at a time, each waiting for the last; the all-cores
rows are with every core doing the same. Sequential reads take 5 ns because the
prefetchers got there first. A write only waits if something makes it: a plain
store retires into the store buffer in ~1 ns, so the write latency is an atomic
add, which has to own the line first.

CPU rows with a size are one call on the smallest batch that gets ~90% of the
throughput; smaller batches pay per-call setup, larger ones just take longer.
Sort and blob rows are the whole transfer, and Network Bandwidth is same-zone.

One TCP flow gets the VM's bandwidth on GCP but 5 Gbit/s (600 MiB/s) on AWS
outside a placement group, and 3 MiB / RTT across regions (\*). Blob Storage
latency is p99 from the same region; Zonal is S3 Express One Zone and GCS Rapid.
Conditional writes are compare-and-swap on one object, one at a time; regional
GCS also allows only ~1 write per second to the same object. Region-to-region
latencies: [cloudping](https://www.cloudping.co/).

## Cost Numbers

Approximate numbers that should be consistent between Cloud providers.

| What                          | Amount | \$ / Month | 1-year \$ / Month | Spot \$ / Month |
| ----------------------------- | ------ | ---------- | ----------------- | --------------- |
| CPU                           | 1 vCPU | \$30       | \$20              | \$10            |
| GPU (B200)                    | 1      | \$10000    | \$8000            | \$5000          |
| Memory                        | 1 GB   | \$2.5      | \$2               | \$1             |
| Storage                       |        |            |                   |                 |
| ├ Warehouse Storage           | 1 GB   | \$0.02     |                   |                 |
| ├ Blob Storage, Regional      | 1 GB   | \$0.02     |                   |                 |
| ├ Blob Storage, Zonal         | 1 GB   | \$0.1      |                   |                 |
| ├ Network Disk, Zonal, HDD    | 1 GB   | \$0.04     |                   |                 |
| ├ SSD                         | 1 GB   | \$0.15     | \$0.1             | \$0.04          |
| ├ SSD, storage-optimized      | 1 GB   | \$0.05     | \$0.05            |                 |
| ├ HDD                         | 1 GB   | \$0.01     |                   | \$0.001         |
| ├ Network Disk, Regional, HDD | 1 GB   | \$0.08     |                   |                 |
| ├ Network Disk, Zonal         | 1 GB   | \$0.1      |                   |                 |
| ├ Network Disk, Regional      | 1 GB   | \$0.2      |                   |                 |
| Network                       |        |            |                   |                 |
| ├ Same-Zone                   | 1 GB   | \$0        |                   |                 |
| ├ Blob Storage, Same-Region   | 1 GB   | \$0        |                   |                 |
| ├ Ingress                     | 1 GB   | \$0        |                   |                 |
| ├ L4 Load Balancer            | 1 GB   | \$0.005    |                   |                 |
| ├ Cross-Zone                  | 1 GB   | \$0.01     |                   |                 |
| ├ Cross-Region                | 1 GB   | \$0.02     |                   |                 |
| ├ Internet Egress ††          | 1 GB   | \$0.1      |                   |                 |
| CDN Egress                    | 1 GB   | \$0.08     |                   |                 |
| CDN Fill ¶                    | 1 GB   | \$0.01     |                   |                 |
| Warehouse Query               | 1 GB   | \$0.005    |                   |                 |
| Logs/Traces ♣                 | 1 GB   | \$0.5      |                   |                 |
| Metrics (series)              | 1000   | \$20       |                   |                 |
| KMS Keys                      | 1      | \$1        |                   |                 |

†† This refers to network leaving your cloud provider, e.g. sending data to S3
from GCP or egress network for sending HTML from AWS to a client.

¶ CDNs also charge ~\$1 per 1M requests, cache hits included.

♣ [Datadog](https://www.datadoghq.com/pricing/?product=log-management#products)
charges \$0.1/GB ingested plus ~\$1.5 per 1M indexed events for 7d retention.

Furthermore, for blob storage (S3/GCS/R2/...), you're charged per read/write
operation (fewer, large files is cheaper):

 |                | Regional, 1M | Zonal, 1M |
 |----------------|--------------|-----------|
 | Reads          | \$0.4        | \$0.1     |
 | Writes, Lists  | \$5          | \$1       |
 | KMS Encryption | \$3          | \$3       |

Zonal buckets also charge \$0.003 per GB written and \$0.0006 per GB read.

Instance shapes are the same few ratios on every cloud, here per vCPU at AWS's
prices (one B200 per 25 vCPU): on-demand, a 1-year Compute Savings Plan with no
upfront (~25% off), and spot (~70% off; ~45% for Memory, High and B200s). Most
cost a vCPU plus its memory and SSD at the unit prices above; the largest memory
shapes and B200s cost more:

| Shape              | vCPU | Memory (GiB) | VRAM (GB) | SSD (GB) | Network (GB/s) | \$ / mo / vCPU | 1-year | Spot  | Max (vCPU) |
| ------------------ | ---- | ------------ | --------- | -------- | -------------- | -------------- | ------ | ----- | ---------- |
| CPU                | 1    | 2            |           |          | 0.07           | \$35           | \$25   | \$10  | 300        |
| General            | 1    | 4            |           |          | 0.07           | \$40           | \$30   | \$15  | 300        |
| General, SSD       | 1    | 4            |           | 60       | 0.07           | \$50           | \$35   | \$15  | 300        |
| Memory             | 1    | 8            |           |          | 0.07           | \$50           | \$35   | \$15  | 300        |
| Memory, SSD        | 1    | 8            |           | 60       | 0.07           | \$60           | \$45   | \$20  | 300        |
| Memory, High       | 1    | 16           |           |          | 0.06           | \$70           | \$50   | \$40  | 200        |
| Memory, High, SSD  | 1    | 16           |           | 30       | 0.1            | \$75           | \$45   | \$20  | 128        |
| Memory, Ultra, SSD | 1    | 32           |           | 30       | 0.1            | \$150          | \$110  | \$50  | 128        |
| SSD                | 1    | 8            |           | 250      | 0.07           | \$65           | \$50   | \$15  | 200        |
| SSD, High          | 1    | 8            |           | 600      | 0.15           | \$95           | \$70   | \$30  | 200        |
| GPU (B200)         | 1    | 14           | 7         | 100      | 2              | \$450          | \$350  | \$250 | 200        |

Reading 1 TiB costs the VM time it takes at the row's throughput (48 vCPU + 192
GB is \$1,900/month, \$0.0007/s; the 1-thread memory row pays for one vCPU; the
GPU row is a B200 at 7 TiB/s), plus the transfer and request fees above:

| What                                                | \$ / TiB read |
| --------------------------------------------------- | ------------- |
| GPU Memory Sequential Read (B200)                   | \$0.0005      |
| Memory Sequential Read, 1 thread                    | \$0.0004      |
| Memory Sequential Read, all cores                   | \$0.004       |
| Memory Random Read (64 B), all cores                | \$0.008       |
| SSD Sequential Read (8 KiB)                         | \$0.2         |
| SSD Random Read (8 KiB)                             | \$0.2         |
| Network Same-Zone                                   | \$0.3         |
| Blob Storage, Regional Read (8 MiB)                 | \$0.3         |
| Network Disk, Zonal Sequential Read (8 KiB)         | \$0.8         |
| Network Disk, Regional Sequential Read (8 KiB)      | \$0.8         |
| Blob Storage, Zonal Read (8 MiB)§                   | \$0.9         |
| Network Disk, Zonal Random Read (8 KiB)             | \$2           |
| Network Disk, Regional Random Read (8 KiB)          | \$2           |
| Blob Storage, Zonal Read (128 KiB)§                 | \$2           |
| Network Disk, Zonal, HDD Sequential Read (8 KiB)    | \$3           |
| Network Disk, Regional, HDD Sequential Read (8 KiB) | \$3           |
| Blob Storage, Regional Read (128 KiB)               | \$4           |
| HDD Sequential Read (8 KiB)                         | \$4           |
| Network Cross-Zone                                  | \$10          |
| Network Cross-Region                                | \$20          |
| Network Disk, Zonal, HDD Random Read (8 KiB)        | \$80          |
| Network Disk, Regional, HDD Random Read (8 KiB)     | \$80          |
| Network Internet Egress                             | \$100         |
| HDD Random Read (8 KiB)                             | \$400         |

§ Zonal GETs cost 4x less, but every byte read costs \$0.6/TiB, so zonal is
cheaper below ~500 KiB per request and regional above.

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
| HTML                   | 4-8x              |
| English                | 2-4x              |
| Source Code            | 3-10x             |
| Executables            | 2-5x              |
| RPC (JSON)             | 5-13x             |
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
at ~15 GiB/s into a whole column, ~50 GiB/s a block at a time into cache;
sorted or low-cardinality columns need RLE or a dictionary first.
FSST decodes at ~2 GiB/s and keeps every string decodable on its own, where zstd
over a whole column gets 3-10x. Dictionary and RLE columns also pay for the
dictionary itself (each distinct value once); the smaller of the two rules
predicts Parquet's dictionary + RLE size of whole tables within 10%.

Search indexes, as a fraction of the text they index, with Tantivy's posting
lists (n-grams per source file for code, per ~16 KiB otherwise). Bloom filters
are per 1 MiB block; a query still decompresses and scans every block that
matches:

| What                                    | Prose | Code  | Logs  | Relational Columns |
| --------------------------------------- | ----- | ----- | ----- | ------------------ |
| Text, zstd -3 (1 MiB blocks)            | 0.35  | 0.2   | 0.08  | 0.3                |
| Inverted index                          | 0.25  | 0.1   | 0.15  | 0.3                |
| Inverted index + positions              | 0.4   | 0.25  | 0.3   | 0.4                |
| Trigram index (regex)                   | 0.1   | 0.1   | 0.05  | 0.1                |
| Fourgram index (regex)                  | 0.3   | 0.25  | 0.1   | 0.25               |
| Sparse n-gram index (regex)             | 0.6   | 0.5   | 0.15  | 0.45               |
| Trigram bloom filter, 1% FP (1 MiB)     | 0.015 | 0.02  | 0.005 | 0.015              |

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

