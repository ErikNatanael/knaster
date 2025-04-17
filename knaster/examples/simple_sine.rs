use std::time::Duration;

use color_eyre::Result;
use knaster::preludef32::*;
fn main() -> Result<()> {
    let mut graph = knaster::knaster().start()?;
    let (mut freq, mut amp) = graph.edit(|g| {
        let sine = g.push(SinWt::new(440.0));
        let amp = g.push(Constant::new(0.2).smooth_params());
        let sig = sine * amp;
        sig.out([0, 0]).to_graph_out();
        (sine.param("freq").unwrap(), amp.param(0).unwrap())
    });
    // Linearly interpolate to new amplitude values from the previous value over 0.1 seconds
    amp.smooth(ParameterSmoothing::Linear(0.1), Time::asap())?;
    for i in 0..11 {
        // Play rising frequencies
        freq.set(440.0 + i as f32 * 44.0, Time::asap())?;
        // Flip between 0.1 and 0.5 every other cycle
        amp.set(if i % 2 == 0 { 0.1 } else { 0.5 }, Time::asap())?;
        std::thread::sleep(Duration::from_secs_f32(0.25));
    }
    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}
