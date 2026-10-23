use criterion::*;
use std::hint::black_box;

use super::KIB;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

const BUF_SIZE: usize = 32 * KIB;

// Round trip of a 32 KiB message through an echo server on loopback. This is the README's
// "TCP Echo, loopback (32 KiB)" row: kernel TCP stack and syscall cost, no NIC involved.
fn tcp_benchmark(c: &mut Criterion) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_nodelay(true).unwrap();
        let mut buffer = [0u8; BUF_SIZE];
        loop {
            let n = stream.read(&mut buffer).unwrap();
            if n == 0 {
                return;
            }
            stream.write_all(&buffer[..n]).unwrap();
        }
    });

    let mut client = TcpStream::connect(addr).unwrap();
    client.set_nodelay(true).unwrap();
    let message: Vec<u8> = (0..BUF_SIZE).map(|i| i as u8).collect();
    let mut reply = vec![0u8; BUF_SIZE];

    let mut group = c.benchmark_group("tcp");
    group.throughput(Throughput::Bytes(BUF_SIZE as u64));
    group.bench_function("echo 32 KiB, loopback", |b| {
        b.iter(|| {
            client.write_all(&message).unwrap();
            client.read_exact(&mut reply).unwrap();
            black_box(&reply);
        })
    });
    group.finish();

    drop(client);
    server.join().unwrap();
}

criterion_group!(benches, tcp_benchmark);
