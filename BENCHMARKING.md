# Benchmarking

How the numbers in the [README](README.md) are measured, and how to refresh
them. Everything here runs on Linux; the Criterion suite also builds on macOS
(without the Linux-only groups and host tuning).

## Criterion suite

```bash
./run                                         # all groups
./run --bench napkin_math -- 'memory_latency|syscall'   # a subset (regex)
NAPKIN_TUNE=1 ./run                           # quieter host, for comparing two runs
make test                                     # every benchmark once, as a smoke test
```

`./run` builds first, then runs `cargo bench` with `-C target-cpu=native` added
to `RUSTFLAGS`. The README numbers come from that on stock cloud VMs, untuned.
`NAPKIN_TUNE=1` turns turbo off, sets the `performance` governor and disables
ASLR (with `sudo`, restored on exit), so two runs on one host vary less; it
understates production speed, so don't refresh README numbers with it. Most
cloud VMs expose none of these knobs. `NAPKIN_PERF=1` wraps the run in `perf
stat`; on locked-down cloud images that needs `sudo sysctl -w
kernel.perf_event_paranoid=-1` first. You won't get the right numbers from a
debug build.

| Group (file in `benches/benchmarks/`) | Measures | README row |
| ----- | -------- | ---------- |
| `memory_sequential_read` | Sequential reads of 1 GiB, 1 thread and all cores, scalar and SIMD | Memory Sequential Read |
| `memory_sequential_write` | memset and memcpy of 1 GiB on 1 thread, memset of 16 GiB on all cores | Memory Sequential Write |
| `memory_random` | One pass of independent random 64 B reads/writes over 1 GiB | Memory Random Read/Write, throughput |
| `memory_random_read_latency` (in `memory_random.rs`) | Dependent pointer chase over 1 GiB, with and without huge pages | Memory Random Read, latency |
| `cache_read`, `cache_read_latency` | The `memory_sequential_read` SIMD loop, and the pointer chase, over 16 KiB, 256 KiB and 8 MiB | L1/L2/L3 Cache Read |
| `cpu_encrypt` | ring AES-256-GCM and ChaCha20-Poly1305 seal and open on 16 KiB | CPU Encrypt / Decrypt |
| `cpu_hash`, `cpu_hash_1mib` | sha256, blake3, crc32, siphash, xxh3 on 64 B calls and 1 MiB buffers | CPU Hash rows |
| `cpu_serialize` | Protobuf (prost), JSON (serde_json, to structs and to a dynamic `Value`) and postcard on a 1 MiB batch of events | CPU Serialize, CPU Deserialize |
| `cpu_sort` | `sort_unstable` on random `u64`, 1 MiB to 1 GiB | CPU Sort |
| `cpu_atomic_mutex` | Atomic increment and `Mutex` lock + unlock, alone and with a second core on the same line | CPU Atomic Increment, CPU Mutex Lock + Unlock |
| `cpu_system_call` | `getppid` (null syscall), `getpid`, `clock_gettime`, `getrusage`, `fstat` | CPU System Call |
| `cpu_context_switch` | futex ping-pong between two threads, same core and cross core (Linux) | CPU Context Switch |
| `cpu_handshake` | TCP connect over loopback, and a full TLS 1.3 handshake (rustls, ECDSA P-256, X25519) in memory | CPU TCP Connect, CPU TLS 1.3 Handshake |
| `cpu_spawn` | `thread::spawn` + join, and spawning `true` and waiting for it | CPU Thread Spawn, CPU Process Spawn |

In October 2026 on a c4-standard-8 (Xeon 8581C), serialization ran at
0.9-1.2 GiB/s for every format and deserialization into structs at 350-400
MiB/s (200 MiB/s into a dynamic JSON `Value`): allocating the objects, not the
format, sets the speed.

