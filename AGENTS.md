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
- Graviton sha256/crc32/blake3 numbers are crate-specific (no ARM crypto
  instructions in this build), not hardware limits.
- The bench binary uses jemalloc; the random-memory numbers depend on it.

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
