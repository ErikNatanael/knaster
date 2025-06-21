use std::time::Duration;

use anyhow::Result;
use knaster_core::envelopes::EnvAsr;
use knaster_core::math::{MathUGen, Mul};
use knaster_core::typenum::U1;
use knaster_core::{
    Done,
    osc::SinNumeric,
    typenum::{U0, U2},
    wrappers_core::{UGenWrapperCoreExt, WrSmoothParams},
};
use knaster_graph::graph_edit::Parameter;
use knaster_graph::runner::RunnerOptions;
use knaster_graph::{
    audio_backend::{
        AudioBackend,
        cpal::{CpalBackend, CpalBackendOptions},
    },
    graph::GraphOptions,
    handle::HandleTrait,
    runner::Runner,
};

fn main() -> Result<()> {
    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut top_graph, runner, log_receiver) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
        block_size: backend.block_size().unwrap_or(64),
        sample_rate: backend.sample_rate(),
        ring_buffer_size: 200,
    });
    backend.start_processing(runner)?;
    // let mut nodes = vec![];
    let mut second_graph = top_graph.edit(|graph| {
        let (gh, graph) = graph.subgraph::<U0, U1>(GraphOptions::default(), |_| {});
        gh.out([0, 0]).to_graph_out();
        graph
    });
    // let mut second_graph = top_graph.subgraph::<U0, U1>(GraphOptions::default());
    // top_graph.connect(&second_graph, [0, 0], [0, 1], top_graph.internal())?;
    // top_graph.commit_changes()?;
    std::thread::spawn(move || -> Result<()> {
        let mut i = 0;
        let mut previous_asr: Option<Parameter> = None;
        let mut freq = 50.;
        let mut attack_time_phase: f64 = 0.0;
        let mut release_time_phase: f64 = 0.0;
        loop {
            if let Some(mut asr) = previous_asr.take() {
                asr.trig()?;
            }
            // push some nodes

            let graph = second_graph.edit(|graph| {
                let (gh, graph) = graph.subgraph::<U0, U1>(GraphOptions::default(), |g| {
                    let osc1 = WrSmoothParams::new(SinNumeric::new(freq * (i + 1) as f32));
                    let osc1 = graph.push(osc1.wr_mul(0.05));
                    osc1.param("freq").set(freq * (i + 1) as f32).unwrap();
                    let attack_time = (attack_time_phase.sin() * 0.1).abs();
                    let release_time = release_time_phase.sin() * 2. + 2.1;
                    let asr = graph.push_with_done_action(
                        EnvAsr::new(attack_time as f32, release_time as f32),
                        Done::FreeParent,
                    );
                    attack_time_phase += 0.001;
                    asr.param("attack_time").set(attack_time).unwrap();
                    release_time_phase += 0.00013;
                    asr.param("release_time").set(release_time).unwrap();
                    asr.param("t_restart").trig().unwrap();
                    previous_asr = Some(asr.param("t_release"));
                    let mult = graph.push(MathUGen::<_, U1, Mul>::new());
                    // connect them together
                    (osc1 * asr).to_graph_out();
                    // graph.connect(&osc1, 0, 0, &mult)?;
                    // graph.connect(&asr, 0, 1, &mult)?;
                    // graph.connect(&mult, 0, 0, graph.internal())?;
                    // graph.commit_changes()?;
                });
                gh.to_graph_out();
                graph
            });
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
}
