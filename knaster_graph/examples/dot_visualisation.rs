use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::Result;
use knaster_core::{
    osc::SinNumeric,
    typenum::{U0, U2},
    wrappers_core::{UGenWrapperCoreExt, WrSmoothParams},
    ParameterSmoothing, UGen,
};
use knaster_graph::connectable::Sink;
use knaster_graph::runner::RunnerOptions;
use knaster_graph::{
    audio_backend::{
        cpal::{CpalBackend, CpalBackendOptions},
        AudioBackend,
    },
    handle::HandleTrait,
    runner::Runner,
};

fn main() -> Result<()> {
    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut graph, runner) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
        block_size: backend.block_size().unwrap_or(64),
        sample_rate: backend.sample_rate(),
        ring_buffer_size: 200,
    });
    backend.start_processing(runner)?;
    // push some nodes
    let mut osc1 = WrSmoothParams::new(SinNumeric::new(200.));
    osc1.param(graph.ctx(), "freq", 200.)?;
    let osc1 = graph.push(osc1.wr_mul(0.2));
    osc1.set(("freq", 250.))?;
    let mut osc2 = SinNumeric::new(250.);
    osc2.param(graph.ctx(), "freq", 300.)?;
    let osc2 = graph.push(osc2.wr_mul(0.2));
    let osc3 = graph.push(SinNumeric::new(200. * 4.).wr_mul(0.2));
    osc3.set(("freq", 200. * 4.))?;
    // connect them together
    graph.connect_replace(&osc1, 0, 0, Sink::Graph)?;
    graph.connect_node_to_output(&osc1, 0, 1, false)?;
    graph.connect(&osc3, 0, 0, Sink::Graph)?;
    graph.connect(&osc2, 0, 0, Sink::Graph)?;
    graph.commit_changes()?;

    let inspection = graph.inspection();
    let dot_string = inspection.to_dot_string();
    println!("{}", dot_string);
    fs::write("graph.dot", &dot_string)?;
    let mut dot_command = Command::new("dot")
        .arg("-Tsvg")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdin = dot_command.stdin.take().expect("Failed to open stdin");
    std::thread::spawn(move || {
        stdin.write_all(dot_string.as_bytes()).unwrap();
    });
    let output = dot_command.wait_with_output().unwrap();
    fs::write("graph.svg", output.stdout).unwrap();
    open::that("graph.svg").unwrap();

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
