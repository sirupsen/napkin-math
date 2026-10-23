// Compression ratio and single-threaded speed for real files, in the spirit of
// lzbench (README reference [11]) but using the crates this repo already
// builds. Speeds are best-of-N over at least one second, reported against the
// uncompressed size, like lzbench.
//
//   cargo run --release --bin compression_corpus -- silesia.tar dickens ...
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const MIN_TIME: Duration = Duration::from_secs(1);

struct Codec {
    name: &'static str,
    compress: fn(&[u8]) -> Vec<u8>,
    decompress: fn(&[u8], usize) -> Vec<u8>,
}

fn zstd_compress(level: i32, input: &[u8]) -> Vec<u8> {
    zstd::bulk::compress(input, level).unwrap()
}

fn zstd_decompress(input: &[u8], len: usize) -> Vec<u8> {
    zstd::bulk::decompress(input, len).unwrap()
}

fn gzip_compress(level: u32, input: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(level));
    encoder.write_all(input).unwrap();
    encoder.finish().unwrap()
}

fn gzip_decompress(input: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    flate2::read::GzDecoder::new(input)
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn brotli_compress(quality: u32, input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::new(&mut out, 1 << 16, quality, 22);
        writer.write_all(input).unwrap();
    }
    out
}

fn brotli_decompress(input: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    brotli::Decompressor::new(input, 1 << 16)
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn codecs() -> Vec<Codec> {
    vec![
        Codec {
            name: "lz4_flex",
            compress: |input| lz4_flex::block::compress(input),
            decompress: |input, len| lz4_flex::block::decompress(input, len).unwrap(),
        },
        Codec {
            name: "snappy",
            compress: |input| snap::raw::Encoder::new().compress_vec(input).unwrap(),
            decompress: |input, _| snap::raw::Decoder::new().decompress_vec(input).unwrap(),
        },
        Codec {
            name: "zstd -1",
            compress: |input| zstd_compress(1, input),
            decompress: zstd_decompress,
        },
        Codec {
            name: "zstd -3",
            compress: |input| zstd_compress(3, input),
            decompress: zstd_decompress,
        },
        Codec {
            name: "zstd -9",
            compress: |input| zstd_compress(9, input),
            decompress: zstd_decompress,
        },
        Codec {
            name: "zstd -19",
            compress: |input| zstd_compress(19, input),
            decompress: zstd_decompress,
        },
        Codec {
            name: "gzip -1",
            compress: |input| gzip_compress(1, input),
            decompress: gzip_decompress,
        },
        Codec {
            name: "gzip -6",
            compress: |input| gzip_compress(6, input),
            decompress: gzip_decompress,
        },
        Codec {
            name: "brotli -5",
            compress: |input| brotli_compress(5, input),
            decompress: brotli_decompress,
        },
        Codec {
            name: "brotli -11",
            compress: |input| brotli_compress(11, input),
            decompress: brotli_decompress,
        },
    ]
}

fn best_of<F: FnMut()>(mut f: F) -> Duration {
    let started = Instant::now();
    let mut best = Duration::MAX;
    loop {
        let start = Instant::now();
        f();
        best = best.min(start.elapsed());
        if started.elapsed() >= MIN_TIME {
            return best;
        }
    }
}

fn mib_per_sec(bytes: usize, elapsed: Duration) -> f64 {
    bytes as f64 / (1024.0 * 1024.0) / elapsed.as_secs_f64()
}

fn main() {
    let only: Option<Vec<String>> = std::env::var("NAPKIN_COMPRESSION_CODECS")
        .ok()
        .map(|s| s.split(',').map(|c| c.trim().to_string()).collect());

    println!("| file | MiB | codec | ratio | compress MiB/s | decompress MiB/s |");
    println!("| ---- | --- | ----- | ----- | -------------- | ---------------- |");
    for path in std::env::args().skip(1) {
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
            let compressed = (codec.compress)(&input);
            assert_eq!((codec.decompress)(&compressed, input.len()), input);
            let compress = best_of(|| {
                std::hint::black_box((codec.compress)(&input));
            });
            let decompress = best_of(|| {
                std::hint::black_box((codec.decompress)(&compressed, input.len()));
            });
            println!(
                "| {} | {:.0} | {} | {:.2} | {:.0} | {:.0} |",
                file,
                input.len() as f64 / (1024.0 * 1024.0),
                codec.name,
                input.len() as f64 / compressed.len() as f64,
                mib_per_sec(input.len(), compress),
                mib_per_sec(input.len(), decompress),
            );
        }
    }
}
