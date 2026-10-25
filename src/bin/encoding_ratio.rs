// Compression ratios for structured data: columnar tables, inverted indexes,
// time series and embeddings. Every ratio is against the naive in-memory
// representation (Arrow buffers, 4 bytes per doc ID, 16 bytes per point, 4
// bytes per dimension), not against a text export.
//
//   cargo run --release --features encodings --bin encoding_ratio -- parquet hits_0.parquet
//   cargo run --release --features encodings --bin encoding_ratio -- postings enwik8
//   cargo run --release --features encodings --bin encoding_ratio -- timeseries nab/*.csv
//   cargo run --release --features encodings --bin encoding_ratio -- vectors embeddings.json
use std::collections::HashMap;
use std::fs::File;

use arrow_array::{Array, RecordBatch};
use bitpacking::{BitPacker, BitPacker4x};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

fn report(name: &str, naive: usize, size: usize) {
    println!(
        "  {:<34} {:>12} bytes  {:>6.1}x",
        name,
        size,
        naive as f64 / size as f64
    );
}

fn parquet(path: &str) {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap())
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<RecordBatch> = reader.map(|b| b.unwrap()).collect();
    let rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    let naive: usize = batches
        .iter()
        .flat_map(|b| b.columns().iter().map(|c| c.get_buffer_memory_size()))
        .sum();
    let schema = batches[0].schema();
    println!(
        "{path}: {rows} rows, {} columns, {naive} bytes in Arrow",
        schema.fields().len()
    );

    let write = |props: WriterProperties| {
        let mut out = Vec::new();
        let mut writer = ArrowWriter::try_new(&mut out, schema.clone(), Some(props)).unwrap();
        for batch in &batches {
            writer.write(batch).unwrap();
        }
        writer.close().unwrap();
        out.len()
    };
    let plain = WriterProperties::builder()
        .set_dictionary_enabled(false)
        .set_compression(Compression::UNCOMPRESSED)
        .build();
    let encoded = WriterProperties::builder()
        .set_compression(Compression::UNCOMPRESSED)
        .build();
    let zstd = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap()))
        .build();
    report("parquet plain", naive, write(plain));
    report("parquet dictionary + RLE", naive, write(encoded));
    report("parquet dictionary + RLE + zstd -3", naive, write(zstd));
}

// enwik8 is a Wikipedia XML dump; each <page> is a document. Only doc IDs are
// indexed, no term frequencies or positions.
fn postings(path: &str) {
    let text = std::fs::read(path).unwrap();
    let text = String::from_utf8_lossy(&text);
    let mut index: HashMap<String, Vec<u32>> = HashMap::new();
    let mut docs = 0u32;
    for page in text.split("<page>").skip(1) {
        for word in page
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
        {
            let ids = index.entry(word.to_lowercase()).or_default();
            if ids.last() != Some(&docs) {
                ids.push(docs);
            }
        }
        docs += 1;
    }
    let postings: usize = index.values().map(|ids| ids.len()).sum();
    let naive = postings * 4;
    println!(
        "{path}: {docs} documents, {} terms, {postings} postings",
        index.len()
    );

    let varint = |mut v: u32| {
        let mut n = 1;
        while v >= 0x80 {
            v >>= 7;
            n += 1;
        }
        n
    };
    let packer = BitPacker4x::new();
    let (mut bitpacked, mut delta_varint, mut roaring) = (0, 0, 0);
    for ids in index.values() {
        delta_varint += ids
            .iter()
            .scan(0, |prev, &id| {
                let delta = id - *prev;
                *prev = id;
                Some(varint(delta))
            })
            .sum::<usize>();

        // 128-ID blocks: one byte of bit width plus the packed deltas; the
        // tail that doesn't fill a block is delta + varint, like Lucene.
        let mut prev = 0;
        let mut blocks = ids.chunks_exact(BitPacker4x::BLOCK_LEN);
        for block in &mut blocks {
            let bits = packer.num_bits_sorted(prev, block);
            bitpacked += 1 + BitPacker4x::compressed_block_size(bits);
            prev = *block.last().unwrap();
        }
        for &id in blocks.remainder() {
            bitpacked += varint(id - prev);
            prev = id;
        }

        roaring += roaring::RoaringBitmap::from_sorted_iter(ids.iter().copied())
            .unwrap()
            .serialized_size();
    }
    let mut raw = Vec::with_capacity(naive);
    for ids in index.values() {
        raw.extend(ids.iter().flat_map(|id| id.to_le_bytes()));
    }
    report(
        "zstd -3 of raw u32s",
        naive,
        zstd::bulk::compress(&raw, 3).unwrap().len(),
    );
    report("delta + varint", naive, delta_varint);
    report("delta + bitpacking (128 blocks)", naive, bitpacked);
    report("roaring", naive, roaring);
}

// Days since 1970-01-01 (Howard Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

