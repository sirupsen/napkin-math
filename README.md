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

Below are numbers rounded for memorization, not faux precision.
The CPU, memory, system call, context switch, hashing, sorting and compression
rows were re-measured on October 23, 2026 on current-generation 48 vCPU VMs: a
GCP `c4-standard-48-lssd` (Intel Xeon 6985P-C, Granite Rapids), an AWS
`m8id.12xlarge` (Intel Xeon 6975P-C, Granite Rapids) and an AWS `m8gd.12xlarge`
(Graviton4, ARM Neoverse-V2), all with ~180-192 GB RAM. The same-zone and
cross-zone network rows were measured between GCP `c4-standard-48` VMs and
between AWS `m8id`/`m8gd` VMs with `netperf` and `iperf3`. The inter-region
rows were measured from GCP C4 VMs to `n2-standard-8` VMs in other regions;
round-trip time and single-flow TCP throughput there are set by distance and
window size, not the VM. The SSD rows were measured with `fio` on the local
NVMe of the GCP C4 and AWS `m8id`/`m8gd` VMs. The blob storage rows come from
the March 2026 multi-cloud runs described below.

**Note 1:** Some throughput and latency numbers don't line up, this is
intentional for ease of calculations.

**Note 2:** Take the numbers with a grain of salt. E.g. for I/O, [`fio`][fio] is
the state-of-the-art. I am continuously updating these numbers as I learn more
to improve accuracy and as hardware improves.

**Note 3:** The 1 MiB / 1 GiB columns are size ÷ throughput only. For
request-based rows (blob storage, cross-region) add the latency: a single 1 MiB
blob GET takes ~150 ms, and a cold 1 MiB cross-region transfer takes several
round trips.

| Operation                           | Latency     | Throughput | 1 MiB  | 1 GiB  |
| ----------------------------------- | -------     | ---------- | ------ | ------ |
| Sequential Memory R/W (64 bytes)    | 3 ns        |            |        |        |
| ├ Single Thread                     |             | 20 GiB/s   | 50 μs  | 50 ms  |
| ├ Threaded                          |             | 200 GiB/s  | 5 μs   | 5 ms   |
| Network Same-Zone ¶                 | 100 μs      | 3 GiB/s    | 300 μs | 300 ms |
| ├ Inside VPC                        | 100 μs      | 3 GiB/s    | 300 μs | 300 ms |
| ├ Outside VPC                       |             | 1 GiB/s    | 1 ms   | 1s     |
| Hashing, not crypto-safe (64 bytes) | 10 ns       | 5 GiB/s    | 200 μs | 200 ms |
| Random Memory R/W (64 bytes) §      | 100 ns      | 3 GiB/s    | 300 μs | 300 ms |
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
| Decompression `[11]` ◊              | N/A         | 1 GiB/s    | 1 ms   | 1s     |
| Compression, fast `[11]` ◊          | N/A         | 500 MiB/s  | 2 ms   | 2s     |
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
| Blob Storage GET, If-None-Match 304 ※ | 30 ms     |            |        |        |
| Blob Storage GET, 1 conn (128KiB)   | 80 ms       | 100 MiB/s  | 10 ms  | 10s    |
| Blob Storage GET, n conn (offsets)  | 80 ms       | NW limit   |        |        |
| Blob Storage LIST (1000 keys)       | 100 ms      |            |        |        |
| Blob Storage PUT, 1 conn (128KiB)   | 200 ms      | 100 MiB/s  | 10 ms  | 10s    |
| Blob Storage PUT, n conn (multipart)| 200 ms      | NW limit   |        |        |
| Network between regions, 1 flow ♦   | [Varies][i] | 25 MiB/s   | 40 ms  | 40s    |
| Network NA Central <-> East         | 25 ms       | 100 MiB/s  | 10 ms  | 10s    |
| Network NA Central <-> West         | 40 ms       | 50 MiB/s   | 20 ms  | 20s    |
| Network NA East <-> West            | 60 ms       | 50 MiB/s   | 20 ms  | 20s    |
| Network EU West <-> NA East         | 80 ms       | 25 MiB/s   | 40 ms  | 40s    |
| Network EU West <-> NA Central      | 100 ms      | 25 MiB/s   | 40 ms  | 40s    |
| Network NA West <-> Singapore       | 180 ms      | 25 MiB/s   | 40 ms  | 40s    |
| Network EU West <-> Singapore       | 160 ms      | 25 MiB/s   | 40 ms  | 40s    |

