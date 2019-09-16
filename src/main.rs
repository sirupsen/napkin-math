#[macro_use]
extern crate byte_unit;

// https://ark.intel.com/content/www/us/en/ark/products/97185/intel-core-i7-7700hq-processor-6m-cache-up-to-3-80-ghz.html
// https://en.wikichip.org/wiki/intel/core_i7/i7-7700hq
//
// Single: 17.88 GiB/s
// Dual:   37.5 GB/s
//
// L1: 256 KiB
// L2: 1 MiB
// L3: 6 MiB
//
// TODO: Would be cool to instrument branch misses etc. here
use byte_unit::Byte;
use failure::Error;
use num_format::{Locale, ToFormattedString};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng, SeedableRng};
use std::fs;
use std::fs::{OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::mem::forget;
use std::ptr;
use std::time::{Duration, Instant};
use page_size;

#[allow(unused_imports)]
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::__rdtscp;

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

        let time_unit = if single_operation_nanoseconds.as_nanos() == 0 {
            format!("{:.3} ns", self.duration.as_nanos() as f64 / self.iterations as f64)
        } else {
            self.get_appropriate_time_unit(single_operation_nanoseconds)
        };

        println!(
            "[{}] Avg single operation: {}",
            name,
            time_unit
        );

        let single_operation_cycles = self.cycles as f64 / (self.iterations as f64);

        println!(
            "[{}] Avg single operation cycles: {:.2}",
            name, single_operation_cycles,
        );
    }

    // impl on duration
    fn get_appropriate_time_unit(&self, duration: Duration) -> String {
        if duration.as_nanos() < 1000 {
            format!("{} ns", duration.as_nanos())
        } else if duration.as_nanos() > 1000 && duration.as_millis() < 100 {
            format!("{} μs", duration.as_micros())
        } else if duration.as_micros() > 1000 && duration.as_millis() < 3000 {
            format!("{} ms", duration.as_millis())
        } else {
            format!("{} s", duration.as_secs())
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
    memory_read_sequential();
    memory_write_sequential();
    memory_read_random();
    memory_write_random();

    disk_write_sequential_fsync();
    disk_write_sequential_no_fsync();
    disk_read_sequential();
    disk_read_random();
}

fn memory_write_sequential() {
    struct Test {
        i: usize,
        vec: Vec<usize>,
    }

    let result = benchmark(
        || {
            let vec: Vec<usize> = Vec::new();
            Test { i: 0, vec }
        },
        |test| {
            test.vec.push(test.i);
            test.i += 1;
            true
        },
    )
    .unwrap();

    result.print_results("Write Seq Vec<usize>", std::mem::size_of::<usize>());
}

fn memory_read_sequential() {
    let size_per_value = std::mem::size_of::<usize>();
    let size_in_elements = (n_mib_bytes!(128) as usize / size_per_value) as usize;

    struct Test {
        i: usize,
        vec: Vec<usize>,
    }

    let result = benchmark(
        || {
            let mut vec: Vec<usize> = Vec::new();
            for i in 0..size_in_elements {
                vec.push(i)
            }

            Test { i: 0, vec }
        },
        |test| {
            black_box(test.vec[test.i]);
            test.i += 1;
            // This is much faster than %
            if test.i == test.vec.len() {
                test.i = 0;
            }

            true
        },
    )
    .unwrap();

    result.print_results("Read Seq Vec<usize>", std::mem::size_of::<usize>());
}

fn memory_write_random() {
    struct Test {
        rng: SmallRng,
        vec: Vec<usize>,
    }

    let size_per_value = std::mem::size_of::<usize>();
    let size_in_elements = (n_mib_bytes!(128) as usize / size_per_value) as usize;

    let result = benchmark(
        || {
            let mut vec: Vec<usize> = (0..size_in_elements).collect();
            vec.shuffle(&mut thread_rng());
            let rng = SmallRng::from_entropy();
            Test { rng, vec }
        },
        |test| {
            test.vec[test.rng.gen_range(0, size_in_elements)] = 1;
            true
        },
    )
    .unwrap();

    result.print_results(
        "Real: Random Write Vec<usize>",
        std::mem::size_of::<usize>(),
    );
}

fn memory_read_random() {
    struct Test {
        vec: Vec<usize>,
        rng: SmallRng,
    }

    //  - Would be cool to use rand::rngs::mock::StepRng to go across pages
    let size_per_value = std::mem::size_of::<usize>();

    // The size is going to matter a lot here in terms of L1/L2/L3
    // It's funny, 28 KiB to 2,000 KiB seem really fast, but below or above that, and it gets slow.
    // Need to graph this!
    let size_in_elements = (n_mib_bytes!(64) as usize / size_per_value) as usize;

    let result = benchmark(
        || {
            let mut vec: Vec<usize> = (0..size_in_elements).collect();
            vec.shuffle(&mut thread_rng());

            let rng = SmallRng::from_entropy();
            Test { vec, rng }
        },
        |test| {
            black_box(test.vec[test.rng.gen_range(0, size_in_elements)]);
            true
        },
    )
    .unwrap();

    result.print_results("Random Read Vec<usize>", size_per_value);
}

fn disk_write_sequential_fsync() {
    struct Test {
        bytes: Vec<u8>,
        file: std::fs::File,
    }

    let file_name = "foo.txt";
    let size_of_writes = n_kib_bytes!(16) as usize;

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

    result.print_results("Sequential Disk Write, Fsync <16KiB>", 64);
}

fn disk_write_sequential_no_fsync() {
    struct Test {
        bytes: Vec<u8>,
        file: std::fs::File,
    }

    let file_name = "foo.txt";
    let size_of_writes = n_kib_bytes!(16) as usize;

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

    result.print_results("Sequential Disk Write, No Fsync <16KiB>", 64);
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
            let buffer = vec![0; n_gib_bytes!(2) as usize];
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
    struct Test {
        rng: SmallRng,
        file_length: u64,
        buffer: [u8; 64],
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

            let buffer: [u8; 64] = [0; 64];
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
            test.i += 1;

            if test.i == test.pages.len() {
                return false
            };

            true
        },
    )
    .unwrap();

    result.print_results("Random Disk Seek <64b>", 64);
}
