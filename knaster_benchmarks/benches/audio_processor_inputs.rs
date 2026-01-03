use criterion::{Criterion, criterion_group, criterion_main};
use knaster::{
    Block, StaticBlock, VecBlock,
    prelude::{AudioProcessor, AudioProcessorOptions},
    typenum::*,
};
use knaster_benchmarks::{add, add_chunked};
use std::hint::black_box;

pub fn criterion_benchmark(c: &mut Criterion) {
    let block_size = 128;
    let num_inputs = 4;

    let mut input_block = VecBlock::new(num_inputs as usize, block_size);
    let mut input_block_pointers = Vec::with_capacity(num_inputs as usize);
    for i in 0..num_inputs {
        input_block_pointers.push(input_block.channel_as_slice(i as usize).as_ptr());
    }
    let (_graph, mut audio_processor, _logger) =
        AudioProcessor::<f32>::new::<U4, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
    c.bench_function("audio_processor raw input", |b| {
        b.iter(|| {
            black_box(&mut input_block);
            unsafe {
                audio_processor.run_raw_ptr_inputs(&input_block_pointers);
            }
            black_box(audio_processor.output_block());
        })
    });

    let mut input_block = [[0.0f32; 128]; 4];

    c.bench_function("audio_processor slice input", |b| {
        b.iter(|| {
            black_box(&mut input_block);
            let input_references: [&[f32]; 4] = [
                &input_block[0],
                &input_block[1],
                &input_block[2],
                &input_block[3],
            ];
            audio_processor.run(&input_references);
            black_box(audio_processor.output_block());
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
