#[macro_use]
extern crate criterion;
#[macro_use]
extern crate byte_unit;

use criterion::Criterion;
use criterion::black_box;
use std::mem;

fn memory_sequential_write_vec_push(n: usize) -> Vec<usize> {
    let mut vec: Vec<usize> = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    vec
}

fn memory_sequential_read_vec_read(vec: &Vec<usize>) -> usize {
    let mut res = 0; // prevent compiler from optimizing out the walk
    for i in vec {
        res += i;
    }
    res
}

fn criterion_benchmark(c: &mut Criterion) {
    let one_mib_in_usizes = (n_mib_bytes!(1) as usize) / mem::size_of::<usize>();

    c.bench_function("write 1MiB", move |b| b.iter(||
        memory_sequential_write_vec_push(black_box(one_mib_in_usizes))
    ));

    let one_mib_vec = memory_sequential_write_vec_push(one_mib_in_usizes);
    c.bench_function("read 1 MiB", move |b| b.iter(||
        memory_sequential_read_vec_read(black_box(&one_mib_vec))
    ));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
