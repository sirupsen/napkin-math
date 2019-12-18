#[macro_use]
extern crate byte_unit;
extern crate clap;

// https://ark.intel.com/content/www/us/en/ark/products/97185/intel-core-i7-7700hq-processor-6m-cache-up-to-3-80-ghz.html
// https://en.wikichip.org/wiki/intel/core_i7/i7-7700hq
//
// Single: 17.88 GiB/s
// Dual:   37.5 GB/s
//
// L1: 32 KiB
// L2: 262 KiB
// L3: 6 MiB
//
// sysctl -a | grep cache <---
//
// TODO: Would be cool to instrument branch misses etc. here
use byte_unit::Byte;
use failure::Error;
use num_format::{Locale, ToFormattedString};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng, SeedableRng};
use std::fs;
use clap::{Arg, App, SubCommand};
use std::fs::{OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::mem::forget;
use std::ptr;
use std::time::{Duration, Instant};
use page_size;
use redis::{Client,Commands};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io;

#[allow(unused_imports)]
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::__rdtscp;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// TODO: use this instead
// from bencher::black_box, avoid compiler dead-code optimizations.
pub fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = ptr::read_volatile(&dummy);
        forget(dummy);
        ret
    }
}

// TODO: Probably we should just expose duration and iterations, and correct for duration_ratio
// directly in whatever produces this data structure to simplify consumption.
struct BenchmarkResult {
    iterations: usize,
    duration: Duration,
    // duration_ratio: f64,
    // intended_duration: Duration,
    cycles: u64,
}

impl BenchmarkResult {
    fn print_results(&self, name: &str, size_of_type: usize) {
        println!(
            "\n[{}] Iterations in {} miliseconds, no overhead: {}",
            name,
            self.duration.as_millis(),
            self.iterations.to_formatted_string(&Locale::en)
        );

        println!(
            "[{}] Iterations / second: {}",
            name,
            ((((self.iterations as f64 / self.duration.as_millis() as f64) as f64) * 1000.0)
                as u128)
                .to_formatted_string(&Locale::en)
        );

        println!(
            "[{}] Bytes handled per iteration: {} bytes",
            name, size_of_type
        );

        let total_bytes_pushed = size_of_type * self.iterations;
        println!(
            "[{}] Total bytes processed: {}",
            name,
            Byte::from_bytes(total_bytes_pushed as u128)
                .get_appropriate_unit(true)
                .format(3)
        );

        let bytes_per_second =
            ((total_bytes_pushed as f64) / self.duration.as_millis() as f64) * 1000.0;
        println!(
            "[{}] Throughput: {}/s",
            name,
            // TODO: Too hard to get right when values aren't just printed!
            Byte::from_bytes(bytes_per_second as u128)
                .get_appropriate_unit(true)
                .format(3)
        );

        // TODO handle less than 1ns
        let single_operation_nanoseconds =
            Duration::from_nanos(self.duration.as_nanos() as u64 / self.iterations as u64);

        let time_unit = if single_operation_nanoseconds.as_nanos() <= 10 {
            format!("{:.3} ns",  self.duration.as_nanos() as f64 / self.iterations as f64)
        } else {
            self.get_appropriate_time_unit(single_operation_nanoseconds)
        };

        println!(
            "[{}] Avg single iteration: {}",
            name,
            time_unit
        );

        let single_operation_cycles = self.cycles as f64 / (self.iterations as f64);

        println!(
            "[{}] Avg single iteration cycles: {:.2}",
            name, single_operation_cycles,
        );

        let single_op_nanos = self.duration.as_nanos() as f64 / self.iterations as f64;
        let nanoseconds_per_byte = 1.0 / ((size_of_type as f64) / single_op_nanos);
        let nanoseconds_per_mebibyte = nanoseconds_per_byte * n_mib_bytes!(1) as f64;
        let duration_per_mebibyte = Duration::from_nanos(nanoseconds_per_mebibyte as u64);

        println!(
            "[{}] Time to process 1 MiB: {}",
            name, self.get_appropriate_time_unit(duration_per_mebibyte),
        );

        let nanoseconds_per_gibibyte = nanoseconds_per_byte * n_gib_bytes!(1) as f64;
        let duration_per_gibibyte = Duration::from_nanos(nanoseconds_per_gibibyte as u64);

        println!(
            "[{}] Time to process 1 GiB: {}",
            name, self.get_appropriate_time_unit(duration_per_gibibyte),
        );

        let nanoseconds_per_tibibyte = nanoseconds_per_byte * n_tib_bytes!(1) as f64;
        let duration_per_tibibyte = Duration::from_nanos(nanoseconds_per_tibibyte as u64);

        println!(
            "[{}] Time to process 1 TiB: {}",
            name, self.get_appropriate_time_unit(duration_per_tibibyte),
        );
    }