[i]: https://www.cloudping.co/ 

**†:** Slow (~100 MiB/s): Go `encoding/json`, Python/Ruby stdlib JSON,
`nlohmann::json`, reflection-heavy and ORM serializers. Fast (~1 GiB/s):
`serde_json`, simdjson, protobuf/FlatBuffers in C++/Java/Rust. Decoding into
owned heap objects is typically ~300 MiB/s even in fast languages (this repo
measured `prost` decode at ~300 MiB/s and `serde_json` decode at ~350 MiB/s);
zero-copy formats reach 1 GiB/s and more. Serialization performance depends
enormously on the data and the implementation.

**§:** 100 ns is one dependent DRAM miss, e.g. a pointer chase, B-tree hop or
hash-chain walk (measured with huge pages at 123 ns on GCP C4, 136 ns on AWS
`m8id` and 115 ns on Graviton4; with 4 KiB pages 160-180 ns on the Intel VMs
and 138 ns on Graviton4). A single thread doing
_independent_ random accesses overlaps ~5 misses, so a shuffled gather streams
at ~3 GiB/s (~20 ns amortized per cache line); the throughput and 1 MiB / 1 GiB
columns assume that independent case. For reference, the same pointer chase
measured ~1.5 ns in L1, ~5 ns in L2 and ~30 ns in L3.

**¶:** Per VM, many parallel flows, typical 16-48 vCPU VM. Measured: GCP
`c4-standard-48` 32 Gbit/s (~3.8 GiB/s) same-zone and cross-zone, AWS
`m8id`/`m8gd.12xlarge` 22.3 Gbit/s (~2.6 GiB/s). AWS caps a single TCP flow at
5 Gbit/s (~600 MiB/s) outside a cluster placement group. Same-zone TCP round
trips measured 74 μs on GCP and 81 μs on AWS; cross-zone 766 μs on GCP and 456
μs on AWS. Outside VPC = to public IPs: GCP caps it at 7 Gbit/s per VM (25 with
Tier_1), AWS at 50% of the NIC for ≥32 vCPU. 10+ GiB/s needs a 96+ vCPU or
network-optimized VM (see the premium row).

**♦:** One TCP flow with default Linux buffers (`tcp_wmem` max 4 MiB, CUBIC)
moves roughly one window per round trip. Measured from GCP `us-central1`: 80
MiB/s to `us-east4` (23 ms), 68 MiB/s to `us-west1` (38 ms), 21 MiB/s to
`europe-west1` (110 ms) and 10 MiB/s to `asia-southeast1` (200 ms). Parallel
flows scale nearly linearly until the VM's egress cap (8 flows: 0.5-0.9 GiB/s).
Round-trip times measured between GCP regions on October 23, 2026 were within
~10% of the rows above, except NA West <-> Singapore: 149 ms on GCP versus
~170-180 ms between AWS regions ([cloudping][i]).

**◊:** Fast codecs (lz4, snappy, zstd -1) on the Silesia corpus, single thread
(measured on AWS `m8id` and Graviton4 `m8gd` with
`src/bin/compression_corpus.rs`, within ~10% of each other): lz4 ~520 MiB/s,
snappy ~555 MiB/s, zstd -1 ~450 MiB/s to compress; zstd and snappy decompress
at ~1 GiB/s, the Rust lz4 at ~1.5 GiB/s and the C lz4 library at ~4 GiB/s per
`[11]`. Slower, denser
settings are much slower to compress: zstd -3 ~270 MiB/s, gzip -6 ~30 MiB/s,
zstd -19 ~3 MiB/s, while decompression stays ~1 GiB/s for zstd and ~300 MiB/s
for gzip.

