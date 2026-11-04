# Benchmarking

How the numbers in the [README](README.md) are measured, and how to refresh
them. Everything here runs on Linux; the Criterion suite also builds on macOS
(without the Linux-only groups and host tuning).

## Running on a fresh VM

```bash
script/bench-vm aws m8g.48xlarge --ttl 60 -- 'cargo bench --bench napkin_math -- memory_'
script/bench-vm gcp c4-standard-48-lssd --ttl 45 --detach -- 'cargo bench --bench storage -- ...'
script/bench-vm list      # bench VMs still alive
script/bench-vm sweep     # delete any past their deadline
```

`script/bench-vm` uploads the working tree to a bucket and boots an Ubuntu 24.04
VM that installs Rust, restores a `target/` cached for its CPU model and
`Cargo.lock`, runs the command in the repo with `-C target-cpu=native`, uploads
the output, and deletes itself (GCP VMs only stop, so the launcher deletes them
once the output is in). Every VM also has a hard deadline: a shutdown timer
plus terminate-on-shutdown on AWS, `--max-run-duration` with `DELETE` on GCP.
A cold run takes 2-4 minutes from launch to output. Bucket, project and
instance profile go in a gitignored `.bench-vm.env`; the script's header lists
the keys.

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
| `memory_sequential_read` | Sequential reads of 1 GiB, 1 thread and all cores, scalar and SIMD, and 1 thread on huge pages | Memory Sequential Read |
| `memory_sequential_write` | memset and memcpy of 1 GiB on 1 thread, memset of 16 GiB on all cores, and on x86 the same memsets with non-temporal stores | Memory Sequential Write |
| `memory_random` | One pass of independent random 64 B reads/writes over 1 GiB (1 thread), and random lines of a shared 16 GiB buffer from 1 thread and all cores, with and without huge pages | Memory Random Read/Write, throughput |
| `memory_random_read_latency` (in `memory_random.rs`) | Dependent pointer chase over 1 GiB, with and without huge pages | Memory Random Read, latency |
| `cache_read`, `cache_read_latency` | The `memory_sequential_read` SIMD loop, and the pointer chase, over 16 KiB, 256 KiB and 8 MiB | L1/L2/L3 Cache Read |
| `cpu_encrypt` | ring AES-256-GCM and ChaCha20-Poly1305 seal and open on 16 KiB | CPU Encrypt / Decrypt |
| `cpu_hash`, `cpu_hash_1mib` | sha256, blake3, crc32, siphash, xxh3 on 64 B calls and 1 MiB buffers | CPU Hash rows |
| `cpu_bitpack` | `bitpacking`'s BitPacker4x packing and unpacking 256 MiB of 12-bit integers | CPU Encode / Decode Bitpacked Integers |
| `cpu_serialize` | Protobuf (prost), JSON (serde_json, to structs and to a dynamic `Value`) and postcard on a 1 MiB batch of events | CPU Serialize, CPU Deserialize |
| `cpu_sort` | `sort_unstable` on random `u64`, 1 MiB to 1 GiB | CPU Sort |
| `cpu_atomic_mutex` | Atomic increment and `Mutex` lock + unlock, alone and with a second core on the same line | CPU Atomic Increment, CPU Mutex Lock + Unlock |
| `cpu_system_call` | `getppid` (null syscall), `getpid`, `clock_gettime`, `getrusage`, `fstat` | CPU System Call |
| `cpu_context_switch` | futex ping-pong between two threads, same core and cross core (Linux) | CPU Context Switch |
| `cpu_handshake` | TCP connect over loopback, and a full TLS 1.3 handshake (rustls, ECDSA P-256, X25519) in memory | CPU TCP Connect, CPU TLS 1.3 Handshake |
| `cpu_spawn` | `thread::spawn` + join, and spawning `true` and waiting for it | CPU Thread Spawn, CPU Process Spawn |

On November 4, 2026, the whole suite ran on a c4-standard-48 (Xeon 8581C), an
m8i.12xlarge (Xeon 6975P-C) and an m8g.12xlarge (Graviton4), one thread unless
noted:

| | C4 | m8i | m8g | README |
| --- | --- | --- | --- | --- |
| L1 / L2 / L3 latency (16 KiB / 256 KiB / 8 MiB) | 1.5 / 4.3 / 25 ns | 1.6 / 4.4 / 35 ns | 1.8 / 4.1 / 93 ns | 2 / 5 / 50 ns |
| L1 / L2 / L3 read | 330 / 169 / 37 GiB/s | 300 / 175 / 28 | 101 / 76 / 51 | 200 / 150 / 40 |
| Memory read, 1 thread / 48 threads | 18.2 / 152 GiB/s | 21.4 / 203 | 28.6 / 293 | 30 / 200 (below) |
| Memory write, 1 thread / 48 threads (non-temporal on x86) | 28.1 / 100 GiB/s | 22.0 / 150 | 59.0 / 321 | 30 / 200 |
| Random 64 B read / write, 1 thread, 16 GiB | 2.9 / 2.2 GiB/s | 2.5 / 3.7 | 1.9 / 1.7 | 3 / 3 |
| Random 64 B read / write, 48 threads, 16 GiB | 56 / 39 GiB/s | 43 / 65 | 137 / 128 | 100 / 100 |
| Random read latency, 1 GiB, huge pages / 4 KiB pages | 102 / 129 ns | 128 / 175 | 116 / 154 | 100 |
| AES-256-GCM encrypt / decrypt | 9.7 / 9.9 GiB/s | 9.5 / 9.7 | 4.1 / 4.1 | 10 |
| xxh3, 64 B / 1 MiB | 2.8 ns / 68 GiB/s | 2.7 / 69 | 5.4 / 20 | 5 ns / 50 GiB/s |
| crc32 / siphash, 64 B | 27 / 18 ns | 29 / 17 | 21 / 23 | |
| sha256, 64 B / 1 MiB | 84 ns / 1.8 GiB/s | 85 / 1.75 | 80 / 1.43 | 100 ns / 2 GiB/s |
| BitPacker4x pack / unpack, 12 bits | 9.6 / 61 GiB/s | 14.5 / 56 | 18.9 / 47 | 10 / 50 |
| Protobuf serialize / deserialize | 1.08 GiB/s / 414 MiB/s | 1.09 / 406 | 1.17 / 295 | 1 GiB/s / 500 MiB/s |
| JSON serialize / deserialize (to `Value`) | 1.18 GiB/s / 464 (226) MiB/s | 1.16 / 444 (214) | 0.99 / 377 (201) | |
| Sort `u64`, 1 MiB / 64 MiB / 1 GiB | 634 / 474 / 339 MiB/s | 615 / 451 / 341 | 560 / 401 / 349 | 300 (1 GiB) |
| Atomic increment / contended | 6.2 / 30 ns | 6.6 / 35 | 6.5 / 19 | 5 / 30 |
| Mutex lock + unlock / contended | 12.7 / 142 ns | 13.0 / 335 | 14.7 / 108 | 10 / 200 |
| `getppid` / `stat` / `getrusage` | 80 / 213 / 773 ns | 82 / 218 / 789 | 134 / 271 / 1028 | 100 |
| futex round trip, same core / cross core | 1.50 / 3.07 us | 1.41 / 3.92 | 1.80 / 5.04 | 1 / 2 us per switch |
| Thread spawn + join / process spawn + wait | 31 / 345 us | 43 / 430 | 25 / 311 | 30 / 300 |
| Loopback TCP connect / TLS 1.3 handshake (both sides) | 8.7 / 273 us | 8.2 / 281 | 11.9 / 296 | 10 / 300 |