    // impl on duration
    fn get_appropriate_time_unit(&self, duration: Duration) -> String {
        if duration.as_nanos() < 1000 {
            format!("{} ns", duration.as_nanos())
        } else if duration.as_nanos() > 1000 && duration.as_millis() < 5 {
            format!("{} μs", duration.as_micros())
        } else if duration.as_micros() > 1000 && duration.as_millis() < 3000 {
            format!("{} ms", duration.as_millis())
        } else if duration.as_secs() <= 120 {
            format!("{:.2} s", duration.as_millis() as f64 / 1000.0)
        } else if duration.as_secs() <= 3600 {
            format!("{:.2} min", (duration.as_secs() as f64) / 60.0)
        } else {
            format!("{:.2} hours", (duration.as_secs() as f64) / 3600.0)
        }
    }
}

fn benchmark<T, F: Fn() -> (T), V: FnMut(&mut T) -> bool>(
    setup: F,
    mut f: V,
) -> Result<BenchmarkResult, Error> {
    // warmup run
    let mut val = setup();
    let intended_duration = Duration::from_millis(100);
    let mut iterations_per_check = 1;
    let mut iterations: usize = 0;
    let instant = Instant::now();

    // The reason for the "done" and boolean return type here is that some benchmarks may want to
    // finish earlier, e.g. random disk reads want to finish as soon as it's read every page since
    // otherwise we're just benchmarking memory. If this is the only use-case, maybe we should just
    // make sure this never happens.
    let mut done = false;
    while instant.elapsed() < intended_duration {
        for i in 0..iterations_per_check {
            if !f(&mut val) {
                done = true;
                iterations_per_check = i;
                break
            }
        }
        iterations += iterations_per_check;
        if done {
            break
        }
    }

    // real run
    let mut val = setup();
    let rdtsc_before: u64;
    let intended_duration = Duration::from_millis(5000);
    let mut iterations_per_check = iterations;
    let mut iterations: usize = 0;
    let instant = Instant::now();
    unsafe {
        rdtsc_before = core::arch::x86_64::_rdtsc();
    }

    let mut done = false;
    while instant.elapsed() < intended_duration {
        for i in 0..iterations_per_check {
            if !f(&mut val) {
                done = true;
                iterations_per_check = i;
                break
            }
        }
        iterations += iterations_per_check;
        if done {
            break
        }
    }
    let actual_duration = instant.elapsed();

    let rdtsc_after: u64;
    unsafe {
        rdtsc_after = core::arch::x86_64::_rdtsc();
    }

    Ok(BenchmarkResult {
        iterations: iterations,
        duration: actual_duration,
        // duration_ratio: intended_duration.as_nanos() as f64 / actual_duration.as_nanos() as f64,
        // intended_duration: intended_duration,
        cycles: rdtsc_after - rdtsc_before,
    })
}

