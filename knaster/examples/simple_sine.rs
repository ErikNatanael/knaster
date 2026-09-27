//! # Simple sine
//!
//! This example shows how to create a simple sine tone and play it back, with a linearly interpolated amplitude.
//!
//! ## Required features:
//! - `std`
//! - `cpal`

use color_eyre::Result;
use knaster::preludef32::*;
fn main() -> Result<()> {
    let mut graph = knaster::knaster().start()?;
    let (mut freq, mut amp) = graph.edit(|g| {
        let mut sine = g.push(SinWt::new(440.0));
        let mut amp = g.push(Constant::new(0.2).smooth_params());
        {
            // Create routing handles, which are Copy and more ergonomic to use for routing
            let sine = sine.route();
            let amp = amp.route();
            // Output sine multiplied by amp in stereo to the graph outputs
            let sig = sine * amp;
            sig.out([0, 0]).to_graph_out();
        }
        (sine.control("freq"), amp.control(0))
    });
    // Linearly interpolate to new amplitude values from the previous value over 0.1 seconds
    amp.smooth(ControlSmoothing::Linear(0.1))?;
    for i in 0..11 {
        // Play rising frequencies
        freq.set(440.0 + i as PFloat * 44.0)?;
        // Create a crescendo
        amp.set((i + 1) as PFloat / 20.0)?;
        std::thread::sleep(std::time::Duration::from_secs_f32(0.25));
    }
    std::thread::sleep(std::time::Duration::from_secs(2));
    Ok(())
}
