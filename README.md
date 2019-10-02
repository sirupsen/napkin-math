# Base Rates

This project contains a series of programs to evaluate various 'base rates' that
are useful to know when designing systems. For example, how fast is reading 1
GiB of memory?

Base rates are useful for determining the expected performance of a system.

Run with `cargo run --release`. You won't get the right numbers when you're
compiling in debug mode.

## TODO

Lots more to add, feel free to help, for example... 

* Compression ratios (snappy, gzip, ..)
* NUMA impact
* Mutexes
* HTTP overhead
* JSON
* MYSQL/POSTGRES numbers
* Hashing
* Real-life Nginx/Envoy overhead
* Read 64 byte cache-lines instead of 64-bit numbers

## 2017 Macbook Pro, 2.8 GHz Intel Core i7

```
[Read Seq Vec<usize>] Iterations in 172 miliseconds, no overhead: 134,217,727
[Read Seq Vec<usize>] Iterations / second: 780,335,622
[Read Seq Vec<usize>] Bytes handled per iteration: 8 bytes
[Read Seq Vec<usize>] Total bytes processed: 1024.000 MiB
[Read Seq Vec<usize>] Throughput: 5.814 GiB/s
[Read Seq Vec<usize>] Avg single iteration: 1.288 ns
[Read Seq Vec<usize>] Avg single iteration cycles: 3.62
[Read Seq Vec<usize>] Time to process 1 MiB: 168 μs
[Read Seq Vec<usize>] Time to process 1 GiB: 172 ms
[Read Seq Vec<usize>] Time to process 1 TiB: 2.95 min

[Write Seq Vec<usize>] Iterations in 5000 miliseconds, no overhead: 906,353,262
[Write Seq Vec<usize>] Iterations / second: 181,270,652
[Write Seq Vec<usize>] Bytes handled per iteration: 8 bytes
[Write Seq Vec<usize>] Total bytes processed: 6.753 GiB
[Write Seq Vec<usize>] Throughput: 1.351 GiB/s
[Write Seq Vec<usize>] Avg single iteration: 5.517 ns
[Write Seq Vec<usize>] Avg single iteration cycles: 15.49
[Write Seq Vec<usize>] Time to process 1 MiB: 723 μs
[Write Seq Vec<usize>] Time to process 1 GiB: 740 ms
[Write Seq Vec<usize>] Time to process 1 TiB: 12.63 min

[Random Read Vec<usize>] Iterations in 3866 miliseconds, no overhead: 134,217,727
[Random Read Vec<usize>] Iterations / second: 34,717,466
[Random Read Vec<usize>] Bytes handled per iteration: 8 bytes
[Random Read Vec<usize>] Total bytes processed: 1024.000 MiB
[Random Read Vec<usize>] Throughput: 264.873 MiB/s
[Random Read Vec<usize>] Avg single iteration: 28 ns
[Random Read Vec<usize>] Avg single iteration cycles: 80.89
[Random Read Vec<usize>] Time to process 1 MiB: 3775 μs
[Random Read Vec<usize>] Time to process 1 GiB: 3.87 s
[Random Read Vec<usize>] Time to process 1 TiB: 1.10 hours

[Real: Random Write Vec<usize>] Iterations in 5006 miliseconds, no overhead: 417,998,964
[Real: Random Write Vec<usize>] Iterations / second: 83,499,593
[Real: Random Write Vec<usize>] Bytes handled per iteration: 8 bytes
[Real: Random Write Vec<usize>] Total bytes processed: 3.114 GiB
[Real: Random Write Vec<usize>] Throughput: 637.051 MiB/s
[Real: Random Write Vec<usize>] Avg single iteration: 11 ns
[Real: Random Write Vec<usize>] Avg single iteration cycles: 33.63
[Real: Random Write Vec<usize>] Time to process 1 MiB: 1569 μs
[Real: Random Write Vec<usize>] Time to process 1 GiB: 1607 ms
[Real: Random Write Vec<usize>] Time to process 1 TiB: 27.43 min

[Sequential Disk Read <8kb>] Iterations in 5016 miliseconds, no overhead: 2,467,080
[Sequential Disk Read <8kb>] Iterations / second: 491,842
[Sequential Disk Read <8kb>] Bytes handled per iteration: 8192 bytes
[Sequential Disk Read <8kb>] Total bytes processed: 18.822 GiB
[Sequential Disk Read <8kb>] Throughput: 3.752 GiB/s
[Sequential Disk Read <8kb>] Avg single iteration: 2 μs
[Sequential Disk Read <8kb>] Avg single iteration cycles: 5710.02
[Sequential Disk Read <8kb>] Time to process 1 MiB: 260 μs
[Sequential Disk Read <8kb>] Time to process 1 GiB: 266 ms
[Sequential Disk Read <8kb>] Time to process 1 TiB: 4.53 min


[Random Disk Seek, No Page Cache <64b>] Iterations in 5054 miliseconds, no overhead: 53,012
[Random Disk Seek, No Page Cache <64b>] Iterations / second: 10,489
[Random Disk Seek, No Page Cache <64b>] Bytes handled per iteration: 64 bytes
[Random Disk Seek, No Page Cache <64b>] Total bytes processed: 3.236 MiB
[Random Disk Seek, No Page Cache <64b>] Throughput: 655.569 KiB/s
[Random Disk Seek, No Page Cache <64b>] Avg single iteration: 95 μs
[Random Disk Seek, No Page Cache <64b>] Avg single iteration cycles: 267742.90
[Random Disk Seek, No Page Cache <64b>] Time to process 1 MiB: 1562 ms
[Random Disk Seek, No Page Cache <64b>] Time to process 1 GiB: 26.65 min
[Random Disk Seek, No Page Cache <64b>] Time to process 1 TiB: 455.03 hours


[Sequential Disk Write, No Fsync <16KiB>] Iterations in 5085 miliseconds, no overhead: 184,896
[Sequential Disk Write, No Fsync <16KiB>] Iterations / second: 36,361
[Sequential Disk Write, No Fsync <16KiB>] Bytes handled per iteration: 64 bytes
[Sequential Disk Write, No Fsync <16KiB>] Total bytes processed: 11.285 MiB
[Sequential Disk Write, No Fsync <16KiB>] Throughput: 2.219 MiB/s
[Sequential Disk Write, No Fsync <16KiB>] Avg single iteration: 27 μs
[Sequential Disk Write, No Fsync <16KiB>] Avg single iteration cycles: 77228.91
[Sequential Disk Write, No Fsync <16KiB>] Time to process 1 MiB: 450 ms
[Sequential Disk Write, No Fsync <16KiB>] Time to process 1 GiB: 7.68 min
[Sequential Disk Write, No Fsync <16KiB>] Time to process 1 TiB: 131.25 hours

[Sequential Disk Write, Fsync <16KiB>] Iterations in 5007 miliseconds, no overhead: 741
[Sequential Disk Write, Fsync <16KiB>] Iterations / second: 147
[Sequential Disk Write, Fsync <16KiB>] Bytes handled per iteration: 64 bytes
[Sequential Disk Write, Fsync <16KiB>] Total bytes processed: 46.312 KiB
[Sequential Disk Write, Fsync <16KiB>] Throughput: 9.249 KiB/s
[Sequential Disk Write, Fsync <16KiB>] Avg single iteration: 6 ms
[Sequential Disk Write, Fsync <16KiB>] Avg single iteration cycles: 18977534.88
[Sequential Disk Write, Fsync <16KiB>] Time to process 1 MiB: 110.73 s
[Sequential Disk Write, Fsync <16KiB>] Time to process 1 GiB: 31.50 hours
[Sequential Disk Write, Fsync <16KiB>] Time to process 1 TiB: 32252.25 hours

[Tcp Echo <64b>] Iterations in 5058 miliseconds, no overhead: 268,016
[Tcp Echo <64b>] Iterations / second: 52,988
[Tcp Echo <64b>] Bytes handled per iteration: 64 bytes
[Tcp Echo <64b>] Total bytes processed: 16.358 MiB
[Tcp Echo <64b>] Throughput: 3.234 MiB/s
[Tcp Echo <64b>] Avg single iteration: 18 μs
[Tcp Echo <64b>] Avg single iteration cycles: 52997.40
[Tcp Echo <64b>] Time to process 1 MiB: 309 ms
[Tcp Echo <64b>] Time to process 1 GiB: 5.27 min
[Tcp Echo <64b>] Time to process 1 TiB: 90.07 hours
```
