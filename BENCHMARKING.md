# Benchmarking

How the numbers in the [README](README.md) are measured, and how to refresh
them. Everything here runs on Linux; the Criterion suite also builds on macOS
(without the Linux-only groups and host tuning).

## Criterion suite

```bash
./run                                         # all groups, with host tuning
./run --bench napkin_math -- 'memory_latency|syscall'   # a subset (regex)
make test                                     # every benchmark once, as a smoke test
```

`./run` turns turbo off, sets the `performance` governor, drops the page cache,
disables ASLR, takes SMT siblings offline and sets transparent huge pages to
`always`, then runs `cargo bench` with `-C target-cpu=native`. Each step is
skipped when the host doesn't expose it (EC2, ARM and many VMs lack some of
them), and everything it changes is restored on exit. Run it as your normal
user: it calls `sudo` itself. `NAPKIN_PERF=1` wraps the run in `perf stat`; on
locked-down cloud images that needs `sudo sysctl -w
kernel.perf_event_paranoid=-1` first. You won't get the right numbers from a
debug build.

| Group | Measures | README row |
| ----- | -------- | ---------- |
| `memory_read` | Sequential reads of 1 GiB, 1 thread and all cores, scalar and SIMD | Memory Sequential Read |
| `memory_latency` | Dependent pointer chase over 16 KiB to 1 GiB, with and without huge pages | Memory Random Read, latency |
| `random_memory` | One pass of independent random 64 B reads/writes over 1 GiB | Memory Random Read, throughput |
| `hash`, `hash_1mib` | sha256, blake3, crc32, siphash, xxh3 on 64 B calls and 1 MiB buffers | CPU Hash rows |
| `syscall` | `getppid` (null syscall), `getpid`, `clock_gettime`, `getrusage`, `fstat` | CPU System Call |
| `context_switch` | futex ping-pong between two threads, same core and cross core (Linux) | CPU Context Switch |
| `sort` | `sort_unstable` on random `u64`, 1 MiB to 1 GiB | CPU Sort |
| `tcp` | 32 KiB echo over loopback | none (loopback only) |
| `serialization` | Protobuf (prost), JSON (serde_json, to structs and to a dynamic `Value`) and postcard on a 1 MiB batch of events | CPU Serialize, CPU Deserialize |
| `compression` | zstd -3 on the event corpus as JSON | none (see `compression_corpus`) |
| `bitpacking` | `BitPacker8x` integer unpack | none (not the `[11]` rows) |

In October 2026 on a c4-standard-8 (Xeon 8581C), serialization ran at
0.9-1.2 GiB/s for every format and deserialization into structs at 350-400
MiB/s (200 MiB/s into a dynamic JSON `Value`): allocating the objects, not the
format, sets the speed.

The README collapses `memory_read`'s scalar and SIMD variants into one
single-thread row and one threaded row for memorability. `random_memory` issues
independent loads, so the CPU overlaps misses; that is throughput, which is why
the latency cell comes from `memory_latency` instead.

## SSD, Network Disk and HDD rows

```bash
script/ssd-fio /mnt/nvme/fio.dat                                # a file on a mounted file system
NAPKIN_FIO_DESTROY_DEVICE=/dev/nvme1n1 script/ssd-fio /dev/nvme1n1   # an unused raw device
```

The README uses the max-throughput results (1 MiB QD32 for sequential, 8 KiB
QD32x4 for random) and QD1 latency for random reads. For an HDD, test the whole
disk with `NAPKIN_FIO_SIZE=1800G NAPKIN_FIO_PRECONDITION=0` so random reads seek
across all of it. The October 2026 rows came from local NVMe on GCP C4 and AWS
m8id/m8gd, EBS gp3 at 16k IOPS / 1000 MiB/s on an m8i.4xlarge, GCP Hyperdisk
Balanced at 80k IOPS / 1600 MiB/s on a c4-standard-16, and the instance-store
HDDs of an AWS d3.xlarge.

`script/ssd-fio` runs `fio` for sequential 1 MiB and buffered 8 KiB reads,
random 8 KiB reads at QD1 and QD32, sequential writes, buffered 8 KiB writes and
8 KiB writes with `fdatasync`. It preconditions the range first (reads of
never-written blocks can skip the flash), checks that the file system the file
actually lands on has 2x the test size free, and refuses to write to a mounted
or formatted device. A file target includes the file system's fsync cost
(the README fsync rows are ext4); a raw device does not.

These rows used to come from a Rust harness in `src/main.rs`, whose sequential
read benchmark reread the same 1 GiB file from page cache after the first pass.
That is where the README's old `8 GiB/s` sequential SSD read came from.

## Network rows

```bash
script/net-pair --server        # on host A
script/net-pair 10.0.0.12       # on host B, with A's private IP
```

Runs ICMP ping, `netperf` TCP_RR with 1 B and 32 KiB messages, and `iperf3`
with 1, 8 and 32 flows. Use TCP_RR, not ping, for round-trip times: ICMP is
often handled on a slower path on cloud networks. Single-flow results show
per-flow caps (5 Gbit/s on AWS outside a cluster placement group);
multi-flow results show the VM's limit.

## Compression rows

```bash
cargo run --release --bin compression_corpus -- silesia.tar dickens ...
NAPKIN_COMPRESSION_CODECS="lz4_flex,zstd -3" cargo run --release --bin compression_corpus -- file
```

