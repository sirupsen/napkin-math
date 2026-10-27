// Network rows between two hosts: TCP round trips and bulk streams. No
// dependencies.
//
//   cargo bench --bench net -- --server              # on host A, listens on TCP 5900
//   cargo bench --bench net -- 10.0.0.12             # on host B, with A's private IP
//   cargo bench --bench net -- 10.0.0.12 --secs 15 --flows 1,8,32
//
// Round trips: one connection with TCP_NODELAY sends a message and waits for
// the same number of bytes back, for 1 B and 32 KiB messages. Streams: N
// connections write as fast as they can, and the server reports how many
// bytes it received, so throughput is counted on the receiving side.
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const PORT: u16 = 5900;

fn serve(mut conn: TcpStream) -> std::io::Result<()> {
    conn.set_nodelay(true)?;
    let mut header = [0u8; 5];
    conn.read_exact(&mut header)?;
    let len = u32::from_le_bytes([header[1], header[2], header[3], header[4]]) as usize;
    let mut buf = vec![0u8; len.max(1 << 20)];
    match header[0] {
        // Echo: read `len` bytes, send them back, until the client hangs up.
        b'r' => loop {
            if conn.read_exact(&mut buf[..len]).is_err() {
                return Ok(());
            }
            conn.write_all(&buf[..len])?;
        },
        // Sink: count bytes until the client shuts down its side, then report.
        b's' => {
            let mut total = 0u64;
            loop {
                match conn.read(&mut buf)? {
                    0 => break,
                    n => total += n as u64,
                }
            }
            conn.write_all(&total.to_le_bytes())
        }
        _ => Ok(()),
    }
}

fn connect(host: &str, mode: u8, len: u32) -> TcpStream {
    let mut conn = TcpStream::connect((host, PORT)).unwrap_or_else(|e| panic!("connect {}:{}: {}", host, PORT, e));
    conn.set_nodelay(true).unwrap();
    let mut header = vec![mode];
    header.extend_from_slice(&len.to_le_bytes());
    conn.write_all(&header).unwrap();
    conn
}

fn round_trips(host: &str, size: usize, secs: u64) {
    let mut conn = connect(host, b'r', size as u32);
    let mut buf = vec![0x5au8; size];
    // Warm up the connection (slow start, ARP, first-packet paths).
    for _ in 0..100 {
        conn.write_all(&buf).unwrap();
        conn.read_exact(&mut buf).unwrap();
    }
    let mut lat: Vec<u32> = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        let begin = Instant::now();
        conn.write_all(&buf).unwrap();
        conn.read_exact(&mut buf).unwrap();
        lat.push(begin.elapsed().as_nanos().min(u32::MAX as u128) as u32);
    }
    let mean = lat.iter().map(|&l| l as f64).sum::<f64>() / lat.len() as f64 / 1e3;
    lat.sort_unstable();
    let pct = |p: f64| lat[((lat.len() - 1) as f64 * p) as usize] as f64 / 1e3;
    println!(
        "round trip {:>6} B   mean={:>9.1}us  min={:>9.1}us  p50={:>9.1}us  p99={:>9.1}us  ({} round trips)",
        size,
        mean,
        pct(0.0),
        pct(0.5),
        pct(0.99),
        lat.len()
    );
}

fn streams(host: &str, flows: usize, secs: u64) {
    let stop = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let workers: Vec<_> = (0..flows)
        .map(|_| {
            let (host, stop) = (host.to_string(), stop.clone());
            std::thread::spawn(move || {
                let mut conn = connect(&host, b's', 0);
                let buf = vec![0x5au8; 1 << 20];
                while !stop.load(Ordering::Relaxed) {
                    conn.write_all(&buf).unwrap();
                }
                conn.shutdown(Shutdown::Write).unwrap();
                let mut total = [0u8; 8];
                conn.read_exact(&mut total).unwrap();
                u64::from_le_bytes(total)
            })
        })
        .collect();
    std::thread::sleep(Duration::from_secs(secs));
    stop.store(true, Ordering::Relaxed);
    let bytes: u64 = workers.into_iter().map(|w| w.join().unwrap()).sum();
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "stream {:>2} flow(s)    {:>6.2} Gbit/s = {:>5.2} GiB/s",
        flows,
        bytes as f64 * 8.0 / elapsed / 1e9,
        bytes as f64 / elapsed / (1u64 << 30) as f64
    );
}

fn main() {
    let args: Vec<String> = std::env::args().filter(|a| a != "--bench").collect();
    let usage = "usage: net --server | net <server-ip> [--secs 15] [--flows 1,8,32]";
    // `cargo bench --bench <name>` adds --bench to the arguments; without any
    // other arguments there is nothing to measure, so skip quietly.
    if args.len() < 2 {
        eprintln!("{} (no arguments, skipping)", usage);
        return;
    }
    let target = args.get(1).expect(usage).clone();
    if target == "--server" {
        let listener = TcpListener::bind(("0.0.0.0", PORT)).unwrap();
        println!("listening on TCP {}", PORT);
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let _ = serve(conn);
            });
        }
        return;
    }
    let flag = |name: &str| args.iter().position(|a| a == name).map(|i| args[i + 1].clone());
    let secs: u64 = flag("--secs").map(|s| s.parse().unwrap()).unwrap_or(15);
    let flows: Vec<usize> = flag("--flows").unwrap_or("1,8,32".into()).split(',').map(|f| f.parse().unwrap()).collect();
    println!("{} -> {}", std::fs::read_to_string("/etc/hostname").unwrap_or_default().trim(), target);
    for size in [1, 32 << 10] {
        round_trips(&target, size, secs);
    }
    for f in flows {
        streams(&target, f, secs);
    }
}
