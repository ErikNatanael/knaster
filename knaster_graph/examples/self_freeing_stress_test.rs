use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::Result;
use knaster_core::envelopes::EnvAsr;
use knaster_core::math::{MathGen, Mul};
use knaster_core::typenum::U1;
use knaster_core::{
    osc::SinNumeric,
    typenum::{U0, U2},
    wrappers_core::{GenWrapperCoreExt, WrSmoothParams},
    Done, Gen, ParameterSmoothing, Trigger,
};
use knaster_graph::handle::AnyHandle;
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
    let (mut top_graph, runner) = Runner::<f32>::new::<U0, U2>(GraphSettings {
        name: "TopLevelGraph".to_owned(),
        block_size: backend.block_size().unwrap_or(64),
        sample_rate: backend.sample_rate(),
        ring_buffer_size: 200,
    });
    backend.start_processing(runner)?;
    // let mut nodes = vec![];
    let mut second_graph = top_graph.subgraph::<U0, U1>(GraphSettings::default());
    top_graph.connect_node_to_output(&second_graph, 0, 0, true)?;
    top_graph.connect_node_to_output(&second_graph, 0, 1, true)?;
    top_graph.commit_changes()?;
    std::thread::spawn(move || -> Result<()> {
        let mut i = 0;
        let mut previous_asr: Option<AnyHandle> = None;
        let mut freq = 50.;
        let mut attack_time_phase: f64 = 0.0;
        let mut release_time_phase: f64 = 0.0;
        loop {
            if let Some(asr) = previous_asr.take() {
                asr.set((EnvAsr::<f32>::T_RELEASE, Trigger))?;
            }
            // push some nodes
            let mut graph = second_graph.subgraph::<U0, U1>(GraphSettings::default());
            second_graph.connect_node_to_output(&graph, 0, 0, true)?;
            second_graph.commit_changes()?;
            let osc1 = WrSmoothParams::new(SinNumeric::new());
            let osc1 = graph.push(osc1.wr_mul(0.05));
            osc1.set(("freq", freq * (i + 1) as f32))?;
            let asr = graph.push_with_done_action(EnvAsr::new(), Done::FreeParent);
            let attack_time = (attack_time_phase.sin() * 0.1).abs();
            attack_time_phase += 0.001;
            asr.set(("attack_time", attack_time)).unwrap();
            let release_time = (release_time_phase.sin() * 2. + 2.1);
            release_time_phase += 0.00013;
            asr.set(("release_time", release_time)).unwrap();
            asr.set(("t_restart", Trigger)).unwrap();
            previous_asr = Some(asr.clone().into_any());
            let mult = graph.push(MathGen::<_, U1, Mul>::new());
            // connect them together
            graph.connect_nodes(&osc1, &mult, 0, 0, false)?;
            graph.connect_nodes(&asr, &mult, 0, 1, false)?;
            graph.connect_node_to_output(&mult, 0, 0, true)?;
            graph.commit_changes()?;
            std::thread::sleep(Duration::from_secs_f32(0.005));
            // asr.set(("t_release", Trigger)).unwrap();
            i += 1;
            if i >= 16 {
                i = 0;
                freq *= 5. / 4.;
                if freq >= 2000. {
                    freq = 25.;
                }
            }
            i %= 24;
            // let inspection = second_graph.inspection();
            // println!("Num_nodes: {:?}", inspection.nodes.len());
            // let dot_string = inspection.to_dot_string();
            // println!("{}", dot_string);
            // let mut dot_command = Command::new("dot").arg("-Tsvg").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
            // let mut stdin = dot_command.stdin.take().expect("Failed to open stdin");
            // std::thread::spawn(move || {
            //     stdin.write_all(dot_string.as_bytes()).unwrap();
            // });
            // let output = dot_command.wait_with_output().unwrap();
            // fs::write("graph.svg", output.stdout).unwrap();
            // open::that("graph.svg").unwrap();
            // std::thread::sleep(Duration::from_secs_f32(0.1));
        }
    });

    loop {
        std::thread::sleep(Duration::from_secs_f32(1.));
    }
    Ok(())
}
