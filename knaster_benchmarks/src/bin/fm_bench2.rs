use std::time::Duration;

use color_eyre::Result;
use knaster::{audio_backend::jack::JackBackend, preludef32::*};
pub fn main() -> Result<()> {
    // let mut backend = CpalBackend::new(CpalBackendOptions::default())?;
    let mut backend = JackBackend::new("fm bench")?;

    // Create a graph
    let (mut graph, audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U2>(AudioProcessorOptions {
            block_size: backend.block_size().unwrap_or(64),
            sample_rate: backend.sample_rate(),
            ring_buffer_size: 200,
            ..Default::default()
        });
    backend.start_processing(audio_processor)?;
    graph.edit(|g| {
        let mut last: Option<DH<_, _>> = None;
        for i in 0..1024 {
            let c = g.push(Constant::new(0.05));
            let s = g.push(SinWt::new(220. + i as f32));
            if let Some(l) = last.take() {
                let add: DH<_, _> = l.clone() * 440.0;
                let mul: DH<_, _> = s.dynamic() * l;
                let node: DH<_, _> = mul + add;
                node.clone().to_graph_out();
                last = Some(node * c.dynamic());
            } else {
                (c * s).to_graph_out();
                last = Some(s.dynamic().to_channels_handle());
            }
        }
        // graph.connect(&g, 0, 0, &m).unwrap();
        // graph.connect(&v, 0, 1, &m).unwrap();
        // graph.connect_node_to_output(&m, 0, 0, false).unwrap();
        // graph.commit_changes().unwrap();
    });
    loop {
        std::thread::sleep(Duration::from_secs_f32(1.));
    }
}