A futex round trip is two wake-ups, so the README halves it: a context switch
costs ~1 us, and waking a thread on another core ~2 us. Graviton4's AES-GCM
is ring's ARM path at 4 GiB/s, against 10 on x86. Serialization runs at ~1
GiB/s for every format, and deserializing into structs at 300-450 MiB/s (200
into a dynamic JSON `Value`): allocating the objects, not the format, sets the
speed. TCP connect and the TLS handshake each add one network round trip on
top. Bitpacking unpacks a 128-value block at a time into a buffer that stays in
L1, the way a reader consumes a column; FastLanes decoding whole columns into
memory runs at 10-17 GiB/s (below).

The README collapses `memory_sequential_read`'s scalar and SIMD variants into one
single-thread row and one threaded row for memorability. `memory_random` issues
independent loads, so the CPU overlaps misses; that is throughput, which is why
the latency cell comes from `memory_random_read_latency` instead.

### Memory writes

On November 2, 2026, writing 1 GiB from one thread and 16 GiB from 48, in GiB/s:

| | C4, Xeon 6985P-C | m8id, Xeon 6975P-C | m8gd, Graviton4 |
| --- | --- | --- | --- |
| 1 thread, memset / plain stores | 10.4 | 11.6 | 61 |
| 1 thread, non-temporal stores | 24.9 | 23.0 | 61 |
| 48 threads, memset | 107 | 142 | 293 |
| 48 threads, non-temporal stores | 136 | 149 | 289 |

An x86 store to a line that isn't cached reads the line first (read for
ownership), so a memset moves every byte across the memory bus twice;
non-temporal stores (`vmovntdq`, AVX2 or AVX-512 alike) write whole lines
without reading them. glibc's memset of 1 GiB is `rep stosb` here and doesn't
use them. Graviton4 notices full-line writes and skips the read on its own:
`stp`, `st1`, `stnp`, `DC ZVA` and memset all wrote 61 GiB/s, twice its read
speed. The README rows (30 and 200 GiB/s) are the non-temporal x86 numbers
averaged with Graviton4's.

### Random memory, one thread and all cores

On November 2, 2026, random 64-byte lines of a shared 16 GiB buffer (each
pinned thread draws its own indices from xorshift), in GiB/s, 4 KiB pages
(2 MiB huge pages in parentheses):

| | C4, Xeon 8581C | m8i, Xeon 6975P-C | m8g, Graviton4 |
| --- | --- | --- | --- |
| Read, 1 thread | 2.9 (3.6) | 2.5 (3.2) | 2.1 (4.6) |
| Write, 1 thread | 2.0 (2.5) | 3.7 (6.3) | 1.8 (9.9) |
| Read, 48 threads | 55 (61) | 42 (54) | 146 (189) |
| Write, 48 threads | 38 (41) | 64 (63) | 144 (333) |

The README rounds the averages: 3 GiB/s on one thread and 100 GiB/s on all
cores, which hides that x86 is 40-60 and Graviton4 ~150. Graviton4's random
writes with huge pages (333 GiB/s, then 282 in two later runs) run at its
sequential write speed: it skips reading lines it writes whole, so random
writes cost what sequential ones do once the TLB stops missing. Huge pages help one thread 1.3-2.2x: with 4 KiB pages, 16 GiB is
4M pages, far more than the TLB holds, so most accesses also walk the page
table. The 1 GiB pass through a shuffled index array, the older bench, reads
2.2-2.5 GiB/s; on an Apple M5 Max the xorshift loop was 1.8x faster (7.4 vs 4.1
GiB/s), on these server cores within 15%.

### SIMD width, prefetching and NUMA

"SIMD" is the widest the CPU has: AVX-512, else AVX2, else the NEON that LLVM
emits on ARM. On AVX-512 hosts the groups add an `AVX2` row for comparison. On
November 1, 2026, single thread, in GiB/s, on a c4-standard-48 (Xeon 8581C), an
m8i.12xlarge (Xeon 6975P-C) and an m8g.48xlarge (Graviton4):

| Read | C4 AVX-512 | C4 AVX2 | m8i AVX-512 | m8i AVX2 | Graviton4 NEON |
| --- | --- | --- | --- | --- | --- |
| 16 KiB (L1) | 327 | 213 | 302 | 223 | 102 |
| 256 KiB (L2) | 168 | 151 | 175 | 147 | 77 |
| 8 MiB (L3) | 35 | 36 | 28 | 28 | 54 |
| 1 GiB (DRAM) | 18.7 | 15.6 | 21.4 | 20.7 | 31 |

Wider vectors pay off in L1 and L2, not from DRAM, where the core's limit on
outstanding misses sets the speed. Graviton4's NEON is an explicit loop with
eight 16-byte accumulators: LLVM's own loop read L1 at 83 GiB/s, eight
accumulators or 64-byte `ld1` loads at 99-102, and nothing else moved L2, L3
or DRAM. The README's L1 row averages to 244 GiB/s and rounds to 200.

AVX-512 from DRAM first ran at 17 GiB/s on m8i, below AVX2 (22) and scalar
(20). It wasn't the clock: 256- and 512-bit adds both ran at 3.86 GHz. Intel's
L1 streaming prefetcher only starts on a second ascending load to a line that
was just loaded; AVX2 makes two loads per 64-byte line and AVX-512 one. An AVX2
loop that reads only half of each line drops to the same 18.5 GiB/s. More
accumulators, more streams, and one software prefetch per line at 0.5-8 KiB
only got AVX-512 to 19. Prefetching both 1 KiB ahead into L1 and 16 KiB ahead
into L2 got it to 21-23 GiB/s on m8i and 18.5-18.7 on C4, so buffers over 64 MiB now
take that path.

That is about the limit for one core. By Little's law, bandwidth is lines in
flight x 64 B / latency: 21-23 GiB/s at 136 ns is ~50 lines, about what a
Granite Rapids L2 tracks. On Graviton4 every loop shape (LLVM's, NEON with 4-16
accumulators, 64-byte `ld1`, `ldnp`, software prefetch) read 31 GiB/s from one
stream. Reading two places at once, the buffer's two halves interleaved, read
35 (~67 lines at 115 ns): the hardware prefetcher runs only so far ahead of
each stream it tracks, and a second stream gets its own. On x86, which already
prefetches in software, two streams were 3-6% slower. An Apple M5 Max core
reads 85-106 GiB/s at the same ~110 ns, ~190 lines in flight.

