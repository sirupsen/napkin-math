// Compression ratio and single-threaded speed for real files, in the spirit of
// lzbench (README reference [11]) but using the crates this repo already
// builds. Speeds are best-of-N over at least one second, reported against the
// uncompressed size, like lzbench.
//
//   cargo run --release --example cpu_compress -- silesia.tar dickens ...
//   cargo run --release --example cpu_compress -- --chunks silesia.tar   # lz4 and zstd -1, one call per chunk
#[path = "common/timing.rs"]
mod timing;
use std::io::{Read, Write};
use std::time::Duration;
use timing::mib_s;

// Each speed is the fastest call in at least this long.
fn best_of(f: impl FnMut()) -> f64 {
    timing::best_for(Duration::from_secs(1), f)
}

// Both write into a caller's buffer that is reused across calls, as lzbench does: allocating a
// fresh output each call would time the page faults of a buffer the size of the file. `out` only
// ever grows, so after the first call nothing is allocated or zeroed. They return bytes written.
struct Codec {
    name: &'static str,
    compress: fn(&[u8], &mut Vec<u8>) -> usize,
    decompress: fn(&[u8], &mut [u8]) -> usize,
}

fn grow(out: &mut Vec<u8>, len: usize) -> &mut [u8] {
    if out.len() < len {
        out.resize(len, 0);
    }
    &mut out[..]
}

// gzip and brotli have no published bound handy; incompressible input grows by well under 1/8.
fn stream_bound(len: usize) -> usize {
    len + len / 8 + 4096
}

fn lz4_compress(input: &[u8], out: &mut Vec<u8>) -> usize {
    let buf = grow(out, lz4_flex::block::get_maximum_output_size(input.len()));
    lz4_flex::block::compress_into(input, buf).unwrap()
}

fn lz4_decompress(input: &[u8], out: &mut [u8]) -> usize {
    lz4_flex::block::decompress_into(input, out).unwrap()
}

fn snappy_compress(input: &[u8], out: &mut Vec<u8>) -> usize {
    let buf = grow(out, snap::raw::max_compress_len(input.len()));
    snap::raw::Encoder::new().compress(input, buf).unwrap()
}

fn snappy_decompress(input: &[u8], out: &mut [u8]) -> usize {
    snap::raw::Decoder::new().decompress(input, out).unwrap()
}

fn zstd_compress(level: i32, input: &[u8], out: &mut Vec<u8>) -> usize {
    let buf = grow(out, zstd::zstd_safe::compress_bound(input.len()));
    zstd::bulk::compress_to_buffer(input, buf, level).unwrap()
}

fn zstd_decompress(input: &[u8], out: &mut [u8]) -> usize {
    zstd::bulk::decompress_to_buffer(input, out).unwrap()
}

fn gzip_compress(level: u32, input: &[u8], out: &mut Vec<u8>) -> usize {
    let buf = grow(out, stream_bound(input.len()));
    let mut cursor = std::io::Cursor::new(buf);
    let mut encoder = flate2::write::GzEncoder::new(&mut cursor, flate2::Compression::new(level));
    encoder.write_all(input).unwrap();
    encoder.finish().unwrap();
    cursor.position() as usize
}

fn gzip_decompress(input: &[u8], out: &mut [u8]) -> usize {
    read_all(flate2::read::GzDecoder::new(input), out)
}

fn brotli_compress(quality: u32, input: &[u8], out: &mut Vec<u8>) -> usize {
    let buf = grow(out, stream_bound(input.len()));
    let mut cursor = std::io::Cursor::new(buf);
    {
        let mut writer = brotli::CompressorWriter::new(&mut cursor, 1 << 16, quality, 22);
        writer.write_all(input).unwrap();
    }
    cursor.position() as usize
}

fn brotli_decompress(input: &[u8], out: &mut [u8]) -> usize {
    read_all(brotli::Decompressor::new(input, 1 << 16), out)
}

// Fill `out` (sized to the original length) from a decoder, and check nothing is left over.
fn read_all(mut reader: impl Read, out: &mut [u8]) -> usize {
    reader.read_exact(out).unwrap();
    assert_eq!(reader.read(&mut [0u8; 1]).unwrap(), 0, "decoded more than the original");
    out.len()
}

fn codecs() -> Vec<Codec> {
    vec![
        Codec {
            name: "lz4_flex",
            compress: lz4_compress,
            decompress: lz4_decompress,
        },
        Codec {
            name: "snappy",
            compress: snappy_compress,
            decompress: snappy_decompress,
        },
        Codec {
            name: "zstd -1",
            compress: |input, out| zstd_compress(1, input, out),
            decompress: zstd_decompress,
        },
        Codec {
            name: "zstd -3",
            compress: |input, out| zstd_compress(3, input, out),
            decompress: zstd_decompress,
        },
        Codec {
            name: "zstd -9",
            compress: |input, out| zstd_compress(9, input, out),
            decompress: zstd_decompress,
        },
        Codec {
            name: "zstd -19",
            compress: |input, out| zstd_compress(19, input, out),
            decompress: zstd_decompress,
        },
        Codec {
            name: "gzip -1",
            compress: |input, out| gzip_compress(1, input, out),
            decompress: gzip_decompress,
        },
        Codec {
            name: "gzip -6",
            compress: |input, out| gzip_compress(6, input, out),
            decompress: gzip_decompress,
        },
        Codec {
            name: "brotli -5",
            compress: |input, out| brotli_compress(5, input, out),
            decompress: brotli_decompress,
        },
        Codec {
            name: "brotli -11",
            compress: |input, out| brotli_compress(11, input, out),
            decompress: brotli_decompress,
        },
    ]
}

