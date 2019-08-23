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
use std::mem::forget;
use std::ptr;
use std::time::{Duration, Instant};

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
    duration_ratio: f64,
    intended_duration: Duration,
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
            "[{}] Values per second: {}",
            name,
            ((((self.iterations as f64 / self.duration.as_millis() as f64) as f64) * 1000.0)
                as u128)
                .to_formatted_string(&Locale::en)
        );

        println!("[{}] Size of type: {} bytes", name, size_of_type,);

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
    }
}

// TODO: return something useful instead of printing it here
fn run_for<F: FnMut(usize) -> ()>(
    iterations_per_check: usize,
    intended_duration: Duration,
    mut func: F,
) -> Result<BenchmarkResult, Error> {
    let instant = Instant::now();
    let mut iterations: usize = 0;
    while instant.elapsed() < intended_duration {
        for _ in 0..iterations_per_check {
            func(iterations)
        }
        iterations += iterations_per_check;
    }
    let actual_duration = instant.elapsed();

    // TODO: It would be cool to get data for every `iterations_per_check` to be able to graph how
    // e.g. write-throughput goes up as the machine warms up and dedicates a core to writing.
    //
    // For the test where we write values to an array, it seems that we get more throughput as we
    // increase the length of the test for some reason... I'm wondering if that's because some
    // seconds get a lot more throughput than others...?
    //
    // Perhaps these are not values, but functions. Even then, we should stick to values... not
    // functions.
    Ok(BenchmarkResult {
        iterations: iterations,
        duration: actual_duration,
        duration_ratio: intended_duration.as_nanos() as f64 / actual_duration.as_nanos() as f64,
        intended_duration: intended_duration,
    })
}

// TODO: take args for how long to perform tests
fn main() {
    // SEQ WRITE
    let mut vec: Vec<usize> = Vec::new();
    let result = run_for(1, Duration::from_millis(100), move |i| vec.push(i)).unwrap();
    result.print_results("Warmup: Write Seq Vec<usize>", std::mem::size_of::<usize>());

    let mut vec: Vec<usize> = Vec::new();
    let result = run_for(result.iterations, Duration::from_millis(1000), move |i| {
        vec.push(i)
    })
    .unwrap();
    result.print_results("Real: Write Seq Vec<usize>", std::mem::size_of::<usize>());

    // SEQ READ
    let mut vec: Vec<usize> = Vec::new();
    let size_per_value = std::mem::size_of::<usize>();
    // Changing this doesn't make a difference, I assume because the pre-fetching is so incredibly
    // aggressive..?
    let size_in_elements = (n_mib_bytes!(256) as usize / size_per_value) as usize;
    for i in 0..size_in_elements {
        vec.push(i)
    }

    let result = run_for(1, Duration::from_millis(100), |i| {
        black_box(vec[i % vec.len()]);
    })
    .unwrap();
    result.print_results("Warmup: Read Seq Vec<usize>", std::mem::size_of::<usize>());

    let result = run_for(result.iterations, Duration::from_millis(1000), |i| {
        black_box(vec[i % vec.len()]);
    })
    .unwrap();
    result.print_results("Real: Read Seq Vec<usize>", std::mem::size_of::<usize>());

    // RANDOM WRITE
    let size_per_value = std::mem::size_of::<usize>();
    let size_in_elements = (n_mib_bytes!(256) as usize / size_per_value) as usize;
    let mut vec: Vec<usize> = (0..size_in_elements).collect();
    vec.shuffle(&mut thread_rng());

    let mut rng = SmallRng::from_entropy();
    let result = run_for(1, Duration::from_millis(100), move |i| {
        vec[rng.gen_range(0, size_in_elements)] = i
    })
    .unwrap();
    result.print_results(
        "Warmup: Random Write Vec<usize>",
        std::mem::size_of::<usize>(),
    );

    let mut vec: Vec<usize> = (0..size_in_elements).collect();
    vec.shuffle(&mut thread_rng());
    let mut rng = SmallRng::from_entropy();
    let result = run_for(result.iterations, Duration::from_millis(100), move |i| {
        vec[rng.gen_range(0, size_in_elements)] = i
    })
    .unwrap();
    result.print_results(
        "Real: Random Write Vec<usize>",
        std::mem::size_of::<usize>(),
    );

    // RANDOM READ
    //  - Would be cool to use rand::rngs::mock::StepRng to go across pages
    let size_per_value = std::mem::size_of::<usize>();
    // The size is going to matter a lot here in terms of L1/L2/L3
    // It's funny, 28 KiB to 2,000 KiB seem really fast, but below or above that, and it gets slow.
    // Need to graph this!
    let size_in_elements = (n_kib_bytes!(2000) as usize / size_per_value) as usize;
    let mut vec: Vec<usize> = (0..size_in_elements).collect();
    vec.shuffle(&mut thread_rng());

    let mut rng = SmallRng::from_entropy();
    let result = run_for(1, Duration::from_millis(100), move |_i| {
        black_box(vec[rng.gen_range(0, size_in_elements)]);
    })
    .unwrap();
    result.print_results("Warmup: Random Read Vec<usize>", size_per_value);

    let mut vec: Vec<usize> = (0..size_in_elements).collect();
    let mut rng = SmallRng::from_entropy();
    vec.shuffle(&mut thread_rng());
    let result = run_for(result.iterations, Duration::from_millis(3000), move |_i| {
        black_box(vec[rng.gen_range(0, size_in_elements)]);
    })
    .unwrap();
    result.print_results("Real: Random Read Vec<usize>", size_per_value);
}

// fn memory_random_read(vec: &Vec<usize>) -> usize {
//     let mut rng = rand::thread_rng();
//     let random_index = rng.gen_range(0, vec.len());
//     vec[random_index]
// }