On October 24, 2026 on a c4-standard-16-lssd (Xeon 6985P-C), these ran at:
L1/L2/L3 latency 1.4 / 4.9 / 31 ns and throughput 227 / 159 / 28 GiB/s; memset
and memcpy 11.3 / 11.5 GiB/s; random 64 B writes 1.9 GiB/s (2.7-5.1 on the
other hosts, hence 3 GiB/s); AES-256-GCM 10.3 GiB/s and ChaCha20-Poly1305 2.5
GiB/s; atomic increment 6 ns (72 ns contended); mutex 12 ns (546 ns
contended); thread spawn 52 us; process spawn 305 us; loopback TCP connect 7.8
us; TLS 1.3 handshake 259 us of CPU for both sides. TCP connect and the TLS
handshake each add one network round trip on top.

On October 26, 2026 on an m8id.12xlarge (Xeon 6975P-C, 48 vCPU), AES-256-GCM
decrypted at 9.7 GiB/s and encrypted at 9.5 GiB/s (ChaCha20-Poly1305 2.3 GiB/s
both ways), and a 16 GiB memset across 48 pinned threads ran at 145 GiB/s
(11.5 GiB/s on one thread).

The README collapses `memory_sequential_read`'s scalar and SIMD variants into one
single-thread row and one threaded row for memorability. `memory_random` issues
independent loads, so the CPU overlaps misses; that is throughput, which is why
the latency cell comes from `memory_random_read_latency` instead.

## SSD, Network Disk and HDD rows

```bash
cargo bench --bench storage -- /mnt/nvme/disk.dat                       # a file on a mounted file system
cargo bench --bench storage -- /dev/nvme1n1 --destroy /dev/nvme1n1      # an unused raw device
```

`benches/storage.rs` has no dependencies. It runs sequential 1 MiB reads and
writes at QD32, random 8 KiB reads and writes at QD1 and QD128, and buffered
8 KiB sequential reads and writes, all with `O_DIRECT` except the buffered ones.
Queue depth is N threads, each with one `pread` or `pwrite` in flight. It writes
the whole range first (reads of never-written blocks can skip the flash), drops
the page cache before buffered reads, checks that the file system the file
actually lands on has 2x the test size free, and refuses to write to a mounted
device or without `--destroy`. A file target includes the file system's cost; a
raw device does not.

The README uses the max-throughput results (1 MiB QD32 for sequential, 8 KiB
QD128 for random) and QD1 latency for random reads and writes. On October 24,
2026, Hyperdisk Balanced (80k IOPS / 1600 MiB/s) did 8 KiB random writes at
449 us QD1 and 633 MiB/s max, the same as its reads: network disks cap random
I/O at the provisioned IOPS either way. One GCP local NVMe disk wrote at 30 us
QD1, capped at 335 MiB/s per disk. An m8id.12xlarge instance-store disk did
8 KiB random writes at 30 us QD1 and 2.8 GiB/s max (reads: 84 us, 5.9 GiB/s);
writes land in the drive's DRAM cache, so they beat reads at QD1. A d3.xlarge
HDD did 8 KiB random writes at 4.2 ms QD1 and 2 MiB/s max (237 IOPS); the
drive's write cache reorders them, so they beat random reads (12 ms). For an
HDD, test the whole disk with `--size 1800g --no-precondition` so random reads
seek across all of it. The October 2026 rows came from local NVMe on GCP C4 and
AWS m8id/m8gd, EBS gp3 at 16k IOPS / 1000 MiB/s, GCP Hyperdisk Balanced at
80k IOPS / 1600 MiB/s on a c4-standard-16, and the instance-store HDDs of an AWS
d3.xlarge.

Those numbers were first measured with `fio`. On October 27, 2026, `storage` and
`fio` ran side by side on m8id instance store, EBS gp3, C4 local SSD, Hyperdisk
Balanced and a d3 HDD, raw and on ext4. Every `O_DIRECT` test agreed within
~5% on throughput and a few us on latency (QD128 on an ext4 file on m8id came
out ~10% lower). Buffered 8 KiB reads were 15-50% faster under `fio`, which
hints the kernel to read further ahead on sequential jobs; no README row uses
them.

These rows used to come from a Rust harness in `src/main.rs`, whose sequential
read benchmark reread the same 1 GiB file from page cache after the first pass.
That is where the README's old `8 GiB/s` sequential SSD read came from.

### Network Disk, Zonal and Regional

