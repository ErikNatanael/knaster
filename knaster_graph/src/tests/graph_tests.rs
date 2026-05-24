use crate::Time;
use crate::connection::{ConnectionOptions, Sink, Source};
use crate::handle::HandleTrait;
use crate::processor::AudioProcessorOptions;
use crate::tests::utils::{TestNumUGen, assert_next_frame_eq_0_0};
use crate::{processor::AudioProcessor, tests::utils::TestInPlusParamUGen};
use knaster_core::typenum::{U0, U1, U2, U4};
use knaster_core::{Block, typenum::U3};
use knaster_core_dsp::math::{Add, MathUGen, Mul};
use knaster_core_dsp::wrappers_core::UGenWrapperCoreExt;
/// no_std_compat prelude import, supporting both std and no_std
use std::prelude::v1::*;

#[test]
fn graph_empty_graph_zero_output() {
    let block_size = 16;
    let (_graph, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 2,
            ..Default::default()
        });
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.0);
}

#[test]
fn graph_empty_graph_zero_output_many_channels() {
    let block_size = 16;
    let (_graph, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U4>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 2,
            ..Default::default()
        });
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.0);
    assert_eq!(output.read(1, 0), 0.0);
    assert_eq!(output.read(2, 0), 0.0);
    assert_eq!(output.read(3, 0), 0.0);
    assert_eq!(output.read(0, 14), 0.0);
    assert_eq!(output.read(1, 13), 0.0);
    assert_eq!(output.read(2, 12), 0.0);
    assert_eq!(output.read(3, 11), 0.0);
}

