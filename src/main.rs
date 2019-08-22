// extern crate rand;
// use rand::prelude::*;

use num_format::{Locale, ToFormattedString};

#[macro_use]
extern crate byte_unit;

use byte_unit::Byte;
use std::time::{Duration, Instant};

fn main() {
    let mut vec: Vec<usize> = Vec::new();

    let instant = Instant::now();
    let mut warmup_iterations: usize = 0;
    while instant.elapsed() < Duration::from_millis(100) {
        vec.push(warmup_iterations);
        warmup_iterations += 1;
    }
    let duration = instant.elapsed().as_millis();
    println!("warmup_iterations in {} miliseconds, with overhead: {}", duration, warmup_iterations);
    // println!("Bytes per second: {}", Byte::from_bytes((std::mem::size_of::<usize>() * warmup_iterations) as u128 * (1000 / duration)).get_appropriate_unit(true).format(3));

    // REAL
    let instant = Instant::now();
    let mut iterations: usize = 0;
    let mut vec: Vec<usize> = Vec::new();
    while instant.elapsed() < Duration::from_millis(1000) {
        for _ in 0..warmup_iterations {
            vec.push(iterations);
        }

        iterations += warmup_iterations;
    }
    let duration = instant.elapsed().as_millis();
    println!("\nIterations in {} miliseconds, no overhead: {}", duration, iterations);

    let total_bytes_pushed = (std::mem::size_of::<usize>() * iterations) as f64;
    println!("Total bytes pushed: {}", total_bytes_pushed);

    let duration_ratio = 1000.0 / duration as f64;
    println!("Duration ratio: {}", duration_ratio);

    let bytes_per_second = total_bytes_pushed / duration_ratio;
    println!("Bytes per second: {}", (bytes_per_second as u128).to_formatted_string(&Locale::en));
    println!("Values per second: {}", ((iterations as f64 / duration_ratio) as u128).to_formatted_string(&Locale::en));

    println!("Bytes per second: {}", Byte::from_bytes(bytes_per_second as u128).get_appropriate_unit(true).format(3));
}

// // TODO: try to go beyond the L1/L2
// fn memory_sequential_read_vec_read(vec: &Vec<usize>) -> usize {
//     // prevent compiler from optimizing out the walk, it's so dominated by memory read that it won't be an issue.
//     let mut res = 0;
//     for i in vec {
//         res += i;
//     }
//     res
// }

// fn memory_random_read(vec: &Vec<usize>) -> usize {
//     let mut rng = rand::thread_rng();
//     let random_index = rng.gen_range(0, vec.len());
//     vec[random_index]
// }
