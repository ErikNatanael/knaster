use criterion::{black_box, criterion_group, criterion_main, Criterion};
use knaster_benchmarks::TestNumUGen;
use knaster_graph::math::{MathUGen, Mul};
use knaster_graph::runner::{Runner, RunnerOptions};
use knaster_graph::typenum::*;
use knaster_graph::wrappers_core::UGenWrapperCoreExt;
use knaster_graph::Block;

pub fn criterion_benchmark(c: &mut Criterion) {
    let block_size = 32;
    let (mut graph, mut runner) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });
    let g = graph.push(TestNumUGen::new(2.0).wr_mul(0.5));
    graph.connect_node_to_output(&g, 0, 0, false).unwrap();
    graph.commit_changes().unwrap();
    c.bench_function("wr_mul block: 32", |b| {
        b.iter(|| {
            unsafe { runner.run(&[]) };
            black_box(runner.output_block().channel_as_slice_mut(0));
            assert_eq!(
                runner.output_block().channel_as_slice_mut(0)[block_size - 1],
                1.0
            );
        })
    });
    let (mut graph, mut runner) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });
    let g = graph.push(TestNumUGen::new(2.0));
    let v = graph.push(TestNumUGen::new(0.5));
    let m = graph.push(MathUGen::<_, U1, Mul>::new());
    graph.connect_nodes(&g, &m, 0, 0, false).unwrap();
    graph.connect_nodes(&v, &m, 0, 1, false).unwrap();
    graph.connect_node_to_output(&m, 0, 0, false).unwrap();
    graph.commit_changes().unwrap();
    c.bench_function("MathGen Mul block: 32", |b| {
        b.iter(|| {
            unsafe { runner.run(&[]) };
            black_box(runner.output_block().channel_as_slice_mut(0));
            assert_eq!(
                runner.output_block().channel_as_slice_mut(0)[block_size - 1],
                1.0
            );
        })
    });
    // 100 nodes
    let (mut graph, mut runner) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });
    for _ in 0..100 {
        let g = graph.push(TestNumUGen::new(2.0).wr_mul(0.5));
        graph.connect_node_to_output(&g, 0, 0, true).unwrap();
    }
    graph.commit_changes().unwrap();
    c.bench_function("100 wr_mul block: 32", |b| {
        b.iter(|| {
            unsafe { runner.run(&[]) };
            black_box(runner.output_block().channel_as_slice_mut(0));
            assert_eq!(
                runner.output_block().channel_as_slice_mut(0)[block_size - 1],
                100.0
            );
        })
    });
    let (mut graph, mut runner) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });
    for _ in 0..100 {
        let g = graph.push(TestNumUGen::new(2.0));
        let v = graph.push(TestNumUGen::new(0.5));
        let m = graph.push(MathUGen::<_, U1, Mul>::new());
        graph.connect_nodes(&g, &m, 0, 0, false).unwrap();
        graph.connect_nodes(&v, &m, 0, 1, false).unwrap();
        graph.connect_node_to_output(&m, 0, 0, true).unwrap();
    }
    graph.commit_changes().unwrap();
    c.bench_function("100 MathGen Mul block: 32", |b| {
        b.iter(|| {
            unsafe { runner.run(&[]) };
            black_box(runner.output_block().channel_as_slice_mut(0));
            assert_eq!(
                runner.output_block().channel_as_slice_mut(0)[block_size - 1],
                100.0
            );
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
