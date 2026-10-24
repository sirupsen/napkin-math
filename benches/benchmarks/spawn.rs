use criterion::*;
use std::process::Command;
use std::thread;

fn spawn_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("spawn");
    group.bench_function("thread spawn + join", |b| {
        b.iter(|| thread::spawn(|| {}).join().unwrap())
    });
    // fork/exec (posix_spawn) of a tiny binary and wait for it to exit.
    group.bench_function("process spawn + wait (true)", |b| {
        b.iter(|| assert!(Command::new("true").status().unwrap().success()))
    });
    group.finish();
}

criterion_group!(benches, spawn_benchmark);