On October 27, 2026, `storage` (raw device) and `storage_durable_write` (ext4)
ran on a zonal and a regional disk with the same provisioning, attached to one
48 vCPU VM per cloud: Hyperdisk Balanced and Hyperdisk Balanced High
Availability (replicas in `us-central1-c` and `us-central1-a`, 80k IOPS / 1600
MiB/s) on a c4-standard-48, and Azure Premium SSD P30 LRS and ZRS (5000 IOPS /
200 MB/s) on a Standard_D48ds_v6 in `eastus2` (p50). An earlier run on 16 vCPU
VMs gave the same numbers within a few percent.

| | Hyperdisk Zonal | Hyperdisk Regional | Azure LRS | Azure ZRS |
| --- | --- | --- | --- | --- |
| Random read 8 KiB, QD1 | 458 us | 481 us | 3.01 ms | 2.25 ms |
| Random write 8 KiB, QD1 | 392 us | 1.28 ms | 1.54 ms | 1.69 ms |
| Random read / write 8 KiB, QD128 | 633 / 633 MiB/s | 633 / 631 MiB/s | 39 / 40 MiB/s | 41 / 42 MiB/s |
| Sequential read / write 1 MiB, QD32 | 1.6 / 1.6 GiB/s | 1.6 / 1.6 GiB/s | 195 / 195 MiB/s | 195 / 203 MiB/s |
| Durable write 64 KiB, 1 writer | 548 us | 1.37 ms | 1.89 ms | 2.01 ms |
| Durable write 64 KiB, 32 writers | 1.6 GiB/s | 1.3 GiB/s | 119 MiB/s | 195 MiB/s |
| Appending durable write 8 KiB | 1.21 ms | 3.93 ms | 6.12 ms | 5.16 ms |

The regional Hyperdisk adds ~0.9 ms to every write, the round trip to its
second zone, and matches the zonal one on reads and throughput. Azure ZRS
writes within ~10% of LRS on the same size, but a P30 is slower than either
Hyperdisk at every queue depth. The README rows use the Hyperdisk Regional
latencies, rounded: 1 ms for an 8 KiB random write and 2 ms for a 64 KiB durable
write (1.4-2.0 ms on both clouds). Azure Premium SSD reports `write back` and
gets one flush per `fdatasync`, ~30 us from one writer and several ms with 32;
every other cloud disk here is `write through`. AWS has no regional block
storage: EBS volumes live in one zone.

Network HDDs, the same day: 2 TB pd-standard zonal and regional (replicas in
`us-central1-c` and `us-central1-a`) on an n2d-standard-48 (C4 can't attach
Persistent Disk), and a 2 TiB EBS st1 on an m8i.12xlarge (p50):

| | pd-standard Zonal | pd-standard Regional | st1 |
| --- | --- | --- | --- |
| Sequential read / write 1 MiB, QD32 | 245 / 245 MiB/s | 245 / 200 MiB/s | 444 / 468 MiB/s |
| Random read 8 KiB, QD1 / QD128 | 16.6 ms / 12 MiB/s | 17.1 ms / 12 MiB/s | 5.4 ms / 4 MiB/s |
| Random write 8 KiB, QD1 / QD128 | 433 us / 24 MiB/s | 1.09 ms / 24 MiB/s | 1.9 ms / 4 MiB/s |
| Durable write 64 KiB, 1 writer / 32 | 675 us / 192 MiB/s | 1.39 ms / 193 MiB/s | 1.56 ms / 33 MiB/s |
| Durable write 1 MiB, 32 writers | 246 MiB/s | 200 MiB/s | 343 MiB/s |

Both scale with size: pd-standard by ~0.12 MB/s and 0.75 read IOPS per GB, st1
by 40 MB/s per TB baseline and 250 MB/s per TB burst (these runs fit in the
burst). The README's Network Disk HDD rows are pd-standard, so Zonal and
Regional compare like for like. st1 streams faster but is worse at small
random I/O, which it isn't built for. Writes to pd-standard land fast and are
destaged later, so its random writes beat its random reads by ~40x.

## Durable write rows

```bash
cargo bench --bench storage_durable_write -- /mnt/disk/dw.dat --sizes 8k,64k,1m --writers 1,4,32
```

