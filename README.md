# Napkin Math

The goal of this project is to collect software, numbers, and techniques to
quickly estimate the expected performance of systems from first-principles. For
example, how quickly can you read 1 GB of memory? By composing these resources
you should be able to answer interesting questions like: how much storage cost
should you expect to pay for a cloud application with 100,000 RPS?

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

Here are the numbers from the program, run on my 2017 Macbook. The goal is to
run this on more platform.

**Note 1:** Numbers have been rounded, which means they don't line up perfectly.
**Note 2:** Some throughput and latency numbers don't line up. Then the highest
throughput achieved (with e.g. another input size) has been noted.

| Operation                           | Latency | Throughput | 1 MiB  | 1 GiB  |
| ----------------------------------- | ------- | ---------- | ------ | ------ |
| Sequential Memory R/W (64 bytes)    | 5 ns    | 10 GiB/s   | 100 us | 100 ms |
| Hashing, not crypto-safe (64 bytes) | 25 ns   | 2 GiB/s    | 500 us | 500 ms |
| Random Memory R/W (64 bytes)        | 50 ns   | 1 GiB/s    | 1 ms   | 1 s    |
| System Call                         | 500 ns  | N/A        | N/A    | N/A    |
| Hashing, crypto-safe (64 bytes)     | 500 ns  | 200 MiB/s  | 10 ms  | 10s    |
| Sequential SSD Read (8 KiB)         | 1 μs    | 4 GiB/s    | 200 us | 200 ms |
| Context Switch `[1] [2]`            | 10 μs   | N/A        | N/A    | N/A    |
| Sequential SSD write, -fsync (8KiB) | 10 μs   | 1 GiB/s    | 1 ms   | 1 s    |
| TCP Echo (TCP overhead) (64 bytes)  | 10 μs   | ?          | ?      | ?      |
| Sorting (64-bit integers)           | N/A     | 200 MiB/s  | 5 ms   | 5 s    |
| Decompression `[3]`                 | N/A     | ?          | 5 ms   | 5s     |
| Random SSD Seek (8 KiB)             | 100 μs  | 70 MiB/s   | 10 ms  | 15 s   |
| Compression `[3]`                   | N/A     | ?          | 10 ms  | 10s    |
| Cloud us-east1 to us-east2          | 250 μs  | ?          | ?      | ?      |
| Sequential SSD write, +fsync (8KiB) | 5 ms    | 2 MiB/s    | 1 s    | 10 min |
| Mutex Lock/Unlock                   | ?       | ?          | ?      | ?      |
| {MySQL, Memcached, Redis, ..} Query | ?       | ?          | ?      | ?      |
| Envoy/Nginx Overhead                | ?       | ?          | ?      | ?      |
| {JSON, Protobuf, ..} Serializee (?) | ?       | ?          | ?      | ?      |
| Cloud us-east to us-central         | ?       | ?          | ?      | ?      |

You can run this with `RUSTFLAGS='-C target-cpu=native' cargo run --release --
-h`. You won't get the right numbers when you're compiling in debug mode. You
can help this project by adding new suites and filling out the blanks.

I am aware of some inefficiencies in this suite. I intend to improve my skills
in this area, in order to ensure the numbers are the upper-bound of performance
you may be able to squeeze out in production. I find it highly unlikely any of
them will be more than 2-3x off, which shouldn't be a problem for most users.

## Cost Numbers

Approximate numbers that should be consistent between Cloud providers.
  
| What        | Amount | $ / Month |
| ----------- | ------ | --------- |
| CPU         | 1      | $10       |
| Memory      | 1 GB   | $1        |
| SSD         | 1 GB   | $0.1      |
| Disk        | 1 GB   | $0.01     |
| S3, GCS, .. | 1 GB   | $0.01     |
| Network     | 1 GB   | $0.01     |

## Compression Ratios

This is sourced from a few sources. `[3]` `[4]` `[5]` Note that compression speeds (but
generally not ratios) vary by an order of magnitude depending on the algorithm
and the level of compression (which trades speed for compression).

I typically ballpark that another _x in compression ratio decreases performance
by 10x_. E.g. we can [get a 2x ratio on English
Wikipedia](https://quixdb.github.io/squash-benchmark/#results-table) at ~200
MiB/s, and 3x at ~20MiB/s, and 4x at 1MB/s.

| What        | Compression Ratio |
| ----------- | ----------------- |
| HTML        | 2-3x              |
| English     | 2-4x              |
| Source Code | 2-4x              |
| Executables | 2-3x              |
| RPC         | 5-10x             |

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
* ["How to get consistent results when benchamrking on
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
* [cargo-asm](https://github.com/gnzlbg/cargo-asm). Cargo extension to allow
  disassembling functions. Unfortunately the support for closure is a bit
  lacking, which requires some refactoring. It's also _very_ slow on even this
  simple program.
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
* [How Long Does It Takes To Make a Context Switch](https://blog.tsunanet.net/2010/11/how-long-does-it-take-to-make-context.html)
* [Integer Compression Comparisons](https://github.com/powturbo/TurboPFor-Integer-Compression)
