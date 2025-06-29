use std::f64::consts::{PI, TAU};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use knaster_core::buffer::BufferReader;
use knaster_core::dsp::buffer::Buffer;
use knaster_core::math::{MathUGen, Mul};
use knaster_core::typenum::{U0, U2};
use knaster_core::util::Constant;
use knaster_graph::runner::RunnerOptions;
use knaster_graph::{
    audio_backend::{
        AudioBackend,
        cpal::{CpalBackend, CpalBackendOptions},
    },
    handle::HandleTrait,
    runner::Runner,
};

fn main() -> Result<()> {
    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut top_level_graph, runner, log_receiver) = Runner::<f64>::new::<U0, U2>(RunnerOptions {
        block_size: backend.block_size().unwrap_or(64),
        sample_rate: backend.sample_rate(),
        ring_buffer_size: 200,
        ..Default::default()
    });
    dbg!(backend.sample_rate());
    let sr = backend.sample_rate() as f64;
    let sr = 57000.;
    backend.start_processing(runner)?;
    // load a stereo sound file
    let samples: Vec<_> = (0..(sr as usize))
        .map(|i| {
            let phase0 = i as f64 * (200. + i as f64 * 0.01) / sr;
            let phase1 = i as f64 * 250. / sr;
            let phase2 = i as f64 * 0.5 / sr;
            let amp = (phase2 * TAU).sin();
            let amp1 = (phase2 * TAU + PI).sin();
            [
                (phase0 * TAU).sin() as f32 * amp as f32,
                (phase1 * TAU).sin() as f32 * amp1 as f32,
            ]
        })
        .flatten()
        .map(|v| v as f64)
        .collect();
    let samples = samples
        .iter()
        .copied()
        .chain(samples.iter().rev().copied())
        .collect();
    let buffer = Buffer::from_vec_interleaved(samples, 2, sr);
    // buffer.save_to_disk("./stereo_sines.wav").unwrap();
    // let buffer = Buffer::from_sound_file("./stereo_sines.wav").unwrap();
    let g = &mut top_level_graph;
    let (mut t_restart, mut loop_param, mut start_secs, mut end_secs) = g.edit(|g| {
        let play = g.push(BufferReader::<_, U2>::new(Arc::new(buffer), 1.0, false));
        let mult = g.push(MathUGen::<_, U2, Mul>::new());
        let amp = g.push(Constant::new(0.5));
        (play * amp.out([0, 0])).to_graph_out();
        // g.connect(&amp, [0, 0], [2, 3], &mult)?;
        // g.connect(&play, [0, 1], [0, 1], &mult)?;
        // g.connect(&mult, [0, 1], [0, 1], g.internal())?;
        // g.commit_changes()?;
        (
            play.param("t_restart"),
            play.param("loop"),
            play.param("start_secs"),
            play.param("end_secs"),
        )
    });
    std::thread::sleep(Duration::from_secs_f32(2.5));
    t_restart.trig().unwrap();
    loop_param.set(1).unwrap();
    // play.change("t_restart")?.trig().send()?;
    // play.change("loop")?.value(1).send()?;
    std::thread::sleep(Duration::from_secs_f32(3.9));
    loop_param.set(0).unwrap();
    std::thread::sleep(Duration::from_secs_f32(4.));
    t_restart.trig().unwrap();
    loop_param.set(1).unwrap();
    start_secs.set(0.1).unwrap();
    end_secs.set(0.9).unwrap();
    // play.change("t_restart")?.trig().send()?;
    // play.change("loop")?.value(1).send()?;
    // play.change("start_secs")?.value(0.1).send()?;
    // play.change("end_secs")?.value(0.9).send()?;
    std::thread::sleep(Duration::from_secs_f32(2.));
    start_secs.set(0.3).unwrap();
    end_secs.set(0.5).unwrap();
    // play.change("start_secs")?.value(0.3).send()?;
    // play.change("end_secs")?.value(0.5).send()?;
    std::thread::sleep(Duration::from_secs_f32(1.));
    start_secs.set(1.4)?;
    end_secs.set(1.5)?;
    // play.change("start_secs")?.value(1.4).send()?;
    // play.change("end_secs")?.value(1.5).send()?;
    std::thread::sleep(Duration::from_secs_f32(1.));
    start_secs.set(0.9)?;
    end_secs.set(1.1)?;
    // play.change("start_secs")?.value(0.9).send()?;
    // play.change("end_secs")?.value(1.1).send()?;
    std::thread::sleep(Duration::from_secs_f32(1.));
    start_secs.set(0.95)?;
    end_secs.set(1.05)?;
    // play.change("start_secs")?.value(0.95).send()?;
    // play.change("end_secs")?.value(1.05).send()?;
    std::thread::sleep(Duration::from_secs_f32(1.));
    start_secs.set(0.975)?;
    end_secs.set(1.025)?;
    // play.change("start_secs")?.value(0.975).send()?;
    // play.change("end_secs")?.value(1.025).send()?;
    std::thread::sleep(Duration::from_secs_f32(1.));
    Ok(())
}
