use knaster_core::typenum::*;
use knaster_core_dsp::wrappers_core::UGenWrapperCoreExt;

use crate::{
    Time,
    connection::{ConnectionOptions, Sink, Source},
    core::vec::Vec,
    handle::HandleTrait,
    inspection::EdgeSource,
    processor::{AudioProcessor, AudioProcessorOptions},
    tests::utils::TestInPlusParamUGen,
};

#[test_log::test]
fn test_graph_inspection() {
    let block_size = 2;
    let (mut g, _audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
    let nodes = (0..10)
        .map(|_| g.push_internal(TestInPlusParamUGen::new()))
        .collect::<Vec<_>>();
    for n in nodes.iter() {
        g.set(n, 0, 0.1, Time::asap()).unwrap();
    }
    let n1 = g.push(TestInPlusParamUGen::new());
    g.set(&n1, 0, 0.5, Time::asap()).unwrap();
    let n2 = g.push(TestInPlusParamUGen::new().ar_params());
    g.connect(
        Source::node(&n1, 0),
        Sink::param(&n2, 0),
        ConnectionOptions::default(),
    )
    .unwrap();
    let n3 = g.push(TestInPlusParamUGen::new().ar_params());
    for n in nodes.iter() {
        g.connect(
            Source::node(n, 0),
            Sink::param(&n3, 0),
            ConnectionOptions::default(),
        )
        .unwrap();
    }

    g.connect(
        Source::node(&n2, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    let inspection = g.inspection();
    // Check that the graph inspection produced matches the expected structure.
    // 13 explicit nodes + 9 add nodes
    assert_eq!(inspection.nodes.len(), 13 + 9);
    let n1_inspection = inspection.get_node(n1.node_id().key()).unwrap();
    assert_eq!(n1_inspection.inputs, 1);
    assert_eq!(n1_inspection.outputs, 1);
    assert_eq!(n1_inspection.input_edges.len(), 0);
    assert_eq!(n1_inspection.input_parameter_edges.len(), 0);
    assert_eq!(n1_inspection.parameter_descriptions.len(), 1);
    assert_eq!(n1_inspection.parameter_hints.len(), 1);
    assert_eq!(n1_inspection.unconnected, false);
    assert_eq!(n1_inspection.is_graph, None);

    let n2_inspection = inspection.get_node(n2.node_id().key()).unwrap();
    let n2_key = n2_inspection.key;
    assert_eq!(n2_inspection.input_edges.len(), 0);
    assert_eq!(n2_inspection.input_parameter_edges.len(), 1);
    assert!(
        inspection
            .graph_output_edges
            .iter()
            .find(|e| matches!(e.source, EdgeSource::Node(n) if n == n2_key))
            .is_some()
    );
}