On November 4, 2026, the same loops over 2 MiB huge pages (single thread, GiB/s,
4 KiB pages / huge pages): AVX-512 with prefetch 19.5 / 21.6 on C4 and 19.0 /
21.0 on m8i, AVX2 19.1 / 20.4 and 18.9 / 20.2, scalar 16.2 / 19.5 and 16.1 /
19.5, and Graviton4's NEON 31.0 / 31.1. With 4 KiB pages the prefetchers stop
at every page boundary and loads miss the TLB; Graviton4 doesn't care.
Prefetch distances from 1K/16K to 2K/64K moved AVX-512 by 2% at most. The
bench now reads two streams on ARM and has a huge-pages row; its run the same
day gave 18.5 / 22.2 GiB/s on C4, 20.4 / 19.4 on m8i and 35.2 / 33.9 on m8g
(4 KiB / huge pages). The README's 1-thread row (30 GiB/s) averages the best of
each: ~20 on x86 and 35 on Graviton4. On November 4 the C4's scalar loop first
read 6.6 GiB/s in the full suite and 15.6-16.2 in two later runs on fresh VMs:
treat one slow number on a cloud VM as noise until it repeats.

On a two-socket m8g.48xlarge (Graviton4, 96 cores per socket), with `numactl`
pinning threads to node 0 and memory to node 0 or node 1: random read latency
113 ns local and 258 ns remote (135 / 282 ns with 4 KiB pages); one thread
reads 31 GiB/s local and 16 remote; 96 threads read 325 GiB/s local and 62
GiB/s remote, the socket-to-socket link. 192 threads over both sockets read
210 GiB/s, which says more about 5 MiB per thread being too short than about
the hardware. The 48 vCPU VMs the README uses have one NUMA node.

An m8i.48xlarge (Xeon 6975P-C, 192 vCPU) is one socket in three NUMA nodes, one
per compute die (sub-NUMA clustering). From node 0 to node 1's memory: latency
127 -> 153 ns (174 -> 208 ns with 4 KiB pages), one thread 21 -> 15 GiB/s, and
node 0's 64 threads 199 -> 173-189 GiB/s. A die boundary costs far less than a
socket boundary. An x86 two-socket run needs the 384 vCPU shapes.

## SSD, Network Disk and HDD rows

```bash
cargo bench --bench storage -- /mnt/nvme/disk.dat                       # a file on a mounted file system
cargo bench --bench storage -- /dev/nvme1n1 --destroy /dev/nvme1n1      # an unused raw device
```

`benches/storage.rs` runs sequential 1 MiB reads and writes at QD32,
sequential and random 8 KiB reads and writes at QD1, random 8 KiB at QD128, and buffered 8 KiB sequential reads
and writes, all with `O_DIRECT` except the buffered ones. On Linux the
`O_DIRECT` tests use io_uring (the only dependency): one thread with its own
ring, the file and buffers registered, reaping completions a quarter of the
queue at a time. Random reads at QD128 use two threads of 64, because one core
does only 500-850k small reads per second. Elsewhere, and for buffered I/O,
queue depth comes from that many threads each doing one blocking `pread` or
`pwrite`. On November 2, 2026, one thread (two for random reads) matched the
earlier 32-128-thread runs on all three NVMe VMs, within 14%. It writes
the whole range first (reads of never-written blocks can skip the flash), drops
the page cache before buffered reads, checks that the file system the file
actually lands on has 2x the test size free, and refuses to write to a mounted
device or without `--destroy`. A file target includes the file system's cost; a
raw device does not.

The README uses the max-throughput results (1 MiB QD32 for sequential, 8 KiB
QD128 for random) and 8 KiB QD1 latency (p50) for both. `--only
seq_read_8k_qd1,...` runs a subset. Local NVMe writes land in
the drive's DRAM cache, so they beat reads at QD1 (30 us against 90). For an
HDD, test the whole disk with `--size 1800g --no-precondition` so random reads
seek across all of it. The rows come from local NVMe on GCP C4 and AWS
m8id/m8gd, the network disks and HDDs below.

On November 1, 2026, all local disks of a VM were striped (`mdadm` RAID0, 256
KiB chunks) and measured with `storage` and `fio` (io_uring, 4 KiB random at
QD64 x 16 jobs). Each met or beat its published limit:

| Whole VM | Seq read 1 MiB | Seq write 1 MiB | 4 KiB rand read | 4 KiB rand write | 8 KiB rand read QD128 / QD1 | 8 KiB rand write QD128 / QD1 |
| --- | --- | --- | --- | --- | --- | --- |
| c4-standard-48-lssd, 8 x 375 GiB | 5.2 GiB/s (limit 5,000 MiB/s) | 2.7 GiB/s (2,640) | 1.33M IOPS (1.2M) | 683k (600k) | 5.2 GiB/s / 94 us | 2.7 GiB/s / 39 us |
| m8id.12xlarge, 1 x 2850 GB | 6.1 GiB/s | 2.7 GiB/s | 858k (805k) | 429k (402k) | 5.9 GiB/s / 94 us | 2.8 GiB/s / 31 us |
| m8gd.12xlarge, 3 x 950 GB | 6.0 GiB/s | 2.7 GiB/s | 850k (805k) | 423k (402k) | 5.9 GiB/s / 71 us | 2.8 GiB/s / 19 us |

From one thread (`fio --numjobs=1`, io_uring), on the same striped disks:

| One thread | c4-standard-48-lssd | m8id.12xlarge | m8gd.12xlarge |
| --- | --- | --- | --- |
| Seq read 1 MiB QD32 | 5.2 GiB/s (15% of a core) | 5.9 | 5.9 |
| Seq write 1 MiB QD32 | 2.7 GiB/s | 2.7 | 2.7 |
| 8 KiB rand read QD128 | 486k IOPS (3.7 GiB/s) | 610k (4.7) | 469k (3.6) |
| 4 KiB rand read QD256, batched | 702k IOPS | 842k | 637k |
| 4-8 KiB rand write | 631k IOPS | 427k | 425k |

One thread reaches every sequential and random-write limit. Small random reads
take ~1.2-2 us of CPU each, mostly in the kernel (md RAID adds some), so one
core stops at 500-850k IOPS: all of m8id's one disk, half of C4's eight.
Batching submissions (`--iodepth_batch_submit=32`) and registered buffers and
files were worth 1.3-1.9x.

The storage bench's own 4 KiB random tests (2 threads x 128 for reads, 1 x 256
for writes) matched `fio`: 1.32M / 678k IOPS on C4, 757k / 436k on m8id (88% of
`fio`'s reads, 94% of the published limit), 867k / 435k on m8gd. So the bench,
not `fio`, is the tool; run `fio` once on a new kind of disk to check the bench
still reaches its limits there.

On November 4, 2026, on the same striped disks (C4 / m8id / m8gd, raw device,
p50), one 8 KiB request at a time took 79 / 71 / 16 us to read sequentially
(94 / 95 / 76 us at random) and 40 / 32 / 19 us to write either way. One 1 MiB
request at a time took 415 / 260 / 151 us to read and 340 / 385 / 289 us to
write, so one thread with one request in flight already gets 2.3-5.0 GiB/s
reading and 2.7-2.8 GiB/s writing. Buffered 8 KiB reads, sped up only by
kernel readahead, ran at 3.6 / 1.5 / 5.2 GiB/s from the device and 5.2 / 1.5 /
3.9 GiB/s from an ext4 file.

AWS publishes no throughput limit for instance store. The README's SSD rows used
to come from one of the C4's 8 disks (650 MiB/s read, 335 write, its per-disk
caps), which is why they were 2-4 GiB/s.

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

