use std::time::Duration;

use anyhow::Result;
use knaster_core::envelopes::EnvAsr;
use knaster_core::math::{MathGen, Mul};
use knaster_core::noise::{RandomLin, WhiteNoise};
use knaster_core::onepole::OnePoleHpf;
use knaster_core::{
    osc::SinNumeric,
    typenum::{U0, U1, U2},
    wrappers_core::{GenWrapperCoreExt, WrSmoothParams},
    Gen,
};
use knaster_core::{Done, Seconds};
use knaster_graph::connectable::Sink;
use knaster_graph::runner::RunnerOptions;
use knaster_graph::{
    audio_backend::{
        cpal::{CpalBackend, CpalBackendOptions},
        AudioBackend,
    },
    graph::GraphOptions,
    handle::HandleTrait,
    runner::Runner,
};
use rand::{thread_rng, Rng};

fn main() -> Result<()> {
    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut top_level_graph, runner) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
        block_size: backend.block_size().unwrap_or(64),
        sample_rate: backend.sample_rate(),
        ring_buffer_size: 200,
    });
    backend.start_processing(runner)?;
    // push some nodes
    loop {
        let mut graph = top_level_graph.subgraph::<U0, U1>(GraphOptions::default());
        top_level_graph.connect_node_to_output(&graph, 0, 0, true)?;
        top_level_graph.connect_node_to_output(&graph, 0, 1, true)?;

        top_level_graph.commit_changes()?;
        let mut rng = thread_rng();
        let freq = rng.gen_range(200.0..800.);
        dbg!(freq);
        let mut osc1 = WrSmoothParams::new(SinNumeric::new(freq));
        osc1.param(graph.ctx(), "freq", freq)?;
        let osc1 = graph.push(osc1.wr_mul(0.2));
        osc1.set(("freq", freq))?;
        let mut osc2 = SinNumeric::new(freq * 1.5);
        osc2.param(graph.ctx(), "freq", freq * 1.5)?;
        let osc2 = graph.push(osc2.wr_mul(0.2));
        let osc3 = graph.push(SinNumeric::new(freq * 4.).wr_mul(0.2));
        osc3.set(("freq", freq * 4.))?;
        let env = graph.push_with_done_action(EnvAsr::new(), Done::FreeParent);
        env.set(("attack_time", 0.2))?;
        env.set(("release_time", 0.2))?;
        env.set(("t_restart", knaster_graph::Trigger))?;
        env.change("t_release")?
            .trig()
            .time(Seconds::from_seconds_f64(0.5));
        let mult = graph.push(MathGen::<_, U1, Mul>::new());
        let modulator = graph.push(SinNumeric::new(0.5).wr_powi(2).wr_mul(5000.).wr_add(freq));
        modulator.set(("freq", 0.5))?;
        let random_lin_modulator =
            graph.push(RandomLin::new().wr_powi(2).wr_mul(5000.).wr_add(100.));
        random_lin_modulator.set(("freq", 4.0))?;
        let lpf = graph.push(OnePoleHpf::new().ar_params());
        // graph.connect_node_to_parameter(&modulator, &lpf, 0, "cutoff_freq", false)?;
        graph.connect_node_to_parameter(&random_lin_modulator, &lpf, 0, "cutoff_freq", false)?;
        let noise = graph.push(WhiteNoise::new().wr_mul(0.2));
        // let noise = graph.push(PinkNoise::new().wr_mul(0.2));
        // let noise = graph.push(BrownNoise::new().wr_mul(0.2));

        // connect them together
        graph.connect_add(&osc1, 0, 0, &lpf)?;
        graph.connect_add(&osc3, 0, 0, &lpf)?;
        graph.connect_add(&osc2, 0, 0, &lpf)?;
        graph.connect_add(&noise, 0, 0, &lpf)?;
        graph.connect(&lpf, 0, 0, &mult)?;
        graph.connect(&env, 0, 1, &mult)?;
        graph.connect(&mult, [0, 0], [0, 1], Sink::Graph)?;
        graph.commit_changes()?;

        // let inspection = top_level_graph.inspection();
        // let dot_string = inspection.to_dot_string();
        // let mut dot_command = Command::new("dot")
        //     .arg("-Tsvg")
        //     .stdin(Stdio::piped())
        //     .stdout(Stdio::piped())
        //     .spawn()?;
        // let mut stdin = dot_command.stdin.take().expect("Failed to open stdin");
        // std::thread::spawn(move || {
        //     stdin.write_all(dot_string.as_bytes()).unwrap();
        // });
        // let output = dot_command.wait_with_output().unwrap();
        // std::fs::write("graph.svg", output.stdout).unwrap();
        // open::that("graph.svg").unwrap();

        std::thread::sleep(Duration::from_secs_f32(1.0));
    }

    // let mut freq = 200.;
    // for _ in 0..5 {
    //     osc1.set(("freq", freq, ParameterSmoothing::Linear(0.5)))?;
    //     osc2.set(("freq", (freq * (5. / 4.)), ParameterSmoothing::Linear(0.5)))?;
    //     osc3.set(("freq", (freq * (3. / 2.))))?;
    //     freq *= 1.5;
    //     std::thread::sleep(Duration::from_secs_f32(1.));
    // }
    // std::thread::sleep(Duration::from_secs_f32(20.));
    // Ok(())
}
