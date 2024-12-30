
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use knaster_benchmarks::{add, add_chunked};

pub fn criterion_benchmark(c: &mut Criterion) {
    let block = 16;
    let signal = vec![1.0; block];
    let mut output = vec![0.0; block];
    c.bench_function("add block: 16", |b| b.iter(|| {
        add(&signal, 3., &mut output);
        black_box(&mut output);
    }));
    let signal = vec![1.0; block];
    let mut output = vec![0.0; block];
    c.bench_function("add_chunked block: 16", |b| b.iter(|| {
        add_chunked(&signal, 3., &mut output);
        black_box(&mut output);
    }));
    let block = 64;
    let signal = vec![1.0; block];
    let mut output = vec![0.0; block];
    c.bench_function("add block: 64", |b| b.iter(|| {
        add(&signal, 3., &mut output);
        black_box(&mut output);
    }));
    let signal = vec![1.0; block];
    let mut output = vec![0.0; block];
    c.bench_function("add_chunked block: 64", |b| b.iter(|| {
        add_chunked(&signal, 3., &mut output);
        black_box(&mut output);
    }));
    let block = 256;
    let signal = vec![1.0; block];
    let mut output = vec![0.0; block];
    c.bench_function("add block: 256", |b| b.iter(|| {
        add(&signal, 3., &mut output);
        black_box(&mut output);
    }));
    let signal = vec![1.0; block];
    let mut output = vec![0.0; block];
    c.bench_function("add_chunked block: 256", |b| b.iter(|| {
        add_chunked(&signal, 3., &mut output);
        black_box(&mut output);
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