`src/bin/compression_corpus.rs` reports the ratio and single-threaded
compress/decompress speed of lz4, snappy, zstd (-1, -3, -9, -19), gzip (-1, -6)
and brotli (-5, -11) for each file, as best of N runs over at least a second.
The CPU Compress/Decompress rows use the
[Silesia corpus](https://sun.aei.polsl.pl/~sdeor/index.php?page=silesia) (on the
c4-standard-8: lz4 2.1x at 442/1246 MiB/s, zstd -1 2.9x at 395/819 MiB/s). The
ratio table used Silesia's `dickens` (English), `samba` (source) and `mozilla`
(executables), a Wikipedia article (HTML), a tarball of the cargo registry
sources, `/usr/bin` and a page of GitHub API JSON.

## Blob storage rows (March 2026)

The blob storage benchmark code isn't in this repository yet; this describes
the setup behind the README's blob rows.

The `blob_storage` Criterion group was opt-in and credentialed: set
`NAPKIN_GCS_BUCKET` and/or `NAPKIN_S3_BUCKET`. Both the GCS and S3 paths used
the AWS S3 SDK. The GCS side talked to the GCS XML interoperability endpoint
and required `NAPKIN_GCS_ACCESS_KEY` plus `NAPKIN_GCS_SECRET_KEY`. The S3 side
used the local AWS profile in `NAPKIN_S3_PROFILE` (default `tpuf-test`) plus
`NAPKIN_S3_REGION` (default `us-west-2`).

The concurrent `get_offsets` and `put_multipart` paths fanned out onto a
multi-thread Tokio runtime, so they could get close to the host NIC limit when
the range count / object count was high enough. On March 9, 2026, same-region
`1 GiB` single-stream GETs landed at about `95 MiB/s` on S3 (`m6id.12xlarge`
in `us-west-2a`) and about `190-200 MiB/s` on GCS XML (`c4-standard-48-lssd`
in `us-central1-c`). On the same machines, concurrent range GETs reached about
`2.0 GiB/s` on S3 and about `4.9 GiB/s` on GCS, while multipart PUTs reached
about `1.8-1.9 GiB/s` on S3 and about `3.3 GiB/s` on GCS. Older AWS S3 guidance
suggested budgeting about `85-90 MB/s` per concurrent request when saturating a
`10 Gbps` instance, which matched the measured S3 single-stream result closely.
The old `500 MiB/s` single-stream blob GET row did not reproduce on either
provider, so it was revised down to a conservative generic `100 MiB/s`.

The blob latency cells came from a small-object latency probe
(`src/bin/s3_latency.rs`, March 10-20, 2026), with 128 requests in flight on
warm connections from a VM in the same region. The README cells are p99 across
S3 Standard, GCS and Azure Blob. For reference, p50 was: an 8 KiB range read 56 /
104 (XML) / 35 (gRPC) / 45 ms, an 8 KiB PUT 49 / 141 / 71 / 32 ms and a
1000-key LIST page 78 / 56 / 89 ms. LIST p99 was 137 / 156 / 174 ms (S3 / GCS
XML / Azure).

The Read and Write rows are p99 at the knee: the largest size
whose p99 is still about what 8 KiB costs. Over 1 GiB objects with 128 KiB- and 1
MiB-aligned ranges, S3 p99 stays at 80-90 ms up to 128 KiB and reaches 110-125
ms at 256 KiB; Azure at 75-115 ms up to 128 KiB and 115-125 ms at 256 KiB. GCS
gRPC stays near 100 ms up to 2 MiB, while GCS XML is noisy at 130-390 ms. Those
runs had 128 requests in flight, so a lone request may stay flat to a larger
size. For PUTs, p99 at 8 / 128 / 256 KiB was 100 / 140 / 186 ms on S3, 51 / 38 /
114 ms on Azure and 271 / 229 / 338 ms on GCS XML.

GCS Rapid read 8 KiB in 6-14 ms. Throughput peaked
at the hosts' NICs with 32-128 connections: GETs at 2.0 (S3), 4.3 (GCS) and 3.9
GiB/s (Azure), PUTs at 1.9, 3.1 and 2.2 GiB/s.

`./script/blob-latency <s3|s3x|gcs|r2|tigris|azure>` swept `get`, `put`, `if_none_match` and
`put_if_match` across an `8 KiB .. 8 MiB` size ladder, except `if_none_match`,
which defaulted to a single `128 KiB` row because its latency was effectively
flat across sizes. `NAPKIN_BLOB_LATENCY_OPS`, `NAPKIN_BLOB_LATENCY_SIZES` and
`NAPKIN_BLOB_LATENCY_IF_NONE_MATCH_SIZES` narrowed a run. Each row had a
wall-clock budget of `300` seconds (`NAPKIN_BLOB_LATENCY_ROW_SECONDS`),
optionally capped by `NAPKIN_BLOB_LATENCY_SAMPLE_CAP`.

The `list` op seeded a `1M`-key namespace and measured one randomized
`1000`-key page per sample via `start_after`
(`NAPKIN_BLOB_LATENCY_LIST_NAMESPACE_KEYS`, `NAPKIN_BLOB_LATENCY_LIST_KEYS`,
`NAPKIN_BLOB_LATENCY_LIST_SEED_CONCURRENCY`); `list_full_scan` walked the whole
namespace. For aligned range-read latency,
`./script/blob-random-range-latency` measured the `128 KiB`-aligned shape and
`./script/blob-random-range-latency-8m` the `8 MiB`-aligned shape over a
`128 x 1 GiB` object pool, and
`./script/blob-random-range-latency-8m-multipart` seeded those objects via
multipart upload with `8 MiB` parts to measure reads against part boundaries.

## Further reading

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
* [io_uring](https://lwn.net/Articles/776703/). Best summary, it links to many
  resources.
* [Integer Compression Comparisons](https://github.com/powturbo/TurboPFor-Integer-Compression)