// "YYYY-MM-DD HH:MM:SS,value" lines, as in the Numenta Anomaly Benchmark.
fn read_series(path: &str) -> Vec<(i64, f64)> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .skip(1)
        .map(|line| {
            let (ts, value) = line.split_once(',').unwrap();
            let n = |r: std::ops::Range<usize>| ts[r].parse::<i64>().unwrap();
            let secs = days_from_civil(n(0..4), n(5..7), n(8..10)) * 86400
                + n(11..13) * 3600
                + n(14..16) * 60
                + n(17..19);
            (secs, value.parse().unwrap())
        })
        .collect()
}

// Size in bits of the Gorilla encoding (Pelkonen et al., VLDB 2015):
// delta-of-delta timestamps and XORed float64 values.
fn gorilla_bits(points: &[(i64, f64)]) -> usize {
    let mut bits = 64 + 64;
    let (mut prev_ts, mut prev_delta) = (points[0].0, 0i64);
    let mut prev_value = points[0].1.to_bits();
    let (mut prev_leading, mut prev_trailing) = (u32::MAX, 0);
    for (i, &(ts, value)) in points.iter().enumerate().skip(1) {
        let delta = ts - prev_ts;
        if i == 1 {
            bits += 14;
        } else {
            bits += match delta - prev_delta {
                0 => 1,
                -63..=64 => 2 + 7,
                -255..=256 => 3 + 9,
                -2047..=2048 => 4 + 12,
                _ => 4 + 32,
            };
        }
        (prev_ts, prev_delta) = (ts, delta);

        let xor = value.to_bits() ^ prev_value;
        prev_value = value.to_bits();
        if xor == 0 {
            bits += 1;
            continue;
        }
        let leading = xor.leading_zeros().min(31);
        let trailing = xor.trailing_zeros();
        if prev_leading != u32::MAX && leading >= prev_leading && trailing >= prev_trailing {
            bits += 2 + (64 - prev_leading - prev_trailing) as usize;
        } else {
            bits += 2 + 5 + 6 + (64 - leading - trailing) as usize;
            (prev_leading, prev_trailing) = (leading, trailing);
        }
    }
    bits
}

fn timeseries(paths: &[String]) {
    let (mut total_naive, mut total_gorilla, mut total_zstd) = (0, 0, 0);
    for path in paths {
        let points = read_series(path);
        let naive = points.len() * 16;
        let gorilla = gorilla_bits(&points).div_ceil(8);
        // Column-wise: timestamp deltas, then values, zstd'd together.
        let mut raw = Vec::with_capacity(naive);
        let mut prev = 0;
        for &(ts, _) in &points {
            raw.extend((ts - prev).to_le_bytes());
            prev = ts;
        }
        for &(_, value) in &points {
            raw.extend(value.to_le_bytes());
        }
        let zstd = zstd::bulk::compress(&raw, 3).unwrap().len();
        println!(
            "{path}: {} points, gorilla {:.1}x ({:.1} B/point), delta + zstd -3 {:.1}x",
            points.len(),
            naive as f64 / gorilla as f64,
            gorilla as f64 / points.len() as f64,
            naive as f64 / zstd as f64
        );
        total_naive += naive;
        total_gorilla += gorilla;
        total_zstd += zstd;
    }
    println!("all series: {total_naive} bytes as 16 B/point");
    report("gorilla", total_naive, total_gorilla);
    report("delta + zstd -3", total_naive, total_zstd);
}

fn vectors(path: &str) {
    let vectors: Vec<Vec<f32>> =
        serde_json::from_reader(std::io::BufReader::new(File::open(path).unwrap())).unwrap();
    let dims = vectors[0].len();
    let raw: Vec<u8> = vectors
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let naive = raw.len();
    println!("{path}: {} vectors of {dims} f32 dimensions", vectors.len());
    // Byte-shuffle: all first bytes, then all second bytes, ... so the
    // exponent bytes sit together, like Blosc's shuffle filter.
    let mut shuffled = Vec::with_capacity(naive);
    for byte in 0..4 {
        shuffled.extend(raw.iter().skip(byte).step_by(4));
    }
    report(
        "zstd -3",
        naive,
        zstd::bulk::compress(&raw, 3).unwrap().len(),
    );
    report(
        "zstd -19",
        naive,
        zstd::bulk::compress(&raw, 19).unwrap().len(),
    );
    report(
        "byte shuffle + zstd -3",
        naive,
        zstd::bulk::compress(&shuffled, 3).unwrap().len(),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: encoding_ratio parquet|postings|timeseries|vectors <files...>";
    let (mode, files) = args.split_first().expect(usage);
    match mode.as_str() {
        "parquet" => files.iter().for_each(|f| parquet(f)),
        "postings" => files.iter().for_each(|f| postings(f)),
        "timeseries" => timeseries(files),
        "vectors" => files.iter().for_each(|f| vectors(f)),
        _ => panic!("{usage}"),
    }
}
