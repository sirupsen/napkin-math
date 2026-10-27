use criterion::*;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{
    ClientConfig, ClientConnection, Connection, RootCertStore, ServerConfig, ServerConnection,
};
use std::hint::black_box;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

// The CPU cost of setting up a connection, without any network round trips:
// a TCP connect over loopback, and a full TLS 1.3 handshake (ECDSA P-256 cert,
// X25519, no resumption) between a rustls client and server in memory, both
// sides on this thread.

fn tls_configs() -> (Arc<ClientConfig>, Arc<ServerConfig>) {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert: CertificateDer<'static> = certified.cert.der().clone();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        certified.signing_key.serialize_der(),
    ));

    let server = ServerConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], key)
        .unwrap();

    let mut roots = RootCertStore::empty();
    roots.add(cert).unwrap();
    let mut client = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    client.resumption = rustls::client::Resumption::disabled();

    (Arc::new(client), Arc::new(server))
}

fn transfer(from: &mut Connection, to: &mut Connection, buffer: &mut Vec<u8>) {
    while from.wants_write() {
        buffer.clear();
        from.write_tls(buffer).unwrap();
        let mut bytes = &buffer[..];
        while !bytes.is_empty() {
            to.read_tls(&mut bytes).unwrap();
        }
        to.process_new_packets().unwrap();
    }
}

fn tls_handshake(client: &Arc<ClientConfig>, server: &Arc<ServerConfig>, buffer: &mut Vec<u8>) {
    let name = ServerName::try_from("localhost").unwrap();
    let mut client = Connection::Client(ClientConnection::new(client.clone(), name).unwrap());
    let mut server = Connection::Server(ServerConnection::new(server.clone()).unwrap());
    while client.is_handshaking() || server.is_handshaking() {
        transfer(&mut client, &mut server, buffer);
        transfer(&mut server, &mut client, buffer);
    }
    black_box((client, server));
}

#[cfg(unix)]
fn reset_on_close(stream: &TcpStream) {
    // SO_LINGER 0 closes with a RST, so connections don't pile up in TIME_WAIT
    // and exhaust ephemeral ports over a long run.
    use std::os::fd::AsRawFd;
    let linger = libc::linger {
        l_onoff: 1,
        l_linger: 0,
    };
    unsafe {
        libc::setsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_LINGER,
            &linger as *const libc::linger as *const libc::c_void,
            std::mem::size_of::<libc::linger>() as libc::socklen_t,
        );
    }
}

fn cpu_handshake_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_handshake");

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            // Wait for the client's RST so the server side never lingers either.
            let mut stream = stream.unwrap();
            let _ = stream.read(&mut [0u8; 1]);
        }
    });
    group.bench_function("tcp connect, loopback", |b| {
        b.iter(|| {
            let stream = TcpStream::connect(addr).unwrap();
            #[cfg(unix)]
            reset_on_close(&stream);
            drop(stream);
        })
    });

    let (client, server) = tls_configs();
    let mut buffer = Vec::new();
    group.bench_function("tls 1.3 handshake, client + server", |b| {
        b.iter(|| tls_handshake(&client, &server, &mut buffer))
    });

    group.finish();
}

criterion_group!(benches, cpu_handshake_benchmark);
