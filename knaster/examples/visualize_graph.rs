//! # Graph visualisation
//!
//! This example shows how to use the [`Inspection`] API to generate a graph visualisation. The
//! graph visualisation will be opened in your default svg viewer.
//!
//! ## Required features:
//! - `std`
//! - `cpal`
//!
//! Prerequisites:
//! - Install graphviz for the `dot` program and make sure it is in your path

use std::time::Duration;

use anyhow::Result;
use knaster::AudioCtx;
use knaster::log::ArLogSender;
use knaster::processor::AudioProcessorOptions;
use knaster::{
    ParameterSmoothing, UGen,
    osc::SinNumeric,
    typenum::{U0, U2},
    wrappers_core::{UGenWrapperCoreExt, WrSmoothParams},
};
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
    let (mut graph, audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U2>(AudioProcessorOptions {
            block_size: backend.block_size().unwrap_or(64),
            sample_rate: backend.sample_rate(),
            ring_buffer_size: 200,
            ..Default::default()
        });
    backend.start_processing(audio_processor)?;
    // push some nodes
    let mut osc1 = WrSmoothParams::new(SinNumeric::new(200.));
    let mut ctx = AudioCtx::new(
        graph.sample_rate(),
        graph.block_size(),
        ArLogSender::non_rt(),
    );
    let ctx = &mut ctx;
    osc1.param(ctx, "freq", 200.)?;
    let (mut osc1_freq, mut osc2_freq, mut osc3_freq) = graph.edit(|graph| {
        let osc1 = graph.push(osc1.wr_mul(0.2).smooth_params());
        osc1.param("freq").set(250.).unwrap();
        let mut osc2 = SinNumeric::new(250.);
        osc2.param(ctx, "freq", 300.).unwrap();
        let osc2 = graph.push(osc2.wr_mul(0.2).smooth_params());
        let osc3 = graph.push(SinNumeric::new(200. * 4.).wr_mul(0.2));
        osc3.param("freq").set(200. * 4.).unwrap();
        // connect them together
        osc1.out([0, 0]).to_graph_out();
        osc3.to_graph_out();
        osc2.to_graph_out();
        // graph.connect_replace(&osc1, 0, 0, graph.internal())?;
        // graph.connect(&osc1, 0, 1, graph.internal())?;
        // graph.connect(&osc3, 0, 0, graph.internal())?;
        // graph.connect(&osc2, 0, 0, graph.internal())?;
        (osc1.param("freq"), osc2.param("freq"), osc3.param("freq"))
    });

    // Generate an "inspection", turn that into an SVG of the graph structure, and open the SVG
    graph.inspection().show_dot_svg();

    let mut freq = 200.;
    for _ in 0..5 {
        osc1_freq.smooth(ParameterSmoothing::Linear(0.5))?;
        osc1_freq.set(freq)?;
        osc2_freq.smooth(ParameterSmoothing::Linear(0.5))?;
        osc2_freq.set(freq * (5. / 4.))?;
        osc3_freq.set(freq * (3. / 2.))?;
        // osc1.set(("freq", freq, ParameterSmoothing::Linear(0.5)))?;
        // osc2.set(("freq", (freq * (5. / 4.)), ParameterSmoothing::Linear(0.5)))?;
        // osc3.set(("freq", (freq * (3. / 2.))))?;
        freq *= 1.5;
        std::thread::sleep(Duration::from_secs_f32(1.));
    }
    std::thread::sleep(Duration::from_secs_f32(20.));
    Ok(())
}
