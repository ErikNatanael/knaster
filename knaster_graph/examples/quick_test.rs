use std::time::Duration;

use anyhow::Result;
use knaster_core::{
    osc::SinTrig,
    typenum::{U0, U2},
};
use knaster_graph::{
    audio_backend::{
        cpal::{CpalBackend, CpalBackendOptions},
        AudioBackend,
    },
    connectable::Connectable,
    graph::GraphSettings,
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
    let osc1 = graph.push(SinTrig::new())?;
    let osc2 = graph.push(SinTrig::new())?;
    // connect them together
    graph.connect(osc1.to(graph.output()));
    graph.connect(osc2.to(graph.output()));

    std::thread::sleep(Duration::from_secs_f32(20.));
    Ok(())
}
