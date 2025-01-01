use std::time::Duration;

use anyhow::Result;
use knaster_core::{
    osc::SinNumeric,
    typenum::{U0, U2},
    wrappers_core::{GenWrapperExt, WrSmoothParams},
    Gen, ParameterSmoothing,
};
use knaster_graph::{
    audio_backend::{
        cpal::{CpalBackend, CpalBackendOptions},
        AudioBackend,
    },
    connectable::Connectable,
    graph::GraphSettings,
    handle::HandleTrait,
    runner::Runner,
};

fn main() -> Result<()> {
    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut graph, runner) = Runner::<f32>::new::<U0, U2>(GraphSettings {
        name: "TopLevelGraph".to_owned(),
        block_size: backend.block_size().unwrap_or(64),
        sample_rate: backend.sample_rate(),
        ring_buffer_size: 200,
    });
    backend.start_processing(runner)?;
    // push some nodes
    let mut osc1 = WrSmoothParams::new(SinNumeric::new());
    osc1.param(graph.ctx(), "freq", 200.)?;
    let osc1 = graph.push(osc1.wr_mul(0.2));
    osc1.set(("freq", 250.))?;
    let mut osc2 = SinNumeric::new();
    osc2.param(graph.ctx(), "freq", 300.)?;
    let osc2 = graph.push(osc2.wr_mul(0.2));
    let osc3 = graph.push(SinNumeric::new().wr_mul(0.2));
    osc3.set(("freq", 200. * 4.))?;
    // connect them together
    graph.connect(osc1.to(graph.output()))?;
    graph.connect(osc3.add_to(graph.output()))?;
    graph.connect(osc2.add_to(graph.output()))?;
    graph.commit_changes()?;

    let mut freq = 200.;
    for _ in 0..5 {
        osc1.set(("freq", freq, ParameterSmoothing::Linear(0.5)))?;
        osc2.set(("freq", (freq * (5. / 4.)), ParameterSmoothing::Linear(0.5)))?;
        osc3.set(("freq", (freq * (3. / 2.))))?;
        freq *= 1.5;
        std::thread::sleep(Duration::from_secs_f32(1.));
    }
    std::thread::sleep(Duration::from_secs_f32(20.));
    Ok(())
}