`benches/storage_durable_write.rs` has no dependencies. Each writer loops `pwrite` +
`fdatasync` over its own 64 MiB slice of a file that was fully written and
synced first, with `O_DIRECT`, and times the pair. It then does the same while
appending to a new file. Before and after each run it reads the disk's flush
and write counters from `/proc/diskstats`, so it shows whether `fdatasync`
reached the device at all. On October 27, 2026, `fio` with the same sizes and
writer counts agreed within ~5% wherever it overwrote written blocks.

On October 27, 2026, on ext4, median of three 20-second runs:

| Disk | 8 KiB, 1 writer | 64 KiB, 1 writer | Most throughput | Appending, 8 KiB |
| --- | --- | --- | --- | --- |
| m8id.12xlarge instance store | 34 us | 39 us | 2.7 GiB/s (8 KiB x32) | 140 us |
| GCP C4 local SSD (one disk) | 31 us | 50 us | 333 MiB/s (per-disk cap) | 138 us |
| EBS gp3, 16k IOPS / 1000 MiB/s | 1.0 ms | 1.6 ms | 1 GiB/s (64 KiB x32) | 3.4 ms |
| Hyperdisk Balanced, 80k IOPS / 1600 MiB/s | 374 us | 515 us | 1.6 GiB/s (64 KiB x32) | 1.3 ms |
| d3.xlarge HDD | 1.0 ms | 6.2 ms | 101 MiB/s (1 MiB x32) | 1.9 ms |

