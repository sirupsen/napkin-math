// Proxy Request and Database Query rows: request round trips to a server on
// one connection, one request at a time. No dependencies; each protocol is the
// few bytes it takes to ask for one small value.
//
//   examples/services.sh                     # Ubuntu: install, configure and measure all of them
//   cargo run --release --example services -- redis=127.0.0.1:6379 postgres=127.0.0.1:5432 --secs 10
//
// Targets are kind=host:port, where kind is http, redis, memcached, postgres
// or mysql. Every request reads a random key of `kv`, a 100k-row table of
// (k int primary key, v text) in database `bench` as user `bench` with no
// password (the keys are k:0..k:99999 in Redis and Memcached). A proxy's cost
// is an http target through the proxy minus the same target directly.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const KEYS: u64 = 100_000;

struct Conn {
    r: BufReader<TcpStream>,
    w: TcpStream,
}

impl Conn {
    fn open(addr: &str) -> Conn {
        let w = TcpStream::connect(addr).unwrap_or_else(|e| panic!("connect {}: {}", addr, e));
        w.set_nodelay(true).unwrap();
        Conn {
            r: BufReader::new(w.try_clone().unwrap()),
            w,
        }
    }
    // One write per request: with TCP_NODELAY, write! on the stream sends each formatted piece
    // as its own syscall and packet, and the server may wake up on half a request.
    fn send(&mut self, request: &[u8]) {
        self.w.write_all(request).unwrap();
    }
    fn line(&mut self) -> String {
        let mut s = String::new();
        if self.r.read_line(&mut s).unwrap() == 0 {
            panic!("server closed the connection");
        }
        s
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        let mut b = vec![0; n];
        self.r.read_exact(&mut b).unwrap();
        b
    }
}

// HTTP/1.1 GET with keep-alive; the body is read by Content-Length.
fn http(c: &mut Conn, _key: u64) {
    c.w.write_all(b"GET / HTTP/1.1\r\nHost: bench\r\n\r\n")
        .unwrap();
    let status = c.line();
    assert!(status.starts_with("HTTP/1.1 200"), "{}", status.trim());
    let mut len = 0;
    loop {
        let h = c.line();
        if h == "\r\n" {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap();
        }
    }
    c.bytes(len);
}

fn redis(c: &mut Conn, key: u64) {
    let k = format!("k:{}", key);
    c.send(format!("*2\r\n$3\r\nGET\r\n${}\r\n{}\r\n", k.len(), k).as_bytes());
    let head = c.line();
    assert!(head.starts_with('$'), "{}", head.trim());
    let len: i64 = head[1..].trim().parse().unwrap();
    assert!(len >= 0, "missing key {}", k);
    c.bytes(len as usize + 2);
}

fn memcached(c: &mut Conn, key: u64) {
    c.send(format!("get k:{}\r\n", key).as_bytes());
    let head = c.line();
    assert!(
        head.starts_with("VALUE"),
        "missing key k:{}: {}",
        key,
        head.trim()
    );
    let len: usize = head.split_whitespace().nth(3).unwrap().parse().unwrap();
    c.bytes(len + 2);
    assert_eq!(c.line(), "END\r\n");
}

// Postgres frontend/backend protocol 3.0 with trust authentication, and a
// simple query (parsed and planned every time, like most ORMs without
// prepared statements).
fn pg_until_ready(c: &mut Conn) {
    loop {
        let head = c.bytes(5);
        let len = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
        let body = c.bytes(len - 4);
        match head[0] {
            b'E' => panic!("postgres: {}", String::from_utf8_lossy(&body)),
            b'R' => assert_eq!(
                &body[..4],
                &[0, 0, 0, 0],
                "postgres wants a password; use trust"
            ),
            b'Z' => return,
            _ => {}
        }
    }
}

fn pg_message(kind: Option<u8>, body: &[u8]) -> Vec<u8> {
    let mut m: Vec<u8> = kind.into_iter().collect();
    m.extend_from_slice(&(body.len() as u32 + 4).to_be_bytes());
    m.extend_from_slice(body);
    m
}

fn postgres_connect(c: &mut Conn) {
    let mut body = 196608u32.to_be_bytes().to_vec();
    body.extend_from_slice(b"user\0bench\0database\0bench\0\0");
    c.w.write_all(&pg_message(None, &body)).unwrap();
    pg_until_ready(c);
}

