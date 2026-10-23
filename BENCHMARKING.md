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
| `memory_read` | Sequential reads of 1 GiB, 1 thread and all cores, scalar and SIMD | Sequential Memory R/W |
| `memory_latency` | Dependent pointer chase over 16 KiB to 1 GiB, with and without huge pages | Random Memory latency (`§`) |
| `random_memory` | One pass of independent random 64 B reads/writes over 1 GiB | Random Memory throughput |
| `hash`, `hash_1mib` | sha256, blake3, crc32, siphash, xxh3 on 64 B calls and 1 MiB buffers | Hashing rows |
| `syscall` | `getppid` (null syscall), `getpid`, `clock_gettime`, `getrusage`, `fstat` | System Call, `stat()` |
| `context_switch` | futex ping-pong between two threads, same core and cross core (Linux) | Context Switch rows |
| `sort` | `sort_unstable` on random `u64`, 1 MiB to 1 GiB | Sorting |
| `tcp` | 32 KiB echo over loopback | TCP Echo, loopback |
| `serialization`, `compression` | prost/serde_json and zstd -3 on a synthetic event corpus | none (workload-specific) |
| `bitpacking` | `BitPacker8x` integer unpack | none (not the `[11]` rows) |

The README collapses `memory_read`'s scalar and SIMD variants into one
single-thread row and one threaded row for memorability. `random_memory` issues
independent loads, so the CPU overlaps misses; that is throughput, which is why
the latency cell comes from `memory_latency` instead.

## SSD rows

```bash
script/ssd-fio /mnt/nvme/fio.dat                                # a file on a mounted file system
NAPKIN_FIO_DESTROY_DEVICE=/dev/nvme1n1 script/ssd-fio /dev/nvme1n1   # an unused raw device
```

`script/ssd-fio` runs `fio` for sequential 1 MiB and buffered 8 KiB reads,
random 8 KiB reads at QD1 and QD32, sequential writes, buffered 8 KiB writes and
8 KiB writes with `fdatasync`. It preconditions the range first (reads of
never-written blocks can skip the flash), checks that the file system the file
actually lands on has 2x the test size free, and refuses to write to a mounted
or formatted device. A file target includes the file system's fsync cost
(the README `+fsync` row is ext4); a raw device does not.

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
The README rows use the [Silesia corpus](https://sun.aei.polsl.pl/~sdeor/index.php?page=silesia);
the compression ratio table also used modern web pages, source trees,
`/usr/bin` and API JSON.

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
(`src/bin/s3_latency.rs`, March 10-20, 2026); see the README's `※` footnote for
the percentile and backends. `./script/blob-latency
<s3|s3x|gcs|r2|tigris|azure>` swept `get`, `put`, `if_none_match` and
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
