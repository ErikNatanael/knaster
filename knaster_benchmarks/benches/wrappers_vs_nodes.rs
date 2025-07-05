use criterion::{Criterion, criterion_group, criterion_main};
use knaster::Block;
use knaster::processor::{AudioProcessor, AudioProcessorOptions};
use knaster::typenum::*;
use knaster::wrappers_core::UGenWrapperCoreExt;
use knaster_benchmarks::TestNumUGen;
use std::hint::black_box;

pub fn criterion_benchmark(c: &mut Criterion) {
    let block_size = 32;
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
    graph.edit(|g| {
        g.push(TestNumUGen::new(2.0).wr_mul(0.5)).to_graph_out();
    });
    c.bench_function("wr_mul block: 32", |b| {
        b.iter(|| {
            unsafe { audio_processor.run(&[]) };
            black_box(audio_processor.output_block().channel_as_slice_mut(0));
            assert_eq!(
                audio_processor.output_block().channel_as_slice_mut(0)[block_size - 1],
                1.0
            );
        })
    });
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
    graph.edit(|graph| {
        let g = graph.push(TestNumUGen::new(2.0));
        let v = graph.push(TestNumUGen::new(0.5));
        (g * v).to_graph_out();
        // graph.connect(&g, 0, 0, &m).unwrap();
        // graph.connect(&v, 0, 1, &m).unwrap();
        // graph.connect_node_to_output(&m, 0, 0, false).unwrap();
        // graph.commit_changes().unwrap();
    });
    c.bench_function("MathGen Mul block: 32", |b| {
        b.iter(|| {
            unsafe { audio_processor.run(&[]) };
            black_box(audio_processor.output_block().channel_as_slice_mut(0));
            assert_eq!(
                audio_processor.output_block().channel_as_slice_mut(0)[block_size - 1],
                1.0
            );
        })
    });
    // 100 nodes
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
    graph.edit(|g| {
        for _ in 0..100 {
            g.push(TestNumUGen::new(2.0).wr_mul(0.5)).to_graph_out();
        }
    });
    // for _ in 0..100 {
    //     let g = graph.push(TestNumUGen::new(2.0).wr_mul(0.5));
    //     graph.connect_node_to_output(&g, 0, 0, true).unwrap();
    // }
    // graph.commit_changes().unwrap();
    c.bench_function("100 wr_mul block: 32", |b| {
        b.iter(|| {
            unsafe { audio_processor.run(&[]) };
            black_box(audio_processor.output_block().channel_as_slice_mut(0));
            assert_eq!(
                audio_processor.output_block().channel_as_slice_mut(0)[block_size - 1],
                100.0
            );
        })
    });
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
    graph.edit(|graph| {
        for _ in 0..100 {
            let g = graph.push(TestNumUGen::new(2.0));
            let v = graph.push(TestNumUGen::new(0.5));
            (g * v).to_graph_out();
        }
        // let m = graph.push(MathUGen::<_, U1, Mul>::new());
        // graph.connect(&g, 0, 0, &m).unwrap();
        // graph.connect(&v, 0, 1, &m).unwrap();
        // graph.connect_node_to_output(&m, 0, 0, true).unwrap();
        // graph.commit_changes().unwrap();
    });
    c.bench_function("100 MathGen Mul block: 32", |b| {
        b.iter(|| {
            unsafe { audio_processor.run(&[]) };
            black_box(audio_processor.output_block().channel_as_slice_mut(0));
            assert_eq!(
                audio_processor.output_block().channel_as_slice_mut(0)[block_size - 1],
                100.0
            );
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
