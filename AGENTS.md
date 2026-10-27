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
- `src/bin/compression_corpus.rs` measures compression ratio and speed on real files.
- `src/bin/encoding_ratio.rs` (`--features encodings`) measures Parquet, posting-list and Gorilla ratios.
- `src/bin/column_codecs.rs` (`--features encodings`) measures ALP, FastLanes FOR/delta and FSST per column, with round-trip checks and decode speed.
- `src/bin/durable_write.rs` (no dependencies) measures pwrite + fdatasync commits, with flush counts from `/proc/diskstats`.
- `run` applies Linux host tuning, then runs `cargo bench`. It calls `sudo` itself; don't wrap it in `sudo`.
- `src/bin/disk.rs` (disk rows) and `src/bin/net.rs` (network rows, `--server` on one host) have no dependencies.
- `newsletter/` is unrelated to the README benchmarks.

## Running

```bash
./run --bench napkin_math                        # everything, tuned (Linux)
./run --bench napkin_math -- 'syscall|hash'      # a subset (regex)
make test                                        # each benchmark once, smoke test
```

- Benchmark on a current-generation cloud VM, not a laptop. A minimal Ubuntu
  VM needs `build-essential pkg-config` and rustup.
- Criterion filters are substrings/regexes: `memory_read` also matches
  `compressed_memory_read`.
- Without host tuning (EC2, ARM, most VMs lack some knobs), pass
  `RUSTFLAGS='-C target-cpu=native'` to `cargo bench` yourself.
- Check `df -h` before disk benchmarks; `disk` needs 2x the test size.

## Gotchas found the hard way

- Benchmark loops must `black_box` their inputs, or LLVM hoists pure calls out
  of the loop (the 64 B hash bench once reported xxh3 at 0.26 ns).
- `random_memory` issues independent loads, so it measures memory-level
  parallelism (throughput). Latency comes from the `memory_latency` pointer chase.
- `getrusage(RUSAGE_SELF)` gets slower with more threads in the process, so it
  is not a clean syscall number inside Criterion. Use `getppid`.
- Reads of a file you just wrote come from page cache unless you drop it, and
  reads of never-written blocks can skip the flash. `disk`
  preconditions and drops caches for this reason.
- AWS caps a single TCP flow at 5 Gbit/s outside a placement group; use 8+
  flows to see the VM limit. Use TCP round trips (`net`), not ping, for RTT.
- Graviton sha256/crc32/blake3 numbers are crate-specific (no ARM crypto
  instructions in this build), not hardware limits.
- The bench binary uses jemalloc; the random-memory numbers depend on it.

## README policy

- Round for memorability: `200 MiB/s`, `20 GiB/s`, `50 ns`, `30 ms`, never
  `173 MiB/s`. Near a boundary, pick the simple number that keeps the order of
  magnitude honest.
- Row names are `[Device] [Access pattern] [Operation]`, grouped by device.
- Throughput is the most you can get (many requests in flight). The 1 GiB
  column is 1 GiB divided by throughput; keep it consistent.
- Keep prose terse and unhedged. Methodology goes in BENCHMARKING.md, not the README.
- Show README changes for review before pushing.
