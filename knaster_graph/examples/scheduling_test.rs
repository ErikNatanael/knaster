// TODO: Used for debugging, remove before release

use std::time::Duration;

use anyhow::Result;
use knaster_core::envelopes::EnvAsr;
use knaster_core::osc::SinWt;
use knaster_core::typenum::{U0, U1, U2};
use knaster_graph::Time;
use knaster_graph::runner::RunnerOptions;
use knaster_graph::{
    audio_backend::{
        AudioBackend,
        cpal::{CpalBackend, CpalBackendOptions},
    },
    graph::GraphOptions,
    runner::Runner,
};

fn main() -> Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut top_level_graph, runner, _log_receiver) =
        Runner::<f32>::new::<U0, U2>(RunnerOptions {
            block_size: backend.block_size().unwrap_or(64),
            sample_rate: backend.sample_rate(),
            ring_buffer_size: 200,
            ..Default::default()
        });
    backend.start_processing(runner)?;
    let mut graph = top_level_graph.edit(|g| {
        let (gh, graph) = g.subgraph::<U0, U1>(GraphOptions::default(), |_| ());
        gh.out([0, 0]).to_graph_out();
        graph
    });
    let mut t_restart = graph.edit(|g| {
        let env = g.push(EnvAsr::new(0.01, 0.2));
        let sine = g.push(SinWt::new(200.));
        (env * sine).to_graph_out();
        env.param("t_restart")
    });
    // push some nodes
    loop {
        for _ in 0..100 {
            t_restart.trig_time(Time::asap()).unwrap();
        }

        std::thread::sleep(Duration::from_secs_f32(1.0));
    }
}
