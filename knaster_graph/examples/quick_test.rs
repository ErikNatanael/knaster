use std::time::Duration;

use anyhow::Result;
use knaster_core::envelopes::EnvAsr;
use knaster_core::log::ArLogSender;
use knaster_core::math::{MathUGen, Mul};
use knaster_core::noise::{RandomLin, WhiteNoise};
use knaster_core::onepole::OnePoleHpf;
use knaster_core::{AudioCtx, Done, Seconds};
use knaster_core::{
    UGen,
    osc::SinNumeric,
    typenum::{U0, U1, U2},
    wrappers_core::{UGenWrapperCoreExt, WrSmoothParams},
};
use knaster_graph::graph::GraphError;
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
use rand::Rng;

fn main() -> Result<()> {
    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut top_level_graph, runner, log_receiver) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
        block_size: backend.block_size().unwrap_or(64),
        sample_rate: backend.sample_rate(),
        ring_buffer_size: 200,
        ..Default::default()
    });
    backend.start_processing(runner)?;
    // push some nodes
    loop {
        let mut graph = top_level_graph.edit(|g| {
            let (gh, graph) = g.subgraph::<U0, U1>(GraphOptions::default(), |_| ());
            gh.out([0, 0]).to_graph_out();
            graph
        });

        let mut ctx = AudioCtx::new(
            graph.sample_rate(),
            graph.block_size(),
            ArLogSender::non_rt(),
        );
        let ctx = &mut ctx;
        graph.edit(|graph| -> Result<(), GraphError> {
            let mut rng = rand::rng();
            let freq = rng.random_range(200.0..800.);
            dbg!(freq);
            let mut osc1 = WrSmoothParams::new(SinNumeric::new(freq));
            osc1.param(ctx, "freq", freq)?;
            let osc1 = graph.push(osc1.wr_mul(0.2));
            osc1.param("freq").set(freq)?;
            let mut osc2 = SinNumeric::new(freq * 1.5);
            osc2.param(ctx, "freq", freq * 1.5)?;
            let osc2 = graph.push(osc2.wr_mul(0.2));
            let osc3 = graph.push(SinNumeric::new(freq * 4.).wr_mul(0.2));
            osc3.param("freq").set(freq * 4.)?;
            let env = graph.push_with_done_action(EnvAsr::new(0.2, 0.2), Done::FreeParent);
            env.param("attack_time").set(0.2)?;
            env.param("release_time").set(0.2)?;
            env.param("t_restart").set(knaster_graph::PTrigger)?;
            env.param("t_release")
                .trig_after(Seconds::from_secs_f64(0.5));
            let mult = graph.push(MathUGen::<_, U1, Mul>::new());
            let modulator = graph.push(SinNumeric::new(0.5).wr_powi(2).wr_mul(5000.).wr_add(freq));
            modulator.param("freq").set(0.5)?;
            let random_lin_modulator =
                graph.push(RandomLin::new(4.0).wr_powi(2).wr_mul(5000.).wr_add(100.));
            random_lin_modulator.param("freq").set(4.0)?;
            let lpf = graph.push(OnePoleHpf::new().ar_params());
            // graph.connect_node_to_parameter(&modulator, &lpf, 0, "cutoff_freq", false)?;
            lpf.link("cutoff_freq", random_lin_modulator);
            // graph.connect_to_parameter(&random_lin_modulator, 0, "cutoff_freq", &lpf)?;
            let noise = graph.push(WhiteNoise::new().wr_mul(0.2));
            // let noise = graph.push(PinkNoise::new().wr_mul(0.2));
            // let noise = graph.push(BrownNoise::new().wr_mul(0.2));

            // connect them together
            (((osc1 + osc2 + osc3 + noise) >> lpf) * env)
                .out([0, 0])
                .to_graph_out();
            // graph.connect(&osc1, 0, 0, &lpf)?;
            // graph.connect(&osc3, 0, 0, &lpf)?;
            // graph.connect(&osc2, 0, 0, &lpf)?;
            // graph.connect(&noise, 0, 0, &lpf)?;
            // graph.connect_replace(&lpf, 0, 0, &mult)?;
            // graph.connect_replace(&env, 0, 1, &mult)?;
            // graph.connect_replace(&mult, [0, 0], [0, 1], graph.internal())?;
            // graph.commit_changes()?;
            Ok(())
        })?;
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