Every SSD and network disk reports `write through` in
`/sys/block/*/queue/write_cache`, and saw 0.00 flushes per commit: `fdatasync`
took ~1 us because there was nothing to send. The HDDs report `write back` and
saw exactly one flush per commit from one writer. Parallel writers share
flushes there but still get slower at 8 KiB (826 to 274 commits/s). A 1 MiB
commit to the HDD takes 14 ms at 69 MiB/s. Appending costs 3 device writes per
commit instead of 1 (data plus ext4's journal), which is the extra
`fdatasync` time in the last column. On a MacBook's SSD, which does flush,
`fdatasync` takes ~3.6 ms.

The README rows use 64 KiB for SSD and network disks, the largest write that
costs about the same as 8 KiB, and 8 KiB for HDD. Until October 26, 2026, these
rows were buffered 8 KiB appends from one writer (300 us, 30 MiB/s on SSD).

## Network rows

```bash
cargo bench --bench network -- --server        # on host A, listens on TCP 5900
cargo bench --bench network -- 10.0.0.12       # on host B, with A's private IP
```

`benches/network.rs` has no dependencies. It times 1 B and 32 KiB TCP round trips
on one connection with `TCP_NODELAY`, then streams on 1, 8 and 32 connections,
counting bytes on the receiving side. Single-flow results show per-flow caps
(5 Gbit/s on AWS outside a cluster placement group); multi-flow results show
the VM's limit. ICMP ping is often handled on a slower path on cloud networks,
so round trips are TCP.

On October 27, 2026 (p50 round trip, then streams):

| Pair | 1 B round trip | 1 flow | 8+ flows |
| --- | --- | --- | --- |
| AWS m8id.12xlarge, same AZ | 140 us | 4.96 Gbit/s | 22.3 Gbit/s |
| AWS m8id.12xlarge, `usw2-az1` -> `az2` | 578 us | 4.96 Gbit/s | 22.3 Gbit/s |
| GCP c4-standard-48, same zone | 49 us | 29.8 Gbit/s | 32.5 Gbit/s |
| GCP c4-standard-48, `us-central1-c` -> `a` | 646 us | 25.5 Gbit/s | 32.5 Gbit/s |
| GCP `us-central1` -> `us-east4` | 23.7 ms | 145 MiB/s | |
| GCP `us-central1` -> `us-west1` | 32 ms | 48 MiB/s | |
| GCP `us-east4` -> `europe-west1` | 95 ms | 32 MiB/s | |
| GCP `us-central1` -> `europe-west1` | 111 ms | 31 MiB/s | |
| GCP `us-west1` -> `asia-southeast1` | 202 ms | 12 MiB/s | |
| GCP `us-central1` -> `asia-southeast1` | 224 ms | 12 MiB/s | |

The Network Bandwidth row (3 GiB/s) is the 8+ flow results on these 48 vCPU
VMs: 22-32 Gbit/s. One cross-region flow moved 1.5-3.4 MiB per round trip
(median ~3 MiB) with Ubuntu 24.04's default TCP buffers; the README rounds that
to 2 MiB with earlier 22.04 runs (1.8-2.6 MiB). More flows add up: 32 flows from
`us-central1` to `us-east4` reached 10.7 Gbit/s.

`network` was checked against `netperf` TCP_RR and `iperf3` on every pair above.
Streams matched iperf3 within ~5%, except the GCP cross-zone single flow
(25.5 vs 19.3 Gbit/s), which moves around from run to run. Round trips matched netperf within ~5% on GCP and the WAN pairs; on AWS
both were noisy and `network` came out lower (p50 140 vs 242 us same-AZ, 578 vs
673 us cross-AZ). An October 23 run on m8id measured 81 us same-AZ and 456 us
cross-AZ, so treat the AWS round trips as 100-500 us.

## Compression rows

```bash
cargo bench --bench cpu_compress -- silesia.tar dickens ...
NAPKIN_COMPRESSION_CODECS="lz4_flex,zstd -3" cargo bench --bench cpu_compress -- file
```

`benches/cpu_compress.rs` reports the ratio and single-threaded
compress/decompress speed of lz4, snappy, zstd (-1, -3, -9, -19), gzip (-1, -6)
and brotli (-5, -11) for each file, as best of N runs over at least a second.
The CPU Compress/Decompress rows use the
[Silesia corpus](https://sun.aei.polsl.pl/~sdeor/index.php?page=silesia) (on the
c4-standard-8: lz4 2.1x at 442/1246 MiB/s, zstd -1 2.9x at 395/819 MiB/s). The
ratio table used Silesia's `dickens` (English), `samba` (source) and `mozilla`
(executables), a Wikipedia article (HTML), a tarball of the cargo registry
sources, `/usr/bin` and a page of GitHub API JSON.
The Logs row is loghub's full `Apache.log` (5 MiB, 11x lz4 to 29x zstd -19) and
`Linux.log` (2 MiB, 6x to 20x).

The structured-data tables and the two CPU Decode rows come from
`benches/compression_ratio.rs`, built with `--features encodings` so the default
build doesn't pull in Parquet:

```bash
cargo bench --features encodings --bench compression_ratio -- parquet taxi.parquet hits_0.parquet
cargo bench --features encodings --bench compression_ratio -- cardinality taxi.parquet hits_0.parquet
cargo bench --features encodings --bench compression_ratio -- postings collection.tsv Apache.log ...  # also 3-/4-gram and sparse n-gram indexes
cargo bench --features encodings --bench compression_ratio -- postings --paragraphs dickens webster
cargo bench --features encodings --bench compression_ratio -- timeseries nab/*.csv
```

On October 24, 2026 (ratios on any machine; speeds single-threaded on an AWS
m8id.12xlarge, Xeon 6975P-C, reported against the decoded output size):

- Parquet, against Arrow's in-memory buffers: NYC taxi (yellow, January 2024,
  3M rows x 19 columns, mostly numeric) 5.4x with dictionary + RLE and 9.0x
  with zstd -3; ClickBench `hits_0.parquet` (1M rows x 105 columns, many
  strings) 2.2x and 10.4x. Plain encoding is 1.0x. Decoding to Arrow: plain
  5.4 / 3.0 GiB/s, dictionary + RLE 4.2 / 2.6 GiB/s, + zstd 2.1 / 1.7 GiB/s
  (taxi / hits).