On November 4, 2026, `storage` (raw device) and `storage_durable_write` (ext4)
ran on zonal and regional disks attached to one 48 vCPU VM per cloud:
Hyperdisk Balanced and Hyperdisk Balanced High Availability (replicas in
`us-central1-c` and `us-central1-a`, 80k IOPS / 1600 MiB/s each) on a
c4-standard-48, and a 200 GB EBS gp3 at 16k IOPS / 1000 MiB/s on an
m8i.12xlarge (p50; durable writes from one thread, 1 at a time and 32 in
flight):

| | Hyperdisk Zonal | Hyperdisk Regional | EBS gp3 |
| --- | --- | --- | --- |
| Sequential read 8 KiB QD1 / 1 MiB QD1 / 1 MiB QD32 | 333 us / 926 us / 1.6 GiB/s | 370 us / 1.05 ms / 1.6 GiB/s | 687 us / 1.86 ms / 1.0 GiB/s |
| Sequential write 8 KiB QD1 / 1 MiB QD1 / 1 MiB QD32 | 361 us / 1.12 ms / 1.6 GiB/s | 1.25 ms / 2.39 ms / 1.6 GiB/s | 1.02 ms / 3.38 ms / 1.0 GiB/s |
| Random read 8 KiB, QD1 / QD128 | 478 us / 633 MiB/s | 509 us / 633 MiB/s | 681 us / 133 MiB/s |
| Random write 8 KiB, QD1 / QD128 | 409 us / 633 MiB/s | 1.27 ms / 557 MiB/s | 1.00 ms / 133 MiB/s |
| Durable write 64 KiB, 1 / 32 in flight | 503 us / 1.6 GiB/s | 1.39 ms / 1.27 GiB/s | 1.52 ms / 0.99 GiB/s |
| Appending durable write 8 KiB | 1.21 ms | 3.93 ms | 3.50 ms |

(The 8 KiB columns were run later the same day, on the same Hyperdisks and a
new gp3 with the same provisioning; the pd-standard ones below on new disks.) Sequential 8 KiB reads beat random ones on Hyperdisk (333 against
497 us) and slightly on gp3 (687 against 735). The regional Hyperdisk adds
~0.9-1.3 ms to every write, the round trip to its
second zone, and matches the zonal one on reads and throughput. Network disks
cap random I/O at the provisioned IOPS, reads and writes alike. The README
averages the clouds and rounds: zonal is Hyperdisk and gp3, and the durable
rows also count Azure Premium SSD P30 from October 27 (64 KiB, one writer: 1.89
ms LRS, 2.01 ms ZRS), hence 1 ms zonal and 2 ms regional. Azure's P30s (5000
IOPS / 200 MB/s) were slower than either Hyperdisk at every queue depth, and
ZRS wrote within ~10% of LRS. AWS has no regional block storage: EBS volumes
live in one zone.

Network HDDs, the same day: 2 TB pd-standard zonal and regional (replicas in
`us-central1-c` and `us-central1-a`) on an n2d-standard-48 (C4 can't attach
Persistent Disk), and a 2 TiB EBS st1 on the m8i.12xlarge:

| | pd-standard Zonal | pd-standard Regional | st1 |
| --- | --- | --- | --- |
| Sequential read 8 KiB QD1 / 1 MiB QD1 / 1 MiB QD32 | 438 us / 1.17 ms / 251 MiB/s | 448 us / 1.26 ms / 245 MiB/s | - / 13.8 ms / 249 MiB/s |
| Sequential write 8 KiB QD1 / 1 MiB QD1 / 1 MiB QD32 | 429 us / 3.94 ms / 251 MiB/s | 1.08 ms / 4.61 ms / 200 MiB/s | - / 3.52 ms / 477 MiB/s |
| Random read 8 KiB, QD1 / QD128 | 17.5 ms / 12 MiB/s | 19.3 ms / 12 MiB/s | 5.2 ms / 4 MiB/s |
| Random write 8 KiB, QD1 / QD128 | 438 us / 25 MiB/s | 1.17 ms / 24 MiB/s | 1.90 ms / 4 MiB/s |
| Durable write 64 KiB, 1 / 32 in flight | 697 us / 197 MiB/s | 1.47 ms / 192 MiB/s | 1.52 ms / 33 MiB/s |

Both scale with size: pd-standard by ~0.12 MB/s and 0.75 read IOPS per GB, st1
by 40 MB/s per TB baseline and 250 MB/s per TB burst. st1 read at 444 MiB/s on
October 27 and 249 here, after a week of runs had used up its burst credits.
The README's Network Disk HDD rows are pd-standard, so Zonal and Regional
compare like for like. st1 is worse at small random I/O, which it isn't built
for. Writes to pd-standard land fast and are destaged later, so its random
writes beat its random reads by ~40x. Its sequential reads come in bursts: p50
is ~1.2 ms, but one request at a time still averages the 250 MiB/s cap (4 ms
each). pd-standard on n2d is the one network disk that reports `write back`
and gets a flush per `fdatasync`, which costs it ~70 us.

A d3.xlarge's instance-store HDD (one of three 2 TB disks, the whole disk with
`--size 1800g --no-precondition`) read and wrote sequentially at 182-196 MiB/s,
5.2-5.3 ms per 1 MiB one at a time and 78 / 74 us per 8 KiB, which the drive's
cache serves without waiting for the platter, and did 8 KiB random reads at 12 ms QD1
(199 IOPS at QD128) and random writes at 4.3 ms QD1 (237 IOPS); the drive's
write cache reorders writes, so they beat random reads. A durable 8 KiB write
took 0.97 ms from one writer (584/s, one flush each) and got slower with 32 in
flight (184/s), as the drive serves flushes one at a time.

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