#[test]
fn graph_inputs_to_outputs() {
    let block_size = 16;
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::new::<U3, U3>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    graph
        .connect(
            Source::graph(1),
            Sink::graph(0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph
        .connect(
            Source::graph(2),
            Sink::graph(1),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph.commit_changes().unwrap();

    let input_allocation = vec![1.0; 16 * 3];
    let input_pointers = [
        input_allocation.as_ptr(),
        unsafe { input_allocation.as_ptr().add(block_size) },
        unsafe { input_allocation.as_ptr().add(block_size * 2) },
    ];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 1.0);
    assert_eq!(output.read(1, 0), 1.0);
    assert_eq!(output.read(2, 0), 0.0);
}

#[test]
fn graph_inputs_to_outputs_graph_edit() {
    let block_size = 16;
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::new::<U3, U3>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    graph.edit(|graph| {
        // Connect input 1 to 0, 2, to 1
        graph.from_inputs(1).unwrap().to_graph_out_channels(0);
        graph.from_inputs(2).unwrap().to_graph_out_channels(1);
    });

    let input_allocation = vec![1.0; 16 * 3];
    let input_pointers = [
        input_allocation.as_ptr(),
        unsafe { input_allocation.as_ptr().add(block_size) },
        unsafe { input_allocation.as_ptr().add(block_size * 2) },
    ];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 1.0);
    assert_eq!(output.read(1, 0), 1.0);
    assert_eq!(output.read(2, 0), 0.0);
}

#[test]
fn graph_inputs_to_parameters() {
    let block_size = 16;
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::new::<U3, U3>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    let n0 = graph.push(TestInPlusParamUGen::new().ar_params());
    let n1 = graph.push(TestInPlusParamUGen::new().ar_params());
    graph
        .connect(
            Source::graph(1),
            Sink::param(n0.node_id(), 0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph
        .connect(
            Source::node(n0.node_id(), 0),
            Sink::graph(1),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph
        .connect(
            Source::graph(2),
            Sink::param(n1.node_id(), 0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph
        .connect(
            Source::node(n1.node_id(), 0),
            Sink::graph(2),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph.commit_changes().unwrap();

    let mut input_allocation = vec![1.0; 16 * 3];
    input_allocation[block_size..block_size * 2].fill(2.);
    input_allocation[block_size * 2..block_size * 3].fill(3.);
    let input_pointers = [
        input_allocation.as_ptr(),
        unsafe { input_allocation.as_ptr().add(block_size) },
        unsafe { input_allocation.as_ptr().add(block_size * 2) },
    ];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.0);
    assert_eq!(output.read(1, 0), 2.0);
    assert_eq!(output.read(2, 0), 3.0);
}

#[test]
fn graph_inputs_to_nodes_to_outputs() {
    let block_size = 16;
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::new::<U3, U3>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
    graph
        .connect(
            Source::graph(0),
            Sink::graph(1),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph
        .connect(
            Source::graph(0),
            Sink::graph(2),
            ConnectionOptions::default(),
        )
        .unwrap();
    let g0 = graph.push(TestInPlusParamUGen::new());
    let g1 = graph.push(TestInPlusParamUGen::new());
    g0.param("number").value(0.75).send().unwrap();
    g1.param("number").value(0.5).send().unwrap();

    graph
        .connect(
            Source::node(g0.node_id(), 0),
            Sink::graph(2),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph
        .connect(
            Source::graph(0),
            Sink::node(g1.node_id(), 0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph
        .connect(
            Source::node(g1.node_id(), 0),
            Sink::graph(0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph.commit_changes().unwrap();

    // Connect input 1 to 0, 2, to 1
    // graph.connect(&g0, 0, 2, graph.internal()).unwrap();
    // graph.connect(graph.internal(), 2, 0, &g1).unwrap();
    // graph.connect(&g1, 0, 0, graph.internal()).unwrap();
    // graph.commit_changes().unwrap();

    let input_allocation = vec![2.0; 16 * 3];
    let input_pointers = [
        input_allocation.as_ptr(),
        unsafe { input_allocation.as_ptr().add(block_size) },
        unsafe { input_allocation.as_ptr().add(block_size * 2) },
    ];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 2.5);
    assert_eq!(output.read(1, 0), 2.0);
    assert_eq!(output.read(2, 0), 2.75);
}

#[test]
fn graph_inputs_to_nodes_to_outputs_graph_edit() {
    let block_size = 16;
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::new::<U3, U3>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    graph.edit(|graph| {
        graph
            .from_inputs([0, 0])
            .unwrap()
            .to_graph_out_channels([1, 2]);
        let g0 = graph.push(TestInPlusParamUGen::new());
        let g1 = graph.push(TestInPlusParamUGen::new());
        g0.param("number").set(0.75).unwrap();
        g1.param("number").set(0.5).unwrap();
        g0.to_graph_out_channels(2);
        graph
            .from_inputs(2)
            .unwrap()
            .to(g1)
            .to_graph_out_channels(0);
    });
    // Connect input 1 to 0, 2, to 1
    // graph.connect(&g0, 0, 2, graph.internal()).unwrap();
    // graph.connect(graph.internal(), 2, 0, &g1).unwrap();
    // graph.connect(&g1, 0, 0, graph.internal()).unwrap();
    // graph.commit_changes().unwrap();

    let input_allocation = vec![2.0; 16 * 3];
    let input_pointers = [
        input_allocation.as_ptr(),
        unsafe { input_allocation.as_ptr().add(block_size) },
        unsafe { input_allocation.as_ptr().add(block_size * 2) },
    ];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 2.5);
    assert_eq!(output.read(1, 0), 2.0);
    assert_eq!(output.read(2, 0), 2.75);
}

#[test]
fn multichannel_nodes() {
    let block_size = 16;
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::new::<U3, U2>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    let (v0_0, _v0_1, v1_0, _v1_1, m) = graph.edit(|graph| {
        let v0_0 = graph.push(TestNumUGen::new(0.125));
        let v0_1 = graph.push(TestNumUGen::new(1.));
        let v1_0 = graph.push(TestNumUGen::new(0.5));
        let v1_1 = graph.push(TestNumUGen::new(4.125));
        // two channel output
        let m = graph.push(MathUGen::<f64, U2, Add>::new());
        (v0_0 | v0_1 | v1_0 | v1_1).to(m).to_graph_out();
        (v0_0.id(), v0_1.id(), v1_0.id(), v1_1.id(), m.id())
    });

    let input_allocation = vec![1.0; 16 * 3];
    let input_pointers = [
        input_allocation.as_ptr(),
        unsafe { input_allocation.as_ptr().add(block_size) },
        unsafe { input_allocation.as_ptr().add(block_size * 2) },
    ];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.625);
    assert_eq!(output.read(1, 0), 5.125);

    graph.edit(|graph| {
        let v0_0 = graph.handle(v0_0).unwrap();
        // let v0_1 = graph.handle(v0_1).unwrap();
        let v1_0 = graph.handle(v1_0).unwrap();
        // let v1_1 = graph.handle(v1_1).unwrap();
        let m = graph.handle(m).unwrap();
        // Change the graph so that the output of m is multiplied by 0.5 and 0.125 respectively, but using two different nodes
        let m2 = graph.push(MathUGen::<f64, U1, Mul>::new()).dynamic();
        let m3 = graph.push(MathUGen::<f64, U1, Mul>::new()).dynamic();
        (m.out(0) | v1_0).to(m2);
        (m.out(1) | v0_0).to(m3);
        (m2 | m3).to_graph_out_replace();

        // graph.connect_replace(&m, 0, 0, &m2).unwrap();
        // graph.connect_replace(&m, 1, 0, &m3).unwrap();
        // graph.connect_replace(&v1_0, 0, 1, &m2).unwrap();
        // graph.connect_replace(&v0_0, 0, 1, &m3).unwrap();
        // // These should replace the previous input edges to the graph outputs
        // graph.connect_replace(&m2, 0, 0, graph.internal()).unwrap();
        // graph.connect_replace(&m3, 0, 1, graph.internal()).unwrap();
    });
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.625 * 0.5);
    assert_eq!(output.read(1, 0), 5.125 * 0.125);
}

#[test]
fn feedback_nodes() {
    let block_size = 16;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    // These are connected in the most common case where a feedback edge is required
    let n0 = g.push(TestInPlusParamUGen::new());
    n0.param(0).value(1.25).send().unwrap();
    let n1 = g.push(TestInPlusParamUGen::new());
    n1.param(0).value(0.125).send().unwrap();

    g.connect(
        Source::node(&n0, 0),
        Sink::node(&n1, 0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.connect(
        Source::node(&n1, 0),
        Sink::node(&n0, 0),
        ConnectionOptions::default().feedback(),
    )
    .unwrap();
    g.connect(
        Source::node(&n1, 0),
        Sink::graph(0),
        ConnectionOptions::default().feedback(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    // Block 1
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 1.375);
    // Block 2
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 1.375 * 2.);
    // Block 3
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 1.375 * 3.);
}

#[test]
fn feedback_nodes_graph_edit() {
    let block_size = 16;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    g.edit(|g| {
        // These are connected in the most common case where a feedback edge is required
        let n0 = g.push(TestInPlusParamUGen::new());
        n0.param(0).set(1.25).unwrap();
        let n1 = g.push(TestInPlusParamUGen::new());
        n1.param(0).set(0.125).unwrap();

        n0.to(n1).to_feedback(n0);
        n1.to_graph_out();
    });

    // Block 1
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 1.375);
    // Block 2
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 1.375 * 2.);
    // Block 3
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 1.375 * 3.);
}

#[test]
fn feedback_nodes2() {
    let block_size = 16;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    g.edit(|g| {
        // These could just as well be connected without feedback edge, but the delay should still be
        // applied
        let n2 = g.push(TestInPlusParamUGen::new());
        n2.param(0).set(1.25).unwrap();
        let n3 = g.push(TestInPlusParamUGen::new());
        n3.param(0).set(0.125).unwrap();
        n2.to_feedback(n3).to_graph_out();
    });

    // Block 1
    assert_next_frame_eq_0_0(&mut audio_processor, 0.125).unwrap();
    // Block 2
    assert_next_frame_eq_0_0(&mut audio_processor, 0.125 + 1.25).unwrap();
    // Block 3
    assert_next_frame_eq_0_0(&mut audio_processor, 0.125 + 1.25).unwrap();
}

#[test_log::test]
fn disconnect() {
    let block_size = 16;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    let n1 = g.push(TestInPlusParamUGen::new());
    g.set(&n1, 0, 0.5, Time::asap()).unwrap();
    let n2 = g.push(TestInPlusParamUGen::new());
    g.set(&n2, 0, 1.25, Time::asap()).unwrap();
    let n3 = g.push(TestInPlusParamUGen::new());
    g.set(&n3, 0, 0.125, Time::asap()).unwrap();
    g.connect(
        Source::node(&n1, 0),
        Sink::node(&n2, 0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.connect(
        Source::node(&n2, 0),
        Sink::node(&n3, 0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.connect(
        Source::node(&n3, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();

    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.5 + 1.25 + 0.125).unwrap();

    g.disconnect_outputs_from_source(Source::node(&n1, 0))
        .unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 1.25 + 0.125).unwrap();

    g.disconnect_inputs_to_sink(Sink::node(&n3, 0)).unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.125).unwrap();

    // Disconnect the n3 node from the graph sink
    g.disconnect(Source::node(&n3, 0), Sink::graph(0)).unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.0).unwrap();
}

/// Test that when several nodes are connected to one output, disconnecting one
/// leaves the others connected.
#[test_log::test]
fn disconnect_summed_nodes() {
    let block_size = 2;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
    let n1 = g.push(TestInPlusParamUGen::new());
    g.set(&n1, 0, 0.5, Time::asap()).unwrap();
    let n2 = g.push(TestInPlusParamUGen::new());
    g.set(&n2, 0, 1.25, Time::asap()).unwrap();
    let n3 = g.push(TestInPlusParamUGen::new());
    g.set(&n3, 0, 0.125, Time::asap()).unwrap();

    g.connect(
        Source::node(&n1, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.connect(
        Source::node(&n2, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.connect(
        Source::node(&n3, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    assert_next_frame_eq_0_0(&mut audio_processor, 0.5 + 1.25 + 0.125).unwrap();

    // Disconnect the n2 node from the graph sink
    g.disconnect(Source::node(&n2, 0), Sink::graph(0)).unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.5 + 0.125).unwrap();

    // Disconnect the n3 node from the graph sink
    g.disconnect_outputs_from_source(Source::node(&n3, 0))
        .unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.5).unwrap();
}

/// Test that nodes can be disconnected from parameters, including when several
/// nodes are summed as the input to one parameter.
#[test_log::test]
fn disconnect_from_parameter() {
    let block_size = 2;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f64>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });
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

    g.connect(
        Source::node(&n2, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.5).unwrap();

    let mut nodes = (0..10)
        .map(|_| g.push_internal(TestInPlusParamUGen::new()))
        .collect::<Vec<_>>();
    for n in nodes.iter() {
        g.set(n, 0, 0.125, Time::asap()).unwrap();
    }
    for n in nodes.iter() {
        g.connect(
            Source::node(n, 0),
            Sink::param(&n3, 0),
            ConnectionOptions::default(),
        )
        .unwrap();
    }

    // Remove the parameter input to n2
    g.disconnect(Source::node(&n1, 0), Sink::param(&n2, 0))
        .unwrap();
    g.commit_changes().unwrap();
    g.set(&n2, 0, 0.0, Time::asap()).unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.0).unwrap();

    // Connect n3 to n2
    g.connect(
        Source::node(&n3, 0),
        Sink::param(&n2, 0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    assert_next_frame_eq_0_0(&mut audio_processor, 0.125 * 10.).unwrap();

    // Disconnect all the nodes from n3 in a roundabout order
    let n = nodes.remove(5);
    g.disconnect(Source::node(&n, 0), Sink::param(&n3, 0))
        .unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.125 * 9.).unwrap();

    let n = nodes.remove(8);
    g.disconnect(Source::node(&n, 0), Sink::param(&n3, 0))
        .unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.125 * 8.).unwrap();

    let n = nodes.remove(0);
    g.disconnect(Source::node(&n, 0), Sink::param(&n3, 0))
        .unwrap();
    g.commit_changes().unwrap();
    assert_next_frame_eq_0_0(&mut audio_processor, 0.125 * 7.).unwrap();
    for (i, n) in nodes.into_iter().enumerate() {
        g.disconnect(Source::node(&n, 0), Sink::param(&n3, 0))
            .unwrap();
        g.commit_changes().unwrap();
        if i == 6 {
            // n3 param 0 has no input anymore, so it retains its last value of 0.125
            assert_next_frame_eq_0_0(&mut audio_processor, 0.125).unwrap();
        } else {
            assert_next_frame_eq_0_0(&mut audio_processor, 0.125 * 6. - (i as f64 * 0.125))
                .unwrap();
        }
    }
}

#[test]
fn graph_inputs_to_nodes() {
    let block_size = 16;
    let (mut graph, mut audio_processor, _log_receiver) =
        AudioProcessor::new::<U1, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    let n1 = graph.push(TestInPlusParamUGen::new().ar_params());

    graph
        .connect(
            Source::graph(0),
            Sink::param(&n1, 0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph
        .connect(
            Source::node(&n1, 0),
            Sink::graph(0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph.commit_changes().unwrap();

    let input_allocation = vec![0.75; 16 * 3];
    let input_pointers = [input_allocation.as_ptr()];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.75);

    graph
        .disconnect(Source::graph(0), Sink::param(&n1, 0))
        .unwrap();
    graph.commit_changes().unwrap();

    let input_allocation = vec![0.5; 16 * 3];
    let input_pointers = [input_allocation.as_ptr()];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    // Since it is disconnected, the input will retain the old value 0.75
    assert_eq!(output.read(0, 0), 0.75);

    // Reconnect
    graph
        .connect(
            Source::graph(0),
            Sink::param(&n1, 0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph.commit_changes().unwrap();

    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.5);

    // Disconnect by disconnect_outputs_from_source
    graph
        .disconnect_outputs_from_source(Source::graph(0))
        .unwrap();
    graph.commit_changes().unwrap();
    graph.set(&n1, 0, 0.1, Time::asap()).unwrap();

    let input_allocation = vec![0.25; 16 * 3];
    let input_pointers = [input_allocation.as_ptr()];
    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.1);

    // Reconnect
    graph
        .connect(
            Source::graph(0),
            Sink::param(&n1, 0),
            ConnectionOptions::default(),
        )
        .unwrap();
    graph.commit_changes().unwrap();

    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.25);

    // Disconnect by disconnect_inputs_to_sink
    graph
        .disconnect_inputs_to_sink(Sink::param(&n1, 0))
        .unwrap();
    graph.commit_changes().unwrap();
    graph.set(&n1, 0, 0.0, Time::asap()).unwrap();

    unsafe { audio_processor.run_raw_ptr_inputs(&input_pointers) };
    let output = audio_processor.output_block();
    assert_eq!(output.read(0, 0), 0.0);
}
#[test]
fn connection_replace_to_node_sink() {
    let block_size = 16;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    let source1 = g.push(TestInPlusParamUGen::new());
    g.set(&source1, 0, 1.0, Time::asap()).unwrap();
    let source2 = g.push(TestInPlusParamUGen::new());
    g.set(&source2, 0, 2.0, Time::asap()).unwrap();
    let sink = g.push(TestInPlusParamUGen::new());

    // Initial connection: source1 -> sink
    g.connect(
        Source::node(&source1, 0),
        Sink::node(&sink, 0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.connect(
        Source::node(&sink, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    assert_next_frame_eq_0_0(&mut audio_processor, 1.0).unwrap();

    // Replace connection with source2 -> sink
    g.connect(
        Source::node(&source2, 0),
        Sink::node(&sink, 0),
        ConnectionOptions::default().replace(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    assert_next_frame_eq_0_0(&mut audio_processor, 2.0).unwrap();
}

#[test]
fn connection_replace_to_graph_sink() {
    let block_size = 16;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    let source1 = g.push(TestInPlusParamUGen::new());
    g.set(&source1, 0, 1.5, Time::asap()).unwrap();
    let source2 = g.push(TestInPlusParamUGen::new());
    g.set(&source2, 0, 3.5, Time::asap()).unwrap();

    // Initial connection: source1 -> graph output 0
    g.connect(
        Source::node(&source1, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    assert_next_frame_eq_0_0(&mut audio_processor, 1.5).unwrap();

    // Replace connection with source2 -> graph output 0
    g.connect(
        Source::node(&source2, 0),
        Sink::graph(0),
        ConnectionOptions::default().replace(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    assert_next_frame_eq_0_0(&mut audio_processor, 3.5).unwrap();
}

#[test]
fn connection_replace_to_param_sink() {
    let block_size = 16;
    let (mut g, mut audio_processor, _log_receiver) =
        AudioProcessor::<f32>::new::<U0, U1>(AudioProcessorOptions {
            block_size,
            sample_rate: 48000,
            ring_buffer_size: 50,
            ..Default::default()
        });

    let source1 = g.push(TestInPlusParamUGen::new());
    g.set(&source1, 0, 0.25, Time::asap()).unwrap();
    let source2 = g.push(TestInPlusParamUGen::new());
    g.set(&source2, 0, 0.75, Time::asap()).unwrap();
    let param_target = g.push(TestInPlusParamUGen::new().ar_params());

    // Initial connection: source1 -> param_target's parameter 0
    g.connect(
        Source::node(&source1, 0),
        Sink::param(&param_target, 0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.connect(
        Source::node(&param_target, 0),
        Sink::graph(0),
        ConnectionOptions::default(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    assert_next_frame_eq_0_0(&mut audio_processor, 0.25).unwrap();

    // Replace connection with source2 -> param_target's parameter 0
    g.connect(
        Source::node(&source2, 0),
        Sink::param(&param_target, 0),
        ConnectionOptions::default().replace(),
    )
    .unwrap();
    g.commit_changes().unwrap();

    assert_next_frame_eq_0_0(&mut audio_processor, 0.75).unwrap();
}
