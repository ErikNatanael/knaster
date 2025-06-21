use iai::black_box;

use knaster::Block;
use knaster::graph_edit::{DH, SH};
use knaster::osc::SinWt;
use knaster::runner::{Runner, RunnerOptions};
use knaster::typenum::*;
use knaster::util::Constant;

pub fn lai_256_sine_mul_0_05_to_out_block_32() {
    let block_size = 32;
    let (mut graph, mut runner, log_receiver) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });
    graph.edit(|g| {
        for _ in 0..256 {
            let c = g.push(Constant::new(0.05));
            let s = g.push(SinWt::new(440.));
            (c * s).to_graph_out();
        }
    });
    for _ in 0..256 {
        unsafe { runner.run(&[]) };
        black_box(runner.output_block().channel_as_slice_mut(0));
    }
}
pub fn lai_256_fm_cascade_block_32() {
    let block_size = 32;
    let (mut graph, mut runner, log_receiver) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });
    graph.edit(|g| {
        let mut last: Option<DH<_, _>> = None;
        for i in 0..256 {
            let c = g.push(Constant::new(0.05));
            let s = g.push(SinWt::new(220. + i as f32));
            if let Some(l) = last.take() {
                let add: DH<_, _> = l.clone() * 440.0;
                let mul: DH<_, _> = s.dynamic() * l;
                let node: DH<_, _> = mul + add;
                node.clone().to_graph_out();
                last = Some(node * c.dynamic());
            } else {
                (c * s).to_graph_out();
                last = Some(s.dynamic().to_channels_handle());
            }
        }
        // graph.connect(&g, 0, 0, &m).unwrap();
        // graph.connect(&v, 0, 1, &m).unwrap();
        // graph.connect_node_to_output(&m, 0, 0, false).unwrap();
        // graph.commit_changes().unwrap();
    });
    for _ in 0..256 {
        unsafe { runner.run(&[]) };
        black_box(runner.output_block().channel_as_slice_mut(0));
    }
}

iai::main!(
    lai_256_sine_mul_0_05_to_out_block_32,
    lai_256_fm_cascade_block_32
);
