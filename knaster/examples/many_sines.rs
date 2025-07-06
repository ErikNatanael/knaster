//! # Many sines
//! 
//! This example creates 600 sine tones with individual envelopes. The frequencies of these sine tones
//! are then moved to be more and more harmonious, sometimes changing the root frequency.
//! 
//! ## Required features:
//! - `std` 
//! - `cpal`


use std::time::Duration;

use anyhow::Result;
use knaster::envelopes::EnvAr;
use knaster::osc::SinWt;
use knaster::pan::Pan2;
use knaster::{
    typenum::{U0, U2},
    wrappers_core::UGenWrapperCoreExt,
};
use knaster_graph::processor::AudioProcessorOptions;
use knaster_graph::{
    audio_backend::{
        AudioBackend,
        cpal::{CpalBackend, CpalBackendOptions},
    },
    processor::AudioProcessor,
};
use rand::Rng;
use rand::seq::{IndexedMutRandom, IndexedRandom};

fn main() -> Result<()> {
    let mut backend = CpalBackend::new(CpalBackendOptions::default())?;

    // Create a graph
    let (mut top_level_graph, audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U2>(AudioProcessorOptions {
            block_size: backend.block_size().unwrap_or(64),
            sample_rate: backend.sample_rate(),
            ring_buffer_size: 200,
            ..Default::default()
        });
    // Start processing audio using the backend. The outputs of the `top_level_graph` will be
    // connected to your OS default audio output.
    backend.start_processing(audio_processor)?;
    // push some nodes
    let g = &mut top_level_graph;
    let mut freqs = vec![];
    let mut envs = vec![];
    let mut rng = rand::rng();

    g.edit(|g| {
        for _i in 0..600 {
            let env = EnvAr::new(0.01, 0.1);
            let env = g.push(env);
            let sine = g.push(
                SinWt::new(rng.random_range(3000.0..10000.0)).wr_mul(rng.random_range(0.01..0.015)),
            );
            let pan = g.push(Pan2::new(rng.random_range(-1.0..1.0)));
            ((env * sine) >> pan).to_graph_out();
            envs.push(env.param("t_restart"));
            freqs.push(sine.param("freq"));
        }
    });
    let mut loops = 0;
    let mut root = 110.0;
    let ratios = [1.0, 9. / 8., 6. / 5., 3. / 2., 8. / 5., 16. / 9., 2.];
    loop {
        // envs.shuffle(&mut rng);
        let mut j = 0;
        while j < envs.len() {
            if loops % 16 == 0 {
                let ratios = [1.0, 9. / 8., 6. / 5., 3. / 2., 8. / 5., 16. / 9., 2.];
                root =
                    55.0 * 2.0_f32.powi(rng.random_range(1..4)) * ratios.choose(&mut rng).unwrap();
            }
            // Set the frequency of the sine tone
            let freq_param = &mut freqs[j];
            let freq = root * ratios[j % ratios.len()];
            freq_param.set(freq)?;
            // Send a trigger to the envelope of this sine tone to restart.
            let env = &mut envs[j];
            env.trig()?;
            // Trigger some other random envelope as well
            let env = envs.choose_mut(&mut rng).unwrap();
            env.trig()?;
            j += rng.random_range(0..10);
            std::thread::sleep(Duration::from_secs_f32(0.01));
        }
        let num = envs.len() - 1;
        for _ in 0..4 {
            envs.swap(rng.random_range(0..num), rng.random_range(0..num));
        }
        loops += 1;
    }
}