// TODO: take args for how long to perform tests
fn main() {
    let matches = App::new("Napkin Math")
        .version("0.1")
        .author("Simon Eskildsen <simon@sirupsen.com>")
        .about("Runs computing benchmarks to find numbers for napkin math.")
        .arg(Arg::with_name("memory")
            .long("memory")
            .help("Run memory benchmarks"))
        .arg(Arg::with_name("io")
            .long("io")
            .help("Run IO benchmarks"))
        .get_matches();

    if matches.occurrences_of("memory") > 0 {
        memory_read_sequential();
        memory_write_sequential();
        memory_read_random();
        memory_write_random();

    }

    if matches.occurrences_of("io") > 0 {
        disk_read_sequential();
        disk_read_random();
        disk_write_sequential_no_fsync();
        disk_write_sequential_fsync();
    }

    // simd();
    // tcp_read_write();
    //redis_read_single_key();
}

fn memory_write_sequential() {
    struct Test {
        i: usize,
        vec: Vec<[u64; 8]>,
    }
    let bytes_per_iteration = 64;
    let size_in_elements = (n_gb_bytes!(1) as u64 / bytes_per_iteration) as usize;

    let result = benchmark(
        || {
            let mut vec = Vec::new();
            vec.resize(size_in_elements, [1, 2, 3, 4, 5, 6, 7, 8]);
            Test { i: 0, vec }
        },
        |test| {
            test.vec[test.i] = [8, 7, 6, 5, 4, 3, 2, 1];
            black_box(test.vec[test.i]);
            test.i += 1;
            if test.i == test.vec.len() {
                return false;
            }
            return true;
        },
    )
    .unwrap();

    result.print_results("Write Seq Vec<64 bytes>", 64);
}

fn memory_read_sequential() {
    let bytes_per_iteration = 64;
    let size_in_elements = (n_gb_bytes!(1) as u64 / bytes_per_iteration) as u64;

    struct Test {
        i: usize,
        vec: Vec<[u64; 8]>,
    }

    let result = benchmark(
        || {
            let mut vec: Vec<[u64; 8]> = Vec::new();
            for i in 0..size_in_elements {
                vec.push([i, i, i, i, i, i, i, i])
            }

            Test { i: 0, vec }
        },
        |test| {
            black_box(test.vec[test.i]);
            test.i += 1;
            if test.i == test.vec.len() {
                return false
            }

            true
        },
    )
    .unwrap();

    result.print_results("Read Seq Vec<64 bytes>", bytes_per_iteration as usize);
}

fn memory_write_random() {
    struct Test {
        vec: Vec<[u64; 8]>,
        order: Vec<usize>,
        i: usize,
    }

    let bytes_per_iteration = 64;
    let size_in_elements = (n_gb_bytes!(1) as u64 / bytes_per_iteration) as usize;

    let result = benchmark(
        || {
            let mut vec = Vec::new();
            vec.resize(size_in_elements, [1, 2, 3, 4, 5, 6, 7, 8]);

            let mut order: Vec<usize> = (0..size_in_elements).collect();
            order.shuffle(&mut thread_rng());
            Test { vec, order, i: 0 }
        },
        |test| {
            test.vec[test.order[test.i]] = [8, 7, 6, 5, 4, 3, 2, 1];
            black_box(test.vec[test.order[test.i]]);
            test.i += 1;
            if test.i == test.vec.len() {
                return false;
            }
            true
        },
    )
    .unwrap();

    result.print_results(
        "Random Write Vec<64 bytes>",
        bytes_per_iteration as usize
    );
}

fn memory_read_random() {
    struct Test {
        vec: Vec<[u64; 8]>,
        order: Vec<usize>,
        i: usize,
    }

    let bytes_per_iteration = 64;
    let size_in_elements = (n_gb_bytes!(1) as u64 / bytes_per_iteration) as usize;

    let result = benchmark(
        || {
            let mut vec = Vec::new();
            vec.resize(size_in_elements, [1, 2, 3, 4, 5, 6, 7, 8]);

            let mut order: Vec<usize> = (0..size_in_elements).collect();
            order.shuffle(&mut thread_rng());
            Test { vec, order, i: 0 }
        },
        |test| {
            black_box(test.vec[test.order[test.i]]);
            test.i += 1;
            if test.i == test.vec.len() {
                return false;
            }

            true
        },
    )
    .unwrap();

    result.print_results("Random Read Vec<64 bytes>", bytes_per_iteration as usize);
}

