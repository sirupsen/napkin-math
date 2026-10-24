use criterion::*;
use std::hint::black_box;
use prost::Message;

use super::workload::{serialization_corpus, BenchPayload};

fn serialization_benchmark(c: &mut Criterion) {
    let corpus = serialization_corpus();
    let payload = corpus.payload;
    let protobuf = corpus.protobuf;
    let json = corpus.json;

    let mut group = c.benchmark_group("serialization");

    {
        let mut buffer = Vec::with_capacity(protobuf.len());
        group.throughput(Throughput::Bytes(protobuf.len() as u64));
        group.bench_function("protobuf/serialize", |b| {
            b.iter(|| {
                buffer.clear();
                payload.encode(&mut buffer).unwrap();
                black_box(buffer.len())
            })
        });
    }

    group.throughput(Throughput::Bytes(protobuf.len() as u64));
    group.bench_function("protobuf/deserialize", |b| {
        b.iter(|| black_box(BenchPayload::decode(protobuf.as_slice()).unwrap()))
    });

    {
        let mut buffer = Vec::with_capacity(json.len());
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_function("json/serialize", |b| {
            b.iter(|| {
                buffer.clear();
                serde_json::to_writer(&mut buffer, &payload).unwrap();
                black_box(buffer.len())
            })
        });
    }

    group.throughput(Throughput::Bytes(json.len() as u64));
    group.bench_function("json/deserialize", |b| {
        b.iter(|| black_box(serde_json::from_slice::<BenchPayload>(&json).unwrap()))
    });

    // Into a dynamic tree, like JSON.parse in Ruby, Python or JavaScript.
    group.throughput(Throughput::Bytes(json.len() as u64));
    group.bench_function("json/deserialize to Value", |b| {
        b.iter(|| black_box(serde_json::from_slice::<serde_json::Value>(&json).unwrap()))
    });

    // A compact binary format that writes fields in order with no tags.
    let postcard = postcard::to_allocvec(&payload).unwrap();
    {
        let mut buffer = Vec::with_capacity(postcard.len());
        group.throughput(Throughput::Bytes(postcard.len() as u64));
        group.bench_function("postcard/serialize", |b| {
            b.iter(|| {
                buffer.clear();
                buffer = postcard::to_extend(&payload, std::mem::take(&mut buffer)).unwrap();
                black_box(buffer.len())
            })
        });
    }

    group.throughput(Throughput::Bytes(postcard.len() as u64));
    group.bench_function("postcard/deserialize", |b| {
        b.iter(|| black_box(postcard::from_bytes::<BenchPayload>(&postcard).unwrap()))
    });

    group.finish();
}

criterion_group!(benches, serialization_benchmark);
