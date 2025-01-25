use std::time::Duration;

use anyhow::Result;
use knaster_core::envelopes::EnvAr;
use knaster_core::math::{MathUGen, Mul};
use knaster_core::osc::SinWt;
use knaster_core::pan::Pan2;
use knaster_core::{
    typenum::{U0, U1, U2},
    wrappers_core::UGenWrapperCoreExt,
};
use knaster_graph::runner::RunnerOptions;
use knaster_graph::{
    audio_backend::{
        cpal::{CpalBackend, CpalBackendOptions},
        AudioBackend,
    },
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
    let g = &mut top_level_graph;
    let mut envs = vec![];
    let mut rng = thread_rng();

    for _i in 0..300 {
        let env = EnvAr::new(0.01, 0.1);
        let env = g.push(env);
        let sine =
            g.push(SinWt::new(rng.gen_range(3000.0..10000.0)).wr_mul(rng.gen_range(0.01..0.015)));
        let mul = g.push(MathUGen::<_, U1, Mul>::new());
        let pan = g.push(Pan2::new(rng.gen_range(-1.0..1.0)));
        g.connect_nodes(&env, &mul, 0, 0, false)?;
        g.connect_nodes(&sine, &mul, 0, 1, false)?;
        g.connect_nodes(&mul, &pan, 0, 0, false)?;
        g.connect_node_to_output(&pan, 0, 0, true)?;
        g.connect_node_to_output(&pan, 1, 1, true)?;
        envs.push(env);
    }
    for _i in 0..300 {
        let env = EnvAr::new(0.01, 0.1);
        let env = g.push(env);
        let sine = g.push(SinWt::new(rng.gen_range(6000.0..6500.0)).wr_mul(0.01));
        let mul = g.push(MathUGen::<_, U1, Mul>::new());
        g.connect_nodes(&env, &mul, 0, 0, false)?;
        g.connect_nodes(&sine, &mul, 0, 1, false)?;
        g.connect_node_to_output(&mul, 0, 0, true)?;
        g.connect_node_to_output(&mul, 0, 1, true)?;
        envs.push(env);
    }
    // let sine = g.push({
    //     let mut s = SinWt::new().wr_mul(0.2);
    //     s.param(g.ctx(), "freq", 440.)?;
    //     s
    // });
    // g.connect_node_to_output(&sine, 0, 0, true);
    // g.connect_node_to_output(&sine, 0, 1, true);
    g.commit_changes()?;
    let mut loops = 0;
    loop {
        // if loops % 16 == 0 {
        //     let ratios = [1.0, 9. / 8., 6. / 5., 3. / 2., 8. / 5., 16. / 9., 2.];
        //     let root = 55.0 * 2.0_f32.powi(rng.gen_range(1..4)) * ratios.choose(&mut rng).unwrap();
        //     for ratio in ratios {
        //         for i in 0..16 {
        //             let mut env = EnvAr::new();
        //             env.param(g.ctx(), "attack_time", 0.001)?;
        //             env.param(g.ctx(), "release_time", 0.9)?;
        //             let env = g.push(env);
        //             let sine = g.push({
        //                 let mut s = SinWt::new().wr_mul(0.05 / ((i + 1) as f32));
        //                 let freq = rng.gen_range(25.0..10000.0);
        //                 let freq = ratio * root * i as f32;
        //                 s.param(g.ctx(), "freq", freq)?;
        //                 s
        //             });
        //             let mul = g.push(MathGen::<_, U1, Mul>::new());
        //             g.connect_nodes(&env, &mul, 0, 0, false)?;
        //             g.connect_nodes(&sine, &mul, 0, 1, false)?;
        //             g.connect_node_to_output(&mul, 0, 0, true)?;
        //             g.connect_node_to_output(&mul, 0, 1, true)?;
        //             envs.push(env);
        //         }
        //     }
        //     g.commit_changes()?;
        // }
        // envs.shuffle(&mut rng);
        let mut j = 0;
        while j < envs.len() {
            j = rng.gen_range(0..envs.len());
            let env = &envs[j];
            // env.change("release_time")?
            //     .value(rng.gen::<f32>().powi(2) * 1.0 + 0.05);
            env.change("t_restart")?.trig();
            j += rng.gen_range(0..10);
            std::thread::sleep(Duration::from_secs_f32(0.01));
        }
        let num = envs.len() - 1;
        for _ in 0..4 {
            envs.swap(rng.gen_range(0..num), rng.gen_range(0..num));
        }
        loops += 1;
    }

    let mut freq = 200.;
    // for _ in 0..5 {
    //     osc1.set(("freq", freq, ParameterSmoothing::Linear(0.5)))?;
    //     osc2.set(("freq", (freq * (5. / 4.)), ParameterSmoothing::Linear(0.5)))?;
    //     osc3.set(("freq", (freq * (3. / 2.))))?;
    //     freq *= 1.5;
    //     std::thread::sleep(Duration::from_secs_f32(1.));
    // }
    std::thread::sleep(Duration::from_secs_f32(20.));
    Ok(())
}