fn disk_write_sequential_fsync() {
    struct Test {
        bytes: Vec<u8>,
        file: std::fs::File,
    }

    let file_name = "foo.txt";
    let size_of_writes = n_kib_bytes!(8) as usize;

    let result = benchmark(
        || {
            let file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(file_name)
                .unwrap();

            let bytes: Vec<u8> = (0..size_of_writes).map(|_| rand::random::<u8>()).collect();

            Test { bytes, file }
        },
        |test| {
            test.file.write_all(&test.bytes).unwrap();
            test.file.sync_data().unwrap();
            true
        },
    )
    .unwrap();

    result.print_results("Sequential Disk Write, Fsync <8KiB>", size_of_writes);
}

fn disk_write_sequential_no_fsync() {
    struct Test {
        bytes: Vec<u8>,
        file: std::fs::File,
    }

    let file_name = "foo.txt";
    let size_of_writes = n_kib_bytes!(8) as usize;

    let result = benchmark(
        || {
            let file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(file_name)
                .unwrap();

            let bytes: Vec<u8> = (0..size_of_writes).map(|_| rand::random::<u8>()).collect();

            Test { bytes, file }
        },
        |test| {
            test.file.write_all(&test.bytes).unwrap();
            true
        },
    )
    .unwrap();

    result.print_results("Sequential Disk Write, No Fsync <8KiB>", size_of_writes);
}

fn disk_read_sequential() {
    const BUF_SIZE: usize = n_kib_bytes!(8) as usize;

    struct Test {
        buffer: [u8; BUF_SIZE],
        file: fs::File,
    }
    let file_name = "foo.txt";

    let result = benchmark(
        || {
            fs::remove_file(file_name).unwrap();
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .read(true)
                .open(file_name)
                .unwrap();
            let buffer = vec![0; n_gib_bytes!(1) as usize];
            file.write_all(&buffer).unwrap();
            file.sync_data().unwrap();

            let buffer: [u8; BUF_SIZE] = [0; BUF_SIZE];
            file.seek(SeekFrom::Start(0)).unwrap();

            Test {
                buffer,
                file,
            }
        },
        |test| {
            let n = test.file.read(&mut test.buffer).unwrap();
            if n < BUF_SIZE {
                test.file.seek(SeekFrom::Start(0)).unwrap();
            };
            true
        },
    )
    .unwrap();

    result.print_results("Sequential Disk Read <8kb>", BUF_SIZE);
}

fn disk_read_random() {
    const BUF_SIZE: usize = n_kib_bytes!(8) as usize;

    struct Test {
        rng: SmallRng,
        file_length: u64,
        buffer: [u8; BUF_SIZE],
        pages: Vec<u64>,
        i: usize,
        file: std::fs::File,
    }
    let file_name = "foo.txt";
    let page_size = page_size::get();

    let result = benchmark(
        || {
            fs::remove_file(file_name).unwrap();
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .read(true)
                .open(file_name)
                .unwrap();
            let buffer = vec![0; n_gib_bytes!(8) as usize];
            file.write_all(&buffer).unwrap();
            file.sync_data().unwrap();

            // This is to ensure we only visit each page once. Otherwise this is essentially just
            // benchmarking syscall + page cache, which is going to be awfully close to random
            // memory read.
            let mut pages: Vec<u64> = Vec::new();
            for i in 0..(buffer.len() / page_size) {
                pages.push((i * page_size + 1) as u64);
            }
            pages.shuffle(&mut thread_rng());

            let buffer: [u8; BUF_SIZE] = [0; BUF_SIZE];
            let metadata = fs::metadata(file_name).unwrap();

            Test {
                file,
                file_length: metadata.len() - buffer.len() as u64,
                rng: SmallRng::from_entropy(),
                pages,
                buffer,
                i: 0,
            }
        },
        |test| {
            test.file.seek(SeekFrom::Start(test.pages[test.i])).unwrap();
            test.file.read_exact(&mut test.buffer).unwrap();
            black_box(test.buffer);
            test.i += 1;

            if test.i == test.pages.len() {
                return false
            };

            true
        },
    )
    .unwrap();

    result.print_results("Random Disk Seek, No Page Cache <8KiB>", BUF_SIZE);
}