- Inverted indexes over 11 corpora, against 4 bytes per doc ID, frequency and
  position, with lowercased alphanumeric tokens. Doc IDs are delta-coded and
  bitpacked in 128-value blocks, frequencies (minus one) bitpacked, positions
  delta-coded within each document and bitpacked. One document per line,
  except per paragraph for `dickens` and `webster`:

  | Corpus | Docs | Postings | Doc IDs | Freqs | Positions | All | Index / text | With positions |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | MS MARCO passages | 8.8M | 358M | 3.3x | 11.7x | 4.5x | 4.9x | 0.19 | 0.33 |
  | enwik8 (Wikipedia XML) | 1.1M | 11M | 2.5x | 8.9x | 4.1x | 4.0x | 0.28 | 0.43 |
  | Silesia `dickens` | 33k | 1.3M | 3.7x | 7.7x | 3.8x | 4.4x | 0.24 | 0.43 |
  | Silesia `webster` | 218k | 4.9M | 3.5x | 8.3x | 4.3x | 4.7x | 0.24 | 0.42 |
  | Silesia `samba` (source) | 596k | 2.7M | 2.9x | 12.0x | 6.1x | 5.1x | 0.27 | 0.36 |
  | Silesia `xml` | 58k | 322k | 3.1x | 8.2x | 4.2x | 4.3x | 0.15 | 0.30 |
  | loghub `Apache.log` | 56k | 1.0M | 5.0x | 64.6x | 8.3x | 8.9x | 0.19 | 0.29 |
  | loghub `Linux.log` | 26k | 426k | 4.6x | 17.7x | 6.8x | 7.1x | 0.27 | 0.38 |
  | ClickBench `Title` | 936k | 11M | 3.2x | 21.8x | 6.3x | 5.8x | 0.13 | 0.19 |
  | ClickBench `URL` | 1.0M | 13M | 3.7x | 18.9x | 6.2x | 6.2x | 0.25 | 0.34 |
  | ClickBench `SearchPhrase` | 69k | 274k | 2.9x | 7.0x | 5.7x | 4.5x | 0.31 | 0.36 |

  The last two columns divide the whole index by the source file's size:
  an uncompressed term dictionary (term bytes, a length byte and a 4-byte
  offset per term) plus bitpacked doc IDs and frequencies, then also
  positions. The index without positions is ~25% of the text, with
  positions ~35%.

  Regex indexes in the style of Google Code Search: every byte trigram or
  fourgram (ASCII-lowercased) with the bitpacked doc IDs that contain it, no
  frequencies or positions, plus n bytes and a 4-byte offset per n-gram.
  Sparse n-grams are GitHub code search's, from its `sparse-ngrams` crate
  over `casefold`-folded text: every bigram, plus each 3-8 byte substring
  whose end bigrams are rarer than every bigram between them, as opaque
  32-bit keys (4 bytes plus a 4-byte offset each). Index size / text size,
  per document as above and with consecutive documents merged into ~16 KiB
  chunks:

  | Corpus | 3-grams per doc | 4-grams per doc | Sparse per doc | 3-grams per 16 KiB | 4-grams per 16 KiB | Sparse per 16 KiB |
  | --- | --- | --- | --- | --- | --- | --- |
  | MS MARCO passages | 0.55 | 0.84 | 1.54 | 0.08 | 0.23 | 0.41 |
  | enwik8 (Wikipedia XML) | 0.90 | 1.26 | 2.53 | 0.14 | 0.38 | 0.73 |
  | Silesia `dickens` | 0.58 | 0.94 | 1.89 | 0.10 | 0.32 | 0.74 |
  | Silesia `webster` | 0.66 | 0.94 | 1.95 | 0.09 | 0.27 | 0.57 |
  | Silesia `samba` (source) | 1.01 | 1.30 | 2.67 | 0.24 | 0.52 | 0.88 |
  | Silesia `xml` | 0.46 | 0.67 | 1.33 | 0.12 | 0.29 | 0.48 |
  | loghub `Apache.log` | 0.73 | 0.81 | 1.87 | 0.03 | 0.07 | 0.11 |
  | loghub `Linux.log` | 0.78 | 0.93 | 2.06 | 0.10 | 0.21 | 0.36 |
  | ClickBench `Title` | 0.61 | 0.86 | 1.15 | 0.04 | 0.07 | 0.11 |
  | ClickBench `URL` | 0.92 | 1.08 | 2.29 | 0.11 | 0.22 | 0.37 |
  | ClickBench `SearchPhrase` | 0.71 | 0.96 | 1.19 | 0.06 | 0.15 | 0.24 |

  The 16 KiB medians are 0.1 for trigrams, 0.25 for fourgrams and 0.4 for
  sparse n-grams, which store 2-3x as many postings as trigrams so that a
  query reads a few selective lists instead of many common ones. Source code
  is the hardest case (0.24 trigrams on `samba`, close to the ~20% of source
  Russ Cox reported for Code Search), and repetitive logs and tables the
  easiest.

  On MS MARCO, delta + varint gets 3.0x on doc IDs and roaring 2.1x. Logs
  and ClickBench rows repeat a lot, so their doc IDs cluster and most
  frequencies are 1. Unpacking MS MARCO's full 128-ID blocks with
  `bitpacking`'s `BitPacker4x` ran at 3.1 billion IDs/s, 11.6 GiB/s of u32
  output. Packing them ran at 8.8 GiB/s of u32 input, against 12.2 GiB/s
  for unpacking on the same October 26 run, so one row covers both. Writing
  Parquet from Arrow with dictionary + RLE + zstd -3 ran at 0.43 GiB/s on taxi
  and 0.17 GiB/s on ClickBench (105 columns, many strings), against 2.1 and 1.8
  GiB/s for reading it back: encoding is 5-10x slower than decoding.
