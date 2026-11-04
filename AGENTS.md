# Repository Notes

`napkin-math` is a table of rough performance and cost numbers
([README.md](README.md)) plus the benchmarks behind them.
[BENCHMARKING.md](BENCHMARKING.md) says which benchmark backs which README row
and how to rerun it. Read it before touching either file.

Private operational notes (hosts, buckets, accounts) live in `AGENTS.local.md`,
which is gitignored. This repository is public: never commit account IDs,
IPs, bucket names or credentials.

## Layout

- `benches/napkin_math.rs` registers the Criterion groups in `benches/benchmarks/`.
- Every other `benches/*.rs` is a standalone tool with `bench = false`: plain `cargo bench` skips it, `cargo bench --bench storage -- <args>` runs it, and without arguments it prints its usage and exits.
- `benches/cpu_compress.rs` measures compression ratio and speed on real files.
- `benches/compression_ratio.rs` (`--features encodings`) measures Parquet, posting-list and Gorilla ratios.
- `benches/index_datasets.sh <dir>` downloads the 53 search-index datasets and writes `<dir>/manifest.tsv`; `compression_ratio -- index_sizes <dir>/manifest.tsv` rebuilds the BENCHMARKING index tables from it in a few minutes.
- `benches/compression_ratio_columns.rs` (`--features encodings`) measures ALP, FastLanes FOR/delta and FSST per column, with round-trip checks and decode speed.
- `benches/storage_durable_write.rs` (no dependencies) measures pwrite + fdatasync commits, with flush counts from `/proc/diskstats`.
- `run` builds, then runs `cargo bench`; with `NAPKIN_TUNE=1` it calls `sudo` itself, so don't wrap it in `sudo`.
- `benches/storage.rs` (disk rows) and `benches/network.rs` (network rows, `--server` on one host) have no dependencies.
- `newsletter/` is unrelated to the README benchmarks.

## Running

```bash
./run --bench napkin_math                        # everything
./run --bench napkin_math -- 'syscall|hash'      # a subset (regex)
make test                                        # each benchmark once, smoke test
```

- Benchmark on a current-generation cloud VM, not a laptop. A minimal Ubuntu
  VM needs `build-essential pkg-config` and rustup.
- Criterion filters are substrings/regexes: `memory_random` also matches
  `memory_random_read_latency`, and `cache_read` matches `cache_read_latency`.
- `./run` adds `-C target-cpu=native` to `RUSTFLAGS`. `NAPKIN_TUNE=1` (turbo off,
  `performance` governor, ASLR off) is for A/B runs on one host, not README numbers.
- Check `df -h` before disk benchmarks; `storage` needs 2x the test size.
- For a fresh VM, use `script/bench-vm` (see BENCHMARKING.md): it deletes the VM
  when done and by a hard deadline. Run `script/bench-vm sweep` at the end of a
  session. Its account settings live in the gitignored `.bench-vm.env`.

## Gotchas found the hard way

- Benchmark loops must `black_box` their inputs, or LLVM hoists pure calls out
  of the loop (the 64 B hash bench once reported xxh3 at 0.26 ns).
- `memory_random` issues independent loads, so it measures memory-level
  parallelism (throughput). Latency comes from the `memory_random_read_latency` pointer chase.
- `getrusage(RUSAGE_SELF)` gets slower with more threads in the process, so it
  is not a clean syscall number inside Criterion. Use `getppid`.
- Reads of a file you just wrote come from page cache unless you drop it, and
  reads of never-written blocks can skip the flash. `storage`
  preconditions and drops caches for this reason.
- AWS caps a single TCP flow at 5 Gbit/s outside a placement group; use 8+
  flows to see the VM limit. Use TCP round trips (`network`), not ping, for RTT.
- Graviton crc32/blake3 numbers are crate-specific (no ARM CRC or NEON paths in
  this build), not hardware limits. sha256 got 4x faster there with `sha2` 0.11
  (1.4 GiB/s), and ring's AES-GCM runs at 4 GiB/s against 10 on x86.
- The bench binary uses jemalloc; the random-memory numbers depend on it.
- One 64-byte AVX-512 load per cache line never triggers Intel's L1 streaming
  prefetcher, so DRAM-sized AVX-512 reads need software prefetch (1 KiB ahead
  to L1 and 16 KiB to L2) to match AVX2.
- Measure SSDs across all of a VM's local disks (RAID0), not one: GCP caps each
  375 GiB disk at 625 / 330 MiB/s, so one disk understates the VM ~8x. Compare
  against the provider's published limits.
- Match local disks by model (`nvme_card[0-9]` on GCP, "Instance Storage" on
  AWS); GCP's boot disk is `nvme_card-pd`. If `/dev/md0` doesn't exist, a
  benchmark that opens it creates a file there in RAM and reports ~60 GiB/s.
- `storage` and `storage_durable_write` reach the published SSD limits from one
  thread (two for small random reads) via io_uring. `fio` is only a cross-check
  for a new kind of disk; nothing depends on it.
- x86 memory writes need non-temporal stores to reach speed of light; a plain
  memset reads every line first and gets half.
- nginx closes a keep-alive connection after 1000 requests unless
  `keepalive_requests` is raised; `benches/services.sh` raises it.
- C4 capacity in `us-central1` ran out on November 4, 2026; `script/bench-vm gcp
  ... --zone us-east4-a` worked. C4 VMs come as Xeon 8581C or 6985P-C
  depending on the zone, so record `lscpu`.

## README policy

- Round for memorability: `200 MiB/s`, `20 GiB/s`, `50 ns`, `30 ms`, never
  `173 MiB/s`. Near a boundary, pick the simple number that keeps the order of
  magnitude honest.
- Row names are `[Device] [Access pattern] [Operation]`, with `, Zonal` /
  `, Regional` after the device where both exist. Rows sort by throughput, then
  by latency, lowest first.
- Throughput is the most you can get (many requests in flight).
- Keep prose terse and unhedged. Methodology goes in BENCHMARKING.md, not the README.
- Show README changes for review before pushing.