fn tcp_read_write() {
    // This server doesn't support multiple clients.
    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:8877").unwrap();
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();

            stream.set_nodelay(true).unwrap();
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_millis(1000))).unwrap();
            stream.set_write_timeout(Some(Duration::from_millis(1000))).unwrap();

            let mut buffer: [u8; 64] = [0; 64];
            let mut i = 0;

            loop {
                match stream.read(&mut buffer) {
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // println!("s{}: failed to read, err: {:?}..", i, e);
                        continue
                    },
                    Ok(n) => {
                        // println!("s{}: read: {}", i, n);

                        match stream.write(&buffer[..n]) {
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                // println!("s{}: failed to write", i);
                                continue
                            },
                            Ok(n) => {
                                // println!("s{}: write: {}", i, n);
                            },
                            Err(e) => {
                                panic!(e)
                            }
                        };
                    },
                    Err(e) => {
                        panic!(e)
                    }
                };

                // i += 1;
            }
        }
    });

    let bytes: Vec<u8> = (0..64).map(|_| rand::random::<u8>()).collect();
    let mut buffer: [u8; 64] = [0; 64];

    // This is done outside the setup block to avoid having to deal with a shutdown signal..
    let mut stream = TcpStream::connect("127.0.0.1:8877").unwrap();
    stream.set_nodelay(true).unwrap();
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(Duration::from_millis(1000))).unwrap();
    stream.set_write_timeout(Some(Duration::from_millis(1000))).unwrap();

    let result = benchmark(|| {
    }, |_| {
        match stream.write(&bytes) {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // println!("c: failed to write");
                return true
            },
            Ok(n) => {
                // println!("c: write: {}", n);

                match stream.read(&mut buffer[0..n]) {
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // println!("c: failed to read, err: {:?}..", e);
                        return true
                    },
                    Ok(_n) => {
                        // println!("c: read: {}\n", n);
                    },
                    Err(e) => {
                        // println!("omgs read! {:?}", e.raw_os_error());
                        panic!(e)
                    }
                };
            },
            Err(e) => {
                // println!("omgs write! {:?}", e.raw_os_error());
                panic!(e)
            }
        };


        true
    }).unwrap();

    result.print_results("Tcp Echo <64b>", 64);
}

#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
pub union i32simd {
    vector: __m256i,
    numbers: [u32; 8],
}

fn simd() {
    unsafe {
        let a = i32simd { vector: _mm256_set_epi32(1, 2, 3, 4, 5, 6, 7, 8) };
        let b = i32simd { vector: _mm256_set_epi32(1, 2, 3, 4, 5, 6, 7, 8) };
        let result = i32simd { vector: _mm256_mul_epi32(a.vector, b.vector) };
        let result2 = i32simd { vector: _mm256_mullo_epi32(a.vector, b.vector) };
        println!("{:?}", result.numbers);
        println!("{:?}", result2.numbers);
    }
}

fn redis_read_single_key() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();

    let result = benchmark(|| {
        let mut con = client.get_connection().unwrap();
        let bytes: Vec<u8> = (0..64).map(|_| rand::random::<u8>()).collect();
        let _ : () = con.set("1", bytes).unwrap();
        con
    }, |con| {
        let _ : Vec<u8> = con.get("1").unwrap();
        true
    }).unwrap();

    result.print_results("Redis Read <64b>", 64);
}