**※:** Blob latencies are roughly p99 for same-region S3 Standard and GCS
(XML API) on warm keep-alive connections, measured with `src/bin/s3_latency.rs`
in March 2026. p50 is ~1.5-3x lower (304 ~10 ms, 128 KiB GET ~60 ms, 128 KiB PUT
~65-125 ms). S3 Express One Zone and Azure Blob answer 304s and conditional
PUTs in ~5 ms, and GCS gRPC serves small GETs in ~15 ms. Multipart PUT latency
(≥3 round trips) is a heuristic.

Sorting is `n log n`: `sort_unstable` on random `u64` measured 560-670 MiB/s
for 1 MiB but 330-360 MiB/s for 1 GiB (~3 s) on all three VMs. A null system
call (`getppid`) measured 77 ns on GCP C4, 83 ns on AWS `m8id` and 134 ns on
Graviton4; `stat()` 300-540 ns. The context switch rows come from a futex
ping-pong between two threads (`context_switch` bench): 0.75-1 μs per switch on
one core, and 1.9-2.7 μs to wake a thread on another core. Neither includes the
cost of refilling caches afterwards. Sequential memory reads on Graviton4 are
notably faster than the Intel VMs: 23-29 GiB/s on one thread and ~350 GiB/s
across all 48 cores, versus 16-22 GiB/s and 170-190 GiB/s.

Local NVMe varies a lot by provider: GCP throttles each local disk to ~650
MiB/s (~5 GiB/s for 8 disks), while one AWS `m8id` instance-store disk read
6 GiB/s sequentially and 760k random 8 KiB IOPS (~6 GiB/s) at QD32, and each of
the `m8gd`'s three disks ~2 GiB/s. Random 8 KiB reads at QD1 took 70-95 μs
everywhere. An 8 KiB append + `fdatasync` on ext4 took 255 μs on `m8id` (88 μs
on `m8gd`); on the raw device without a file system it was 17-42 μs, and
buffered 8 KiB writes without fsync ran at 0.7-3 GiB/s.

For the active Criterion suite, run `./run --bench napkin_math` to get the
right optimization levels and Linux tuning. You won't get the right numbers
when you're compiling in debug mode. The wrapper already uses `sudo`
internally. On locked-down cloud images, run
`sudo sysctl -w kernel.perf_event_paranoid=-1` once before invoking it. You
can help this project by adding new suites and filling out the blanks.

