//! # Buffer Player
//!
//! This example shows how to use the [`BufferReader`] UGen to play a stereo buffer.
//!
//! If you want to load your own sound file, uncomment line 39 and comment out line 41.
use std::f64::consts::{PI, TAU};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use knaster::buffer::BufferReader;
use knaster::dsp::buffer::Buffer;
use knaster::processor::AudioProcessorOptions;
use knaster::typenum::{U0, U2};
use knaster::util::Constant;
use knaster::{
    audio_backend::{
        AudioBackend,
        cpal::{CpalBackend, CpalBackendOptions},
    },
    processor::AudioProcessor,
};

fn main() -> Result<()> {
    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut top_level_graph, audio_processor, _log_receiver) =
        AudioProcessor::<f64>::new::<U0, U2>(AudioProcessorOptions {
            block_size: backend.block_size().unwrap_or(64),
            sample_rate: backend.sample_rate(),
            ring_buffer_size: 200,
            ..Default::default()
        });
    #[allow(unused)]
    let sr = backend.sample_rate() as f64;
    backend.start_processing(audio_processor)?;
    // load a stereo sound file
    let buffer = load_buffer_from_file()?;
    // Generate a stereo buffer out of sweeping sine tones
    // let buffer = generate_stereo_buffer(sr);
    let g = &mut top_level_graph;
    let (mut t_restart, mut loop_param, mut start_secs, mut end_secs) = g.edit(|g| {
        let play = g.push(BufferReader::<_, U2>::new(Arc::new(buffer), 1.0, false));
        let amp = g.push(Constant::new(0.5));
        (play * amp.out([0, 0])).to_graph_out();
        // g.connect(&amp, [0, 0], [2, 3], &mult)?;
        // g.connect(&play, [0, 1], [0, 1], &mult)?;
        // g.connect(&mult, [0, 1], [0, 1], g.internal())?;
        // g.commit_changes()?;
        (
            play.param("t_restart"),
            play.param("looping"),
            play.param("start_s"),
            play.param("end_s"),
        )
    });
    std::thread::sleep(Duration::from_secs_f32(2.5));
    t_restart.trig().unwrap();
    loop_param.set(true).unwrap();
    // play.change("t_restart")?.trig().send()?;
    // play.change("loop")?.value(1).send()?;
    std::thread::sleep(Duration::from_secs_f32(3.9));
    loop_param.set(false).unwrap();
    std::thread::sleep(Duration::from_secs_f32(4.));
    t_restart.trig().unwrap();
    loop_param.set(true).unwrap();
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

#[allow(unused)]
fn generate_stereo_buffer(sr: f64) -> Buffer<f64> {
    let samples: Vec<_> = (0..(sr as usize))
        .flat_map(|i| {
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
        .map(|v| v as f64)
        .collect();
    let samples = samples
        .iter()
        .copied()
        .chain(samples.iter().rev().copied())
        .collect();
    println!("Generated a 1 second long stereo buffer");
    Buffer::from_vec_interleaved(samples, 2, sr)
}
#[allow(unused)]
fn load_buffer_from_file() -> anyhow::Result<Buffer<f64>> {
    let path = rfd::FileDialog::new()
        .add_filter("audio file", &["wav", "flac", "mp3", "ogg", "aiff"])
        .pick_file()
        .ok_or(anyhow::anyhow!("Failed to pick a sound file"))?;
    let path = path.to_str().unwrap();
    let buffer = Buffer::from_sound_file(path)?;
    println!(
        "Loaded a {} second long buffer from {path:?}",
        buffer.length_seconds()
    );
    Ok(buffer)
}