Every SSD and network disk here reports `write through` in
`/sys/block/*/queue/write_cache` (Azure Premium SSD, above, reports `write
back`), and saw 0.00 flushes per commit: `fdatasync`
took ~1 us because there was nothing to send. The HDDs report `write back` and
saw exactly one flush per commit from one writer. Parallel writers share
flushes there but still get slower at 8 KiB (826 to 274 commits/s). A 1 MiB
commit to the HDD takes 14 ms at 69 MiB/s. Appending costs 3 device writes per
commit instead of 1 (data plus ext4's journal), which is the extra
`fdatasync` time in the last column. On a MacBook's SSD, which does flush,
`fdatasync` takes ~3.6 ms.

On November 4, 2026, on ext4 over all of a VM's local disks, one thread with
32 write + `fdatasync` pairs in flight on io_uring (`--depths 32`) committed
64 KiB at 2.66 GiB/s on c4-standard-48-lssd, 2.60 on m8id and 2.60 on m8gd, as
much as 32 writer threads (2.49-2.67; 2.3-2.6 on November 2). One writer alone
took 54 / 38 / 36 us per commit, 8 KiB 41 / 32 / 18 us. The README row is that
one thread.

The README rows use 64 KiB for SSD and network disks, which costs 20-100% more
than 8 KiB from one writer but reaches the disk's bandwidth, and 8 KiB for HDD. Until October 26, 2026, these
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
(median ~3 MiB) with Ubuntu 24.04's default TCP buffers, the README's 3 MiB
(1.8-2.6 MiB on 22.04). The cross-region rows round the round trips: 24-32 ms
within the US here and 55 ms `us-west1` -> `us-east4` by ping on October 23
(25-60 ms), 95-111 ms across the Atlantic (100 ms), and 202-224 ms from the US
to Singapore (200 ms; Tokyo is closer). More flows add up: 32 flows from
`us-central1` to `us-east4` reached 10.7 Gbit/s.

`network` was checked against `netperf` TCP_RR and `iperf3` on every pair above.
Streams matched iperf3 within ~5%, except the GCP cross-zone single flow
(25.5 vs 19.3 Gbit/s), which moves around from run to run. Round trips matched netperf within ~5% on GCP and the WAN pairs; on AWS
both were noisy and `network` came out lower (p50 140 vs 242 us same-AZ, 578 vs
673 us cross-AZ). An October 23 run on m8id measured 81 us same-AZ and 456 us
cross-AZ, so treat the AWS round trips as 100-500 us.

## Proxy and database rows

```bash
benches/services.sh        # on a throwaway Ubuntu 24.04 VM; it reconfigures system services
```

`benches/services.sh` installs nginx, HAProxy, Redis, Memcached, Postgres and
MySQL, loads the same 100k keys into each, and runs `benches/services.rs`: one
connection doing one request at a time over loopback, 10 seconds per target, a
random key each time. Postgres and MySQL run `SELECT v FROM kv WHERE k = ...`
on a primary key as a plain query, parsed and planned every time. nginx answers
on its own, and nginx and HAProxy each proxy to it over a kept-alive upstream
connection. On November 4, 2026, mean round trip in us:

| | C4, Xeon 8581C | m8i, Xeon 6975P-C | m8g, Graviton4 |
| --- | --- | --- | --- |
| nginx | 19 | 32 | 12 |
| nginx, through nginx | 41 | 67 | 26 |
| nginx, through HAProxy | 43 | 68 | 27 |
| Redis GET | 18 | 31 | 20 |
| Memcached get | 15 | 33 | 14 |
| Postgres, primary key | 29 | 42 | 29 |
| MySQL, primary key | 28 | 44 | 40 |

A proxy adds 15-35 us (24 on average), a primary-key query takes 30-45 us and
a cache get 15-35 us, each including a loopback round trip of 12-32 us. Over a
network, add the round trips: the client's to the server, and the proxy's to
its backend. The README rows used to be guesses (50 us per proxy, 500 us per
query). A query that reads many rows, sorts or joins costs more, and one that
misses the database's cache waits for its disk.

## Compression rows

```bash
cargo bench --bench cpu_compress -- silesia.tar dickens ...
NAPKIN_COMPRESSION_CODECS="lz4_flex,zstd -3" cargo bench --bench cpu_compress -- file
```

`benches/cpu_compress.rs` reports the ratio and single-threaded
compress/decompress speed of lz4, snappy, zstd (-1, -3, -9, -19), gzip (-1, -6)
and brotli (-5, -11) for each file, as best of N runs over at least a second.
The CPU Compress/Decompress rows use the
[Silesia corpus](https://sun.aei.polsl.pl/~sdeor/index.php?page=silesia). On
November 4, 2026, compress / decompress in MiB/s, on C4 (Xeon 8581C), m8i and
m8g:

| Silesia | Ratio | C4 | m8i | m8g |
| --- | --- | --- | --- | --- |
| lz4 | 2.10 | 531 / 1518 | 519 / 1574 | 528 / 1631 |
| snappy | 2.07 | 566 / 1080 | 566 / 1105 | 564 / 1105 |
| zstd -1 | 2.90 | 480 / 995 | 474 / 1026 | 440 / 919 |
| zstd -9 | 3.59 | 71 / 940 | 68 / 943 | 50 / 830 |
| zstd -19 | 4.01 | 4 / 866 | 3 / 858 | 2 / 697 |

The ratio table, lz4 to zstd -19 (brotli -11 in parentheses where it beats
both), on the same day:

| What | File | Ratio |
| --- | --- | --- |
| HTML | two Wikipedia articles (1.5 MiB) | 3.9-8.0x (8.9) |
| English | Silesia `dickens` | 1.6-3.6x |
| Source Code | Silesia `samba` / the first 64 MB of a tarball of the cargo registry's sources | 2.8-5.5x / 4.0-9.8x |
| Executables | Silesia `mozilla` / the first 64 MB of a tarball of macOS's `/usr/bin` | 1.9-3.4x / 2.2-5.6x |
| RPC (JSON) | 100 issues from GitHub's API (0.7 MiB) | 5.6-12.9x (14.5) |
| Logs | loghub's full `Apache.log` (5 MiB) / `Linux.log` (2 MiB) | 11.3-28.9x / 6.3-19.6x |

The structured-data tables and the two CPU Decode rows come from
`benches/compression_ratio.rs`, built with `--features encodings` so the default
build doesn't pull in Parquet:

```bash
cargo bench --features encodings --bench compression_ratio -- parquet taxi.parquet hits_0.parquet
cargo bench --features encodings --bench compression_ratio -- cardinality taxi.parquet hits_0.parquet
cargo bench --features encodings --bench compression_ratio -- postings --tsv --details collection.tsv
cargo bench --features encodings --bench compression_ratio -- timeseries nab/*.csv
```

On October 24, 2026 (ratios on any machine; speeds single-threaded on an AWS
m8id.12xlarge, Xeon 6975P-C, reported against the decoded output size; the
November 4 runs on C4, m8i and m8g follow each item):

- Parquet, against Arrow's in-memory buffers: NYC taxi (yellow, January 2024,
  3M rows x 19 columns, mostly numeric) 5.4x with dictionary + RLE and 9.0x
  with zstd -3; ClickBench `hits_0.parquet` (1M rows x 105 columns, many
  strings) 2.2x and 10.4x. Plain encoding is 1.0x. Decoding to Arrow: plain
  5.4 / 3.0 GiB/s, dictionary + RLE 4.2 / 2.6 GiB/s, + zstd 2.1 / 1.7 GiB/s
  (taxi / hits). On November 4, dictionary + RLE + zstd decoded at 2.17 /
  2.09 / 1.85 GiB/s on taxi and 1.83 / 1.77 / 1.46 on hits (C4 / m8i / m8g),
  and encoded at 0.44 / 0.42 / 0.29 and 0.18 / 0.17 / 0.24 GiB/s: the README's
  2 GiB/s and 300 MiB/s.
- Search indexes over 53 datasets in four kinds, as index size / text size.
  `benches/index_datasets.sh` downloads and prepares them (~15 GB of
  downloads, 40 GB on disk, skipping whatever is already there), and
  `index_sizes` builds every index on every dataset, four datasets at a time
  with each index on its own thread, and prints the two tables below. On an
  18-core M-series Mac the whole run takes about 3.5 minutes:

  ```bash
  benches/index_datasets.sh ~/index-datasets
  cargo bench --features encodings --bench compression_ratio -- index_sizes ~/index-datasets/manifest.tsv
  ```

  Prose, logs and relational columns are one document per line (MS MARCO's
  passage text, a log line, a non-empty value), or per paragraph for books.
  Code is one document per source file: every text file in a shallow clone
  at the tag, skipping `.git`, files with a NUL byte and files over 350 KiB,
  as GitHub code search does.

  Posting lists are in Tantivy's layout: full 128-value blocks bitpacked with
  `BitPacker4x`, doc ID gaps minus one (`compress_strictly_sorted`) and
  frequencies minus one, the rest of each list as varints, and a skip entry
  per full block (last doc ID and bit width, 5 bytes; 8 with frequencies; 12
  with positions). Positions are delta-coded within each document. That
  comes to ~1 byte per doc ID, ~0.1-0.9 per frequency and ~0.5-1.5 per
  position. The term dictionary is uncompressed (term bytes, a length byte
  and a 4-byte offset per term); an FST would make it smaller.

  Inverted indexes use lowercased alphanumeric terms. The regex indexes are
  in the style of Google Code Search: every byte trigram or fourgram
  (ASCII-lowercased), with no frequencies or positions, plus n bytes and a
  4-byte offset per n-gram. Sparse n-grams are GitHub code search's, from its
  `sparse-ngrams` crate over `casefold`-folded text: every bigram, plus each
  3-8 byte substring whose end bigrams are rarer than every bigram between
  them, as opaque 32-bit keys (4 bytes plus a 4-byte offset each). The
  n-gram indexes are per source file for code and per ~16 KiB chunk of
  consecutive documents otherwise.

  The last three columns are per 1 MiB block of consecutive documents (of
  files, for code): the block compressed with zstd -3, and bloom filters
  over its distinct tokens and trigrams at 1% false positives (9.6 bits per
  key, uncompressed: a filter at its optimal fill is half ones and doesn't
  compress). A query checks every block's filter and decompresses and scans
  the blocks that match. At 1 MiB a block repeats most of its tokens and
  trigrams; at 16 KiB the filters are 3-5x bigger, and trigram filters are
  bigger than the exact trigram index.

  | Dataset | Kind | Docs | Text | Inverted | + positions | 3-grams | 4-grams | Sparse | zstd -3 | Token bloom | Trigram bloom |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | MS MARCO passages | Prose | 8.8M | 3.1 GB | 0.19 | 0.35 | 0.08 | 0.23 | 0.41 | 0.321 | 0.018 | 0.018 |
  | Amazon reviews (Video Games) | Prose | 4.6M | 1.4 GB | 0.19 | 0.42 | 0.07 | 0.21 | 0.40 | 0.361 | 0.012 | 0.015 |
  | C4 (web pages, one shard) | Prose | 356k | 774 MB | 0.15 | 0.39 | 0.07 | 0.23 | 0.46 | 0.380 | 0.021 | 0.019 |
  | enwik8 (Wikipedia XML) | Prose | 1.1M | 100 MB | 0.29 | 0.44 | 0.14 | 0.38 | 0.74 | 0.360 | 0.023 | 0.031 |
  | Silesia `webster` | Prose | 218k | 41 MB | 0.24 | 0.42 | 0.09 | 0.27 | 0.57 | 0.294 | 0.019 | 0.015 |
  | PG-19 books (test split) | Prose | 139k | 40 MB | 0.24 | 0.44 | 0.09 | 0.31 | 0.66 | 0.368 | 0.015 | 0.013 |
  | Silesia `dickens` | Prose | 33k | 10 MB | 0.25 | 0.44 | 0.09 | 0.32 | 0.74 | 0.362 | 0.013 | 0.010 |
  | LLVM 19.1.0 (C++) | Code | 144k | 1.2 GB | 0.04 | 0.23 | 0.08 | 0.15 | 0.28 | 0.121 | 0.004 | 0.012 |
  | Linux v6.12 (C) | Code | 86k | 988 MB | 0.05 | 0.28 | 0.11 | 0.24 | 0.44 | 0.204 | 0.006 | 0.020 |
  | .NET runtime 9.0.0 (C#) | Code | 59k | 644 MB | 0.05 | 0.19 | 0.06 | 0.13 | 0.24 | 0.102 | 0.004 | 0.011 |
  | Node v23.0.0 (C++, JS) | Code | 41k | 338 MB | 0.07 | 0.26 | 0.12 | 0.26 | 0.45 | 0.178 | 0.007 | 0.022 |
  | TensorFlow 2.18.0 (C++, Python) | Code | 33k | 247 MB | 0.06 | 0.23 | 0.12 | 0.24 | 0.41 | 0.163 | 0.005 | 0.018 |
  | Elasticsearch 8.16.0 (Java) | Code | 31k | 218 MB | 0.07 | 0.21 | 0.12 | 0.26 | 0.42 | 0.154 | 0.006 | 0.017 |
  | Kubernetes v1.32.0 (Go) | Code | 26k | 171 MB | 0.08 | 0.24 | 0.12 | 0.26 | 0.46 | 0.148 | 0.006 | 0.018 |
  | Swift 6.0 (C++, Swift) | Code | 26k | 131 MB | 0.08 | 0.24 | 0.12 | 0.25 | 0.45 | 0.183 | 0.007 | 0.019 |
  | Rust 1.84.0 (Rust) | Code | 50k | 121 MB | 0.07 | 0.23 | 0.19 | 0.35 | 0.62 | 0.192 | 0.005 | 0.021 |
  | Postgres 17.0 (C) | Code | 7k | 103 MB | 0.10 | 0.30 | 0.10 | 0.25 | 0.48 | 0.207 | 0.011 | 0.021 |
  | Symfony v7.2.0 (PHP) | Code | 13k | 80 MB | 0.12 | 0.23 | 0.13 | 0.27 | 0.45 | 0.148 | 0.009 | 0.018 |
  | CPython v3.13.0 (C, Python) | Code | 5k | 80 MB | 0.09 | 0.28 | 0.11 | 0.29 | 0.52 | 0.218 | 0.010 | 0.024 |
  | Go 1.23.0 (Go) | Code | 13k | 78 MB | 0.12 | 0.32 | 0.16 | 0.40 | 0.72 | 0.204 | 0.010 | 0.026 |
  | VS Code 1.96.0 (TypeScript) | Code | 8k | 77 MB | 0.08 | 0.22 | 0.11 | 0.28 | 0.46 | 0.172 | 0.007 | 0.020 |
  | Spring Framework v6.2.0 (Java) | Code | 11k | 56 MB | 0.09 | 0.23 | 0.15 | 0.29 | 0.50 | 0.159 | 0.006 | 0.015 |
  | Django 5.1 (Python) | Code | 5k | 34 MB | 0.10 | 0.28 | 0.17 | 0.39 | 0.64 | 0.201 | 0.009 | 0.024 |
  | React 19.0.0 (JS) | Code | 7k | 30 MB | 0.08 | 0.20 | 0.16 | 0.36 | 0.56 | 0.155 | 0.005 | 0.023 |
  | Rails 8.0.0 (Ruby) | Code | 5k | 21 MB | 0.08 | 0.26 | 0.19 | 0.42 | 0.70 | 0.209 | 0.006 | 0.025 |
  | loghub `Spark` | Logs | 33.2M | 2.9 GB | 0.12 | 0.22 | 0.02 | 0.05 | 0.07 | 0.065 | 0.005 | 0.003 |
  | loghub `HDFS` | Logs | 11.2M | 1.6 GB | 0.12 | 0.20 | 0.03 | 0.09 | 0.12 | 0.099 | 0.005 | 0.002 |
  | loghub `Thunderbird` (first GiB) | Logs | 6.0M | 1.1 GB | 0.14 | 0.26 | 0.03 | 0.05 | 0.09 | 0.051 | 0.003 | 0.005 |
  | loghub `BGL` | Logs | 4.7M | 743 MB | 0.14 | 0.28 | 0.03 | 0.06 | 0.11 | 0.092 | 0.012 | 0.003 |
  | NASA HTTP access, Jul-Aug 1995 | Logs | 3.5M | 373 MB | 0.15 | 0.27 | 0.05 | 0.09 | 0.14 | 0.105 | 0.003 | 0.008 |
  | loghub `Android` | Logs | 1.6M | 192 MB | 0.19 | 0.29 | 0.10 | 0.19 | 0.30 | 0.093 | 0.006 | 0.015 |
  | loghub `SSH` | Logs | 655k | 73 MB | 0.13 | 0.23 | 0.02 | 0.04 | 0.07 | 0.067 | 0.005 | 0.003 |
  | loghub `OpenStack` | Logs | 208k | 61 MB | 0.19 | 0.31 | 0.06 | 0.13 | 0.28 | 0.081 | 0.012 | 0.008 |
  | loghub `Hadoop` | Logs | 394k | 49 MB | 0.13 | 0.22 | 0.05 | 0.08 | 0.13 | 0.042 | 0.003 | 0.005 |
  | loghub `HPC` | Logs | 433k | 34 MB | 0.37 | 0.47 | 0.02 | 0.08 | 0.15 | 0.099 | 0.025 | 0.002 |
  | loghub `HealthApp` | Logs | 253k | 24 MB | 0.22 | 0.29 | 0.04 | 0.09 | 0.23 | 0.098 | 0.010 | 0.004 |
  | loghub `Mac` | Logs | 117k | 17 MB | 0.20 | 0.30 | 0.11 | 0.19 | 0.32 | 0.067 | 0.005 | 0.012 |
  | loghub `Zookeeper` | Logs | 74k | 10 MB | 0.11 | 0.20 | 0.03 | 0.06 | 0.11 | 0.043 | 0.003 | 0.003 |
  | loghub `Apache` | Logs | 56k | 5 MB | 0.19 | 0.30 | 0.04 | 0.07 | 0.11 | 0.053 | 0.002 | 0.003 |
  | loghub `Proxifier` | Logs | 21k | 3 MB | 0.17 | 0.30 | 0.06 | 0.12 | 0.18 | 0.073 | 0.005 | 0.005 |
  | loghub `Linux` | Logs | 26k | 2 MB | 0.27 | 0.40 | 0.10 | 0.21 | 0.36 | 0.084 | 0.011 | 0.010 |
  | IMDB `title.akas` titles | Relational Columns | 60.0M | 1.3 GB | 0.27 | 0.33 | 0.08 | 0.18 | 0.31 | 0.171 | 0.015 | 0.017 |
  | DBLP paper titles | Relational Columns | 13.1M | 764 MB | 0.21 | 0.30 | 0.07 | 0.22 | 0.38 | 0.301 | 0.014 | 0.013 |
  | DBLP authors | Relational Columns | 34.9M | 546 MB | 0.38 | 0.45 | 0.11 | 0.39 | 0.64 | 0.400 | 0.043 | 0.012 |
  | IMDB `primaryTitle` | Relational Columns | 12.9M | 273 MB | 0.38 | 0.47 | 0.12 | 0.38 | 0.71 | 0.385 | 0.036 | 0.021 |
  | IMDB `primaryName` | Relational Columns | 15.8M | 230 MB | 0.50 | 0.58 | 0.10 | 0.41 | 0.83 | 0.479 | 0.061 | 0.015 |
  | ClickBench `Title` | Relational Columns | 936k | 139 MB | 0.13 | 0.19 | 0.04 | 0.07 | 0.11 | 0.077 | 0.007 | 0.008 |
  | Amazon review titles (Video Games) | Relational Columns | 4.6M | 111 MB | 0.27 | 0.38 | 0.08 | 0.25 | 0.47 | 0.364 | 0.011 | 0.013 |
  | Open Food Facts `product_name` | Relational Columns | 4.5M | 105 MB | 0.37 | 0.45 | 0.14 | 0.43 | 0.77 | 0.403 | 0.028 | 0.023 |
  | ClickBench `URL` | Relational Columns | 1000k | 90 MB | 0.25 | 0.36 | 0.11 | 0.23 | 0.37 | 0.146 | 0.013 | 0.024 |
  | ClickBench `Referer` | Relational Columns | 921k | 81 MB | 0.28 | 0.39 | 0.12 | 0.27 | 0.41 | 0.151 | 0.013 | 0.032 |
  | Amazon product titles (Video Games) | Relational Columns | 137k | 12 MB | 0.28 | 0.40 | 0.15 | 0.46 | 0.81 | 0.334 | 0.015 | 0.024 |
  | ClickBench `SearchPhrase` | Relational Columns | 69k | 4 MB | 0.31 | 0.37 | 0.06 | 0.15 | 0.24 | 0.136 | 0.013 | 0.007 |

  Medians per kind (the README rounds them):

  | Kind | Inverted | + positions | 3-grams | 4-grams | Sparse | zstd -3 | Token bloom | Trigram bloom |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | Prose (7) | 0.24 | 0.42 | 0.09 | 0.27 | 0.57 | 0.361 | 0.018 | 0.015 |
  | Code (18) | 0.08 | 0.24 | 0.12 | 0.27 | 0.46 | 0.175 | 0.006 | 0.020 |
  | Logs (16) | 0.16 | 0.28 | 0.04 | 0.09 | 0.14 | 0.077 | 0.005 | 0.005 |
  | Relational Columns (12) | 0.28 | 0.39 | 0.11 | 0.26 | 0.44 | 0.318 | 0.014 | 0.016 |

  Identifiers repeat within and across files, so code's inverted index is a
  third of prose's. Logs repeat line after line, so their n-gram indexes are
  a third of prose's or smaller. Short, distinct values (names, product
  names) are the worst case for relational columns, and compress worst with
  zstd too. Sparse n-grams store 2-3x as many postings as trigrams so that a
  query reads a few selective lists instead of many common ones. Russ Cox
  reported ~0.2 for Code Search's trigram index.

  With `postings --details`, MS MARCO's doc IDs get 3.0x as delta + varint
  and 2.1x as roaring bitmaps. Logs
  and ClickBench rows repeat a lot, so their doc IDs cluster and most
  frequencies are 1. Unpacking MS MARCO's full 128-ID blocks with
  `bitpacking`'s `BitPacker4x` ran at 3.1 billion IDs/s, 11.6 GiB/s of u32
  output. Packing them ran at 8.8 GiB/s of u32 input, against 12.2 GiB/s
  for unpacking on the same October 26 run; the README's bitpacking rows now
  come from `cpu_bitpack`. Writing
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
  ~6 GiB/s. On November 4, all integer columns decoded at 16.6 / 15.3 / 9.6
  GiB/s on taxi and 16.4 / 15.3 / 13.9 on hits (C4 / m8i / m8g), and taxi's
  floats at 13.0 / 15.5 / 10.6.
- Strings: FSST with one symbol table per 64 Ki strings, compressed lengths
  bitpacked so every string decodes on its own, against the bytes plus a
  4-byte offset each. ClickBench `Title` 2.06x, `URL` 1.97x, `Referer` 1.86x,
  `OriginalURL` 2.33x; loghub `Apache.log` 2.9x and `Linux.log` 2.2x, one
  string per line; enwik8 lines 1.71x; MS MARCO passages 1.79x. zstd -3 over
  each whole column gets 3-11x. Decode 2-4 GiB/s.

## Cost rows

The shape prices are AWS us-east-1 Linux list prices on November 4, 2026, per
vCPU per month (730 h), from `aws pricing get-products` (on-demand),
`aws savingsplans describe-savings-plans-offering-rates` (1-year Compute
Savings Plan, no upfront; the newest families have no reserved instances) and
`aws ec2 describe-spot-price-history` (median of the latest price per zone):

| Shape | Instance | On-demand | 1-year | Spot |
| --- | --- | --- | --- | --- |
| CPU | c8i.12xlarge | \$34 | \$25 | \$12 |
| General | m8i.12xlarge | \$39 | \$28 | \$14 |
| General, SSD | m8id.12xlarge | \$48 | \$35 | \$13 |
| Memory | r8i.12xlarge | \$51 | \$37 | \$15 |
| Memory, High | x8g.12xlarge | \$71 | \$53 | \$38 |
| Memory, Ultra | x2iedn.8xlarge | \$152 | \$112 | \$47 |
| SSD | i8g.12xlarge | \$63 | \$49 | \$17 |
| SSD, High | i8ge.12xlarge | \$87 | \$68 | \$28 |
| GPU (B200) | p6-b200.48xlarge | \$433 | \$341 | ~\$250 |

B200 spot is from us-west-2 and us-east-2 (\$43-86/h per instance; us-east-1
had one zone at \$167/h). The unit rows come from the differences: the SSD in
`m8id` over `m8i` is \$0.15 / \$0.11 per GB, and memory in `r8i` over `m8i` is
\$3 / \$2 per GiB. Graviton (`c8g`, `m8g`, `r8g`) is ~15% cheaper on-demand.

## Blob storage rows (March 2026)

The blob storage benchmark code isn't in this repository yet; this describes
the setup behind the README's blob rows. A latency probe ran from a VM in the
same region with 128 requests in flight on warm connections, from March 10 to
20, 2026. It swept GETs, PUTs and conditional PUTs over 8 KiB to 8 MiB for 300
seconds per row, listed one random 1000-key page per sample of a 1M-key
namespace, and read random 128 KiB- and 8 MiB-aligned ranges of 128 objects of
1 GiB. The regional tiers and the zonal tiers S3 Express One Zone and GCS Rapid
(gRPC, reads through a plain and an open object handle) gave p99 (p50), median
over runs:

| | S3 | GCS gRPC | Azure | S3 Express One Zone | GCS Rapid |
| --- | --- | --- | --- | --- | --- |
| Read 128 KiB | 77 (58) ms | 56 (16) | 97 (66) | 60 (50) | 26-27 (12-13) |
| Write 128 KiB | 154 (64) | 373 (66) | 38 (22) | 104 (51) | 164-333 (80-85) |
| Conditional write (if-match) 128 KiB | - | 105 (63) | 20 (8) | 33 (6) | 93-97 (64-67) |
| Conditional write (if-none-match) 128 KiB | 124 (46) | 153 (60) | 330 (23) | 484 (35) | 149-469 (72-75) |
| List 1000 keys | 131 (78) | 87 (42) | 174 (88) | 27 (23) | 152-189 (69) |

The README rows average the providers' p99s and round. Zonal buckets win on
reads (50 ms against 100 ms) and S3 Express on lists (27 ms), but zonal writes
are no faster than regional ones: both average ~180-190 ms, hence 200 ms. The
Conditional Write rows (100 ms) are if-match, a compare-and-swap on one object,
one at a time: S3 Express 12-54 ms p99 (6 ms p50), GCS Rapid 93-97 ms (64-67
ms), GCS 105 ms and Azure 20 ms; S3 Standard has no if-match, and its
if-none-match took 124 ms. The zonal row used to be 500 ms, from if-none-match
at 128 creates in flight, where S3 Express is 35 ms p50 but 484 ms p99 (no
retries; every sample took one attempt). Two GCS Rapid if-match runs that
pushed ~430 writes/s at one object had p99s of 7.6-8.4 s (max 80 s); one at a
time it took 14 writes/s with no errors. One run had 31% of its writes fail
their precondition: several writers on one object. Both zonal tiers cost about
\$0.11 per GB-month, ~5x regional.

p50 for an 8 KiB range read was 56 ms on S3, 104 on GCS's XML API, 35 on its
gRPC API and 45 on Azure; for an 8 KiB PUT 49 / 141 / 71 / 32 ms. GCS Rapid
read 8 KiB in 6-14 ms. The Read and Write rows use 128 KiB, the largest size
whose p99 is still about what 8 KiB costs: S3's read p99 stays at 80-90 ms up
to 128 KiB and reaches 110-125 ms at 256 KiB, Azure's 75-115 ms and then
115-125 ms. GCS gRPC stays near 100 ms up to 2 MiB, while GCS XML is noisy at
130-390 ms. With 128 requests in flight, a lone request may stay flat to a
larger size. For PUTs, p99 at 8 / 128 / 256 KiB was 100 / 140 / 186 ms on S3,
51 / 38 / 114 ms on Azure and 271 / 229 / 338 ms on GCS XML.

Throughput came from a Criterion group that used the AWS S3 SDK for both S3
and GCS (through GCS's XML API), on an m6id.12xlarge in `us-west-2a` and a
c4-standard-48-lssd in `us-central1-c` on March 9, 2026. One 1 GiB GET on one
connection ran at 95 MiB/s on S3 and 190-200 MiB/s on GCS: the README's
1-connection read row (100 MiB/s). AWS says to budget 85-90 MB/s per
connection. It used to say 500 MiB/s, which reproduced on neither. With 32-128
connections, GETs peaked at the hosts' NICs: 2.0 GiB/s on S3, 4.3-4.9 on GCS
and 3.9 on Azure, and PUTs (multipart, 32 MiB parts) at 1.9, 3.1-3.3 and 2.2
GiB/s.

## GPU rows

The GPU rows are NVIDIA's B200 (HGX, SXM) specs, not measured here, rounded
down to what kernels reach in practice:

- GPU Memory Sequential Read: HBM3e at 8 TB/s; copy kernels reach ~90%.
- GPU ↔ GPU Transfer, NVLink: NVLink 5 at 1.8 TB/s, 900 GB/s per direction.
- GPU ↔ CPU Transfer, PCIe: PCIe 5.0 x16 at 64 GB/s per direction; pinned
  `cudaMemcpy` reaches ~55 GB/s, and a small copy costs ~10 μs.
- GPU ↔ GPU Network, RDMA: one 400 Gbit/s InfiniBand or RoCE NIC per GPU, to
  a GPU in another machine, ~5 μs round trip.

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