- Time series from the Numenta Anomaly Benchmark (9 real CloudWatch and sensor
  series at 5-minute intervals), against 16 bytes per point: Gorilla gets
  2.2-2.8x (~7 B/point) on float gauges like CPU utilization and temperature,
  and 7-13x (1.2-2.4 B/point) on integer-valued counts. Regular timestamps cost
  1 bit. Column-wise delta + zstd -3 gets 4.0x overall.

The cardinality rows were checked with `compression_ratio cardinality`, which
writes each column of taxi and ClickBench `hits_0` on its own with Parquet's
dictionary + RLE-hybrid encoding (uncompressed, one dictionary per column) and
compares its size with the smaller of log2(distinct) bits per value and 3 bytes
per run, each plus the dictionary (every distinct value once, strings with a
4-byte length). Whole tables land within 6% (taxi: actual 0.94x of the rule)
and 1% (hits: 1.01x); log2(distinct) alone overestimates them by 12% and 24%.
Per column, among the 59 at 1 bit per value or more, the median error is 15%
and 50 are within 50%. Low-cardinality columns with skewed values come in under
the rule (taxi `payment_type`: 5 values, 1.9 bits instead of 3), because the
RLE-hybrid catches short runs of the common value. Columns whose runs are
shorter than 8 come in over it (`ClientIP`: 7.0 bits, rule 4.3), because
Parquet only run-length encodes runs of 8 or more.

The Floats / Integers / Strings rows come from `benches/compression_ratio_columns.rs`
(same feature), which runs the crates Vortex builds on: spiraldb's `alp`,
`fastlanes` and `fsst-rs`. Every column is round-tripped and compared bit for
bit, and decode is timed single-threaded, best of 5, against the decoded size:

```bash
cargo bench --features encodings --bench compression_ratio_columns -- floats taxi.parquet gov10.bin ...
cargo bench --features encodings --bench compression_ratio_columns -- ints taxi.parquet hits_0.parquet
cargo bench --features encodings --bench compression_ratio_columns -- strings hits_0.parquet passages.txt ...
```

On October 25, 2026 (ratios on any machine; speeds on an AWS m8id.12xlarge,
Xeon 6975P-C, built with `-C target-cpu=native`):

- Floats: ALP per 1024-value vector, cascaded into frame of reference and
  bitpacking, with exceptions at 10 bytes each, or ALP-RD where that is smaller
  (full-precision values). Two departures from the `alp` crate's defaults:
  its `bit_width` charges constant vectors 1 bit per value instead of 0, and
  its exponent search on a 32-value sample misses rare outliers that are
  cheaper as exceptions, so exponents are searched exhaustively per vector.
  On the ALP paper's 30 double datasets (raw files from the authors' data
  folder) that reproduces the paper's published bits per value with a median
  difference of 0.0 bits (Gov/26 0.4 vs 0.41, Stocks-USA 7.9 vs 7.9, POI-lat
  55.6 vs 55.5; worst Blockchain 41.3 vs 36.2 and Gov/30 8.9 vs 7.5). The
  median ratio across the folder's 31 double files is 3.9x, the same as
  zstd -3 over the raw bytes (3.9x). Decimals like prices and
  temperatures take 8-16 bits per value (4-8x); full-precision coordinates
  55 bits (1.15x, ALP-RD). Low-cardinality taxi columns (`mta_tax`,
  `improvement_surcharge`) get 15-30x from ALP and 100-200x from zstd: they
  want a dictionary or RLE. Decode: 13-17 GiB/s when the column fits in
  cache, 7-9 GiB/s on the 1 GB datasets, ALP-RD ~7 GiB/s.
