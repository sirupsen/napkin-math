# Napkin Math

The goal of this project is to collect software, numbers, and techniques to
quickly estimate the expected performance of systems from first-principles. For
example, how quickly can you read 1 GB of memory? By composing these resources
you should be able to answer interesting questions like: how much storage cost
should you expect to pay for a cloud application with 100,000 RPS?

The best introduction to this skill is [through my talk at
SRECON](https://www.youtube.com/watch?v=IxkSlnrRFqc).

The best way to practise napkin math in the grand domain of computers is to work
on your own problems. The second-best is to subscribe to [this
newsletter](http://sirupsen.com/napkin) where you'll get a problem every few
weeks to attack. It should only take you a few minutes to solve each one as your
facility with these techniques improve.

The archive of problems to practise with are
[here](https://tinyletter.com/computer-napkins/archive). The solution will be in
the following newsletter.

## Numbers

Here are the numbers from the program, run on my 2017 Macbook. The goal is to
run this on more platform. Note that all numbers don't line up as they've been
rounded to make them more memorable.


| Operation                              | Latency | Throughput | 1 MiB  | 1 GiB  |
|----------------------------------------|---------|------------|--------|--------|
| Sequential Memory R/W (64 bytes)       | 5 ns    | 10 GiB/s   | 100 us | 100 ms |
| Random Memory R/W (64 bytes)           | 50 ns   | 1 GiB/s    | 1 ms   | 1 s    |
| Sequential SSD Read (8 KiB)            | 1 μs    | 4 GiB/s    | 200 us | 200 ms |
| Sequential SSD write, -fsync (8KiB)    | 10 μs   | 1 GiB/s    | 1 ms   | 1 s    |
| TCP Echo (TCP overhead) (64 bytes)     | 15 μs   | ?          | ?      | ?      |
| Random SSD Seek (8 KiB)                | 100 μs  | 70 MiB/s   | 10 ms  | 15 s   |
| Cloud us-east1 to us-east2             | 250 μs  | ?          | ?      | ?      |
| Sequential SSD write, +fsync (8KiB)    | 5 ms    | 2 MiB/s    | 1 s    | 10 min |
| Mutex Lock/Unlock                      | ?       | ?          | ?      | ?      |
| {Snappy, Gzip, ..} Compression (? KiB) | ?       | ?          | ?      | ?      |
| Hashing (? bytes)                      | ?       | ?          | ?      | ?      |
| {MySQL, Memcached, Redis, ..} Query    | ?       | ?          | ?      | ?      |
| Envoy/Nginx Overhead                   | ?       | ?          | ?      | ?      |
| {JSON, Protobuf, ..} Serializee (?)    | ?       | ?          | ?      | ?      |
| Cloud us-east to us-central            | ?       | ?          | ?      | ?      |

You can run this with `RUSTFLAGS='-C target-cpu=native' cargo run --release`. You won't get the right numbers
when you're compiling in debug mode. You can help this project by adding new
suites and filling out the blanks.

## Cost Numbers

Approximate numbers that should be consistent between Cloud providers.

| What        | Amount | $ / Month |
|-------------|--------|-----------|
| CPU         | 1      | $10       |
| Memory      | 1 GB   | $1        |
| SSD         | 1 GB   | $0.1      |
| Disk        | 1 GB   | $0.01     |
| S3, GCS, .. | 1 GB   | $0.01     |
| Network     | 1 GB   | $0.01     |

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