// lz4 and zstd -1 on the file split into chunks, one call per chunk, each on its own. zstd reuses
// its context, as anything calling it in a loop would. A README row's size is the largest that
// takes about as long as the smallest, and its throughput the fastest at any size.
fn chunks(path: &str) {
    let input = std::fs::read(path).unwrap();
    println!("| chunk | codec | ratio | compress MiB/s | us | decompress MiB/s | us |");
    println!("| ----- | ----- | ----- | -------------- | -- | ---------------- | -- |");
    for size in [64, 256, 1 << 10, 4 << 10, 16 << 10, 64 << 10, 256 << 10, 1 << 20, 4 << 20] {
        // Millions of tiny calls would take minutes, and don't change the answer.
        let sample = if size < 4096 { &input[..input.len().min(16 << 20)] } else { &input[..] };
        let parts: Vec<&[u8]> = sample.chunks_exact(size).collect();
        let total = parts.len() * size;
        let per_chunk = |secs: f64| secs * 1e6 / parts.len() as f64;
        let row = |codec: &str, compressed: usize, c: f64, d: f64| {
            println!(
                "| {} B | {} | {:.2} | {:.0} | {:.2} | {:.0} | {:.2} |",
                size,
                codec,
                total as f64 / compressed as f64,
                mib_s(total, c),
                per_chunk(c),
                mib_s(total, d),
                per_chunk(d),
            );
        };

        let packed: Vec<Vec<u8>> = parts.iter().map(|p| lz4_flex::block::compress(p)).collect();
        let mut lz4_out = vec![0u8; lz4_flex::block::get_maximum_output_size(size)];
        let c = best_of(|| {
            for p in &parts {
                let n = lz4_flex::block::compress_into(p, &mut lz4_out).unwrap();
                std::hint::black_box(n);
            }
        });
        let mut out = vec![0u8; size];
        let d = best_of(|| {
            for p in &packed {
                let n = lz4_flex::block::decompress_into(p, &mut out).unwrap();
                std::hint::black_box((n, out.as_ptr()));
            }
        });
        row("lz4_flex", packed.iter().map(Vec::len).sum(), c, d);

        let mut compressor = zstd::bulk::Compressor::new(1).unwrap();
        let mut decompressor = zstd::bulk::Decompressor::new().unwrap();
        let mut packed_out = vec![0u8; zstd::zstd_safe::compress_bound(size)];
        let packed: Vec<Vec<u8>> = parts
            .iter()
            .map(|p| compressor.compress(p).unwrap())
            .collect();
        let c = best_of(|| {
            for p in &parts {
                let n = compressor.compress_to_buffer(p, &mut packed_out).unwrap();
                std::hint::black_box(n);
            }
        });
        let d = best_of(|| {
            for p in &packed {
                let n = decompressor.decompress_to_buffer(p, &mut out).unwrap();
                std::hint::black_box((n, out.as_ptr()));
            }
        });
        row("zstd -1", packed.iter().map(Vec::len).sum(), c, d);
    }
}

fn main() {
    let only: Option<Vec<String>> = std::env::var("NAPKIN_COMPRESSION_CODECS")
        .ok()
        .map(|s| s.split(',').map(|c| c.trim().to_string()).collect());

    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: cpu_compress [--chunks] <files...>");
        std::process::exit(2);
    }
    if paths[0] == "--chunks" {
        for path in &paths[1..] {
            chunks(path);
        }
        return;
    }
    println!("| file | MiB | codec | ratio | compress MiB/s | decompress MiB/s |");
    println!("| ---- | --- | ----- | ----- | -------------- | ---------------- |");
    for path in paths {
        let input = std::fs::read(&path).unwrap();
        let file = std::path::Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for codec in codecs() {
            if let Some(only) = &only {
                if !only.iter().any(|c| c == codec.name) {
                    continue;
                }
            }
            let mut packed = Vec::new();
            let n = (codec.compress)(&input, &mut packed);
            let compressed = packed[..n].to_vec();
            let mut out = vec![0u8; input.len()];
            assert_eq!((codec.decompress)(&compressed, &mut out), input.len());
            assert!(out == input, "{} didn't round-trip {}", codec.name, file);
            let compress = best_of(|| {
                std::hint::black_box((codec.compress)(&input, &mut packed));
            });
            let decompress = best_of(|| {
                std::hint::black_box((codec.decompress)(&compressed, &mut out));
            });
            println!(
                "| {} | {:.0} | {} | {:.2} | {:.0} | {:.0} |",
                file,
                input.len() as f64 / (1024.0 * 1024.0),
                codec.name,
                input.len() as f64 / compressed.len() as f64,
                mib_s(input.len(), compress),
                mib_s(input.len(), decompress),
            );
        }
    }
}