- Integers: frame of reference + FastLanes bitpacking per 1024-value vector,
  or FastLanes delta (transposed, one base per lane) under it where smaller.
  Each value takes about log2 of its vector's range: taxi timestamps 33-35
  bits (1.9x), ClickBench `EventTime` 17 bits (3.8x), location IDs 9 bits,
  flags 0-2 bits, hashes and random IDs their full width. Overall 4.2x on
  taxi's 8 integer columns (zstd 5.2x) and 2.5x on ClickBench's 76 (zstd
  7.7x: `hits` is sorted by counter, date and user, so its values come in
  runs that RLE or a dictionary would catch first). Decode 12-18 GiB/s, delta
  ~6 GiB/s.
- Strings: FSST with one symbol table per 64 Ki strings, compressed lengths
  bitpacked so every string decodes on its own, against the bytes plus a
  4-byte offset each. ClickBench `Title` 2.06x, `URL` 1.97x, `Referer` 1.86x,
  `OriginalURL` 2.33x; loghub `Apache.log` 2.9x and `Linux.log` 2.2x, one
  string per line; enwik8 lines 1.71x; MS MARCO passages 1.79x. zstd -3 over
  each whole column gets 3-11x. Decode 2-4 GiB/s.

## Blob storage rows (March 2026)

On the same setup (128 requests in flight from a VM in the same region), the
zonal tiers S3 Express One Zone and GCS Rapid (gRPC, reads through a plain and
an open object handle) gave p99 (p50), median over runs:

| | S3 | GCS gRPC | Azure | S3 Express One Zone | GCS Rapid |
| --- | --- | --- | --- | --- | --- |
| Read 128 KiB | 77 (58) ms | 56 (16) | 97 (66) | 60 (50) | 26-27 (12-13) |
| Write 128 KiB | 154 (64) | 373 (66) | 38 (22) | 104 (51) | 164-333 (80-85) |
| Conditional write (if-match) 128 KiB | - | 105 (63) | 20 (8) | 33 (6) | 3.9-7.4 s (61-64 ms) |
| Conditional write (if-none-match) 128 KiB | 124 (46) | 153 (60) | 330 (23) | 484 (35) | 149-469 (72-75) |
| List 1000 keys | 131 (78) | 87 (42) | 174 (88) | 27 (23) | 152-189 (69) |

Zonal buckets only win on reads (Blob Storage, Zonal Read: 50 ms). Zonal
writes are no faster than regional ones (100 ms, from S3 Express; GCS Rapid is
slower). The Zonal Conditional Write row (500 ms) uses if-none-match, where
both tiers land at 150-500 ms; GCS Rapid's if-match p99 runs to seconds. The
Zonal List row (100 ms) splits S3 Express (27 ms) and GCS Rapid (150-190 ms).
Both zonal tiers cost about \$0.11 per GB-month, ~5x regional.


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

## GPU rows

The GPU rows are NVIDIA's B200 (HGX, SXM) specs, not measured here, rounded
down to what kernels reach in practice:

- GPU Memory Sequential Read: HBM3e at 8 TB/s; copy kernels reach ~90%.
- GPU ↔ GPU Transfer, NVLink: NVLink 5 at 1.8 TB/s, 900 GB/s per direction.
- GPU ↔ CPU Transfer, PCIe: PCIe 5.0 x16 at 64 GB/s per direction; pinned
  `cudaMemcpy` reaches ~55 GB/s, and a small copy costs ~10 μs.
- Network RDMA, GPU: one 400 Gbit/s InfiniBand or RoCE NIC per GPU, ~5 μs
  round trip.

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