fn postgres(c: &mut Conn, key: u64) {
    let q = format!("SELECT v FROM kv WHERE k = {}\0", key);
    c.w.write_all(&pg_message(Some(b'Q'), q.as_bytes()))
        .unwrap();
    pg_until_ready(c);
}

// MySQL client/server protocol 4.1, as user `bench` with an empty password,
// which every auth plugin accepts as empty auth data.
fn mysql_read(c: &mut Conn) -> (u8, Vec<u8>) {
    let head = c.bytes(4);
    let len = u32::from_le_bytes([head[0], head[1], head[2], 0]) as usize;
    (head[3], c.bytes(len))
}

fn mysql_write(c: &mut Conn, seq: u8, payload: &[u8]) {
    let mut packet = (payload.len() as u32).to_le_bytes().to_vec();
    packet[3] = seq;
    packet.extend_from_slice(payload);
    c.send(&packet);
}

fn mysql_connect(c: &mut Conn) {
    let (_, greeting) = mysql_read(c);
    // The greeting ends with the server's default auth plugin name.
    let plugin = greeting[..greeting.len() - 1]
        .rsplit(|&b| b == 0)
        .next()
        .unwrap()
        .to_vec();
    let caps: u32 = 0x1 | 0x8 | 0x200 | 0x8000 | 0x80000; // long password, with db, 4.1, secure auth, plugin auth
    let mut p = caps.to_le_bytes().to_vec();
    p.extend_from_slice(&(16u32 << 20).to_le_bytes());
    p.push(33);
    p.extend_from_slice(&[0; 23]);
    p.extend_from_slice(b"bench\0\0bench\0");
    p.extend_from_slice(&plugin);
    p.push(0);
    mysql_write(c, 1, &p);
    loop {
        let (seq, r) = mysql_read(c);
        match r[0] {
            0x00 => return,
            0xff => panic!("mysql: {}", String::from_utf8_lossy(&r[3..])),
            // Auth switch to another plugin: answer with empty auth data.
            0xfe => mysql_write(c, seq + 1, &[]),
            _ => {}
        }
    }
}

fn mysql(c: &mut Conn, key: u64) {
    let q = format!("\x03SELECT v FROM kv WHERE k = {}", key);
    mysql_write(c, 0, q.as_bytes());
    let (_, first) = mysql_read(c);
    assert_ne!(
        first[0],
        0xff,
        "mysql: {}",
        String::from_utf8_lossy(&first[3..])
    );
    // Column definitions, EOF, rows, EOF.
    let mut eofs = 0;
    while eofs < 2 {
        let (_, p) = mysql_read(c);
        assert_ne!(p[0], 0xff, "mysql: {}", String::from_utf8_lossy(&p[3..]));
        if p[0] == 0xfe && p.len() < 9 {
            eofs += 1;
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: services kind=host:port ... [--secs 10]   (kind: http redis memcached postgres mysql)";
    let mut secs = 10;
    let mut targets = vec![];
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--secs" {
            secs = args[i + 1].parse().expect(usage);
            i += 2;
            continue;
        }
        targets.push(args[i].split_once('=').expect(usage));
        i += 1;
    }
    if targets.is_empty() {
        eprintln!("{usage}");
        std::process::exit(2);
    }
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    for (kind, addr) in targets {
        let mut c = Conn::open(addr);
        let request: fn(&mut Conn, u64) = match kind {
            "http" => http,
            "redis" => redis,
            "memcached" => memcached,
            "postgres" => {
                postgres_connect(&mut c);
                postgres
            }
            "mysql" => {
                mysql_connect(&mut c);
                mysql
            }
            _ => panic!("{}", usage),
        };
        let mut lat = vec![];
        let start = Instant::now();
        let deadline = Duration::from_secs(secs);
        while start.elapsed() < deadline {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let t = Instant::now();
            request(&mut c, x % KEYS);
            lat.push(t.elapsed());
        }
        // The first second warms caches and connections.
        let skip = lat.len() / secs as usize;
        let mut lat = lat.split_off(skip.min(lat.len() - 1));
        let mean = lat.iter().sum::<Duration>() / lat.len() as u32;
        lat.sort();
        let us = |d: Duration| d.as_secs_f64() * 1e6;
        println!(
            "{:<10} {:<21} requests/s={:>7.0}  mean={:>7.1}us  p50={:>7.1}us  p99={:>7.1}us",
            kind,
            addr,
            1.0 / mean.as_secs_f64(),
            us(mean),
            us(lat[lat.len() / 2]),
            us(lat[lat.len() * 99 / 100])
        );
    }
}