**Note:** The active benchmark path today is Criterion.rs in `benches/`.
`src/main.rs` is still the older ad hoc harness and remains the source of truth
for the benches that have not been fully migrated and revalidated yet. The
current Criterion suite now includes `blob_storage`, `memory_read`,
`memory_random`, `memory_latency`, `hash`, `syscall`, `context_switch`, `sort`,
`serialization`, `compression`, and `compressed_memory_read`.
`memory_latency` is a dependent pointer chase over 16 KiB to 1 GiB, with and
without huge pages; `memory_random` is the independent-access throughput
counterpart. `hash` reports both 64-byte calls and 1 MiB bulk throughput
(`hash_1mib`, including xxh3 and BLAKE3), and `sort` covers 1 MiB, 64 MiB and
1 GiB inputs. `context_switch` is Linux-only.
The SSD rows were re-checked with `fio` rather than the older harness. Until
October 2026, `disk_read_sequential` (and its io_uring variant) in
`src/main.rs` dropped the page cache once and then looped over the same 1 GiB
file, so after the first pass they measured page cache, not the SSD. That's
where the old `8 GiB/s` came from. They now read an 8 GiB file once and stop,
so `NAPKIN_BENCH_FILE` needs ~8 GiB free. On an Azure `D48ds_v5` temp SSD
they measure 1.6 GiB/s with 8 KiB reads and 2.0 GiB/s with io_uring. `src/bin/compression_corpus.rs` reports ratio and single-threaded
speed for real files (e.g. the Silesia corpus) for lz4, snappy, zstd, gzip and
brotli.
The `compressed_memory_read` Criterion bench is a BitPacker integer-unpack
microbenchmark; it should not be used to rewrite the generic `[11]`
compression/decompression rows above. The new `serialization` and
`compression` Criterion groups are workload-specific and are not yet wired into
the generic README rows above.
The new `blob_storage` Criterion group is opt-in and credentialed: set
`NAPKIN_GCS_BUCKET` and/or `NAPKIN_S3_BUCKET`. Both the GCS and S3 paths now
use the AWS S3 SDK. The GCS side talks to the GCS XML interoperability endpoint
and requires `NAPKIN_GCS_ACCESS_KEY` plus `NAPKIN_GCS_SECRET_KEY`. The S3 side
uses the local AWS profile in `NAPKIN_S3_PROFILE` (default `tpuf-test`) plus
`NAPKIN_S3_REGION` (default `us-west-2`).
The concurrent `get_offsets` and `put_multipart` paths explicitly fan out onto
a multi-thread Tokio runtime, so they can get close to the host NIC limit when
the range count / object count is high enough. On March 9, 2026, same-region
`1 GiB` single-stream GETs landed at about `95 MiB/s` on S3 (`m6id.12xlarge`
in `us-west-2a`) and about `190-200 MiB/s` on GCS XML
(`c4-standard-48-lssd` in `us-central1-c`). On the same machines, explicit
concurrent range GETs reached about `2.0 GiB/s` on S3 and about `4.9 GiB/s` on
GCS, while multipart PUTs reached about `1.8-1.9 GiB/s` on S3 and about
`3.3 GiB/s` on GCS. Older AWS S3 guidance suggested budgeting about
`85-90 MB/s` per concurrent request when saturating a `10 Gbps` instance, which
matched the measured S3 single-stream result closely. The old `500 MiB/s`
single-stream blob GET row above did not reproduce on either provider, so it
has been revised down to a conservative generic `100 MiB/s`.
The blob latency cells come from the small-object latency probe in
`src/bin/s3_latency.rs` (March 10-20, 2026); see the `※` footnote for the
percentile and backends. Run
`./script/blob-latency <s3|s3x|gcs|r2|tigris|azure>` to sweep
`get`, `put`, `if_none_match`, and `put_if_match` across the default
`8 KiB .. 8 MiB` size ladder, except `if_none_match`, which now defaults to a
single `128 KiB` row because its latency was effectively flat across the small
size sweep. Override `NAPKIN_BLOB_LATENCY_OPS`,
`NAPKIN_BLOB_LATENCY_SIZES`, and
`NAPKIN_BLOB_LATENCY_IF_NONE_MATCH_SIZES` when you want a smaller or
provider-specific run. The latency probe now defaults to a per-row wall-clock
budget of `300` seconds, collecting as many samples as fit in that time.
Tune that with `NAPKIN_BLOB_LATENCY_ROW_SECONDS`; optionally cap it with
`NAPKIN_BLOB_LATENCY_SAMPLE_CAP` (or the older `NAPKIN_BLOB_LATENCY_SAMPLES`
alias) when you want shorter count-limited experiments instead.
The `list` op now seeds a larger namespace by default (`1M` keys) and
measures one randomized `1000`-key page per sample via `start_after`, rather
than repeatedly scanning the same small fixed prefix. Tune that shape with
`NAPKIN_BLOB_LATENCY_LIST_NAMESPACE_KEYS`,
`NAPKIN_BLOB_LATENCY_LIST_KEYS`, and
`NAPKIN_BLOB_LATENCY_LIST_SEED_CONCURRENCY`. If you also want the old
whole-namespace walk, add `list_full_scan` to `NAPKIN_BLOB_LATENCY_OPS`.
For aligned range-read latency, `./script/blob-random-range-latency` measures
the `128 KiB`-aligned shape and `./script/blob-random-range-latency-8m`
measures the `8 MiB`-aligned shape over the same `128 x 1 GiB` object pool.
`./script/blob-random-range-latency-8m-multipart` seeds those `1 GiB` source
objects via multipart upload with `8 MiB` parts, then measures `8 MiB`-aligned
reads against those multipart boundaries.
`memory_read` now emits explicit `No SIMD` and `SIMD` variants in Criterion,
but the README intentionally collapses them to one single-thread row and one
threaded row for memorability.

I am aware of some inefficiencies in this suite. I intend to improve my skills
in this area, in order to ensure the numbers are the upper-bound of performance
you may be able to squeeze out in production. An October 2026 audit found
several cells that were more than 3x off (random memory latency, same-zone
network bandwidth, the sequential SSD read); those have been fixed, and the
rest should be within 2-3x, which shouldn't be a problem for most users.

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
