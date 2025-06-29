use crate::Time;
use crate::connectable::NodeOrGraph;
use crate::runner::RunnerOptions;
use crate::tests::utils::TestNumUGen;
use crate::{handle::HandleTrait, runner::Runner, tests::utils::TestInPlusParamUGen};
use alloc::vec;
use knaster_core::envelopes::EnvAsr;
use knaster_core::math::{Add, MathUGen, Mul};
use knaster_core::typenum::{U0, U1, U2};
use knaster_core::{Block, Done, PTrigger, typenum::U3};

#[test]
fn graph_inputs_to_outputs() {
    let block_size = 16;
    let (mut graph, mut runner, log_receiver) = Runner::new::<U3, U3>(RunnerOptions {
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
    unsafe { runner.run(&input_pointers) };
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 1.0);
    assert_eq!(output.read(1, 0), 1.0);
    assert_eq!(output.read(2, 0), 0.0);
}

#[test]
fn graph_inputs_to_nodes_to_outputs() {
    let block_size = 16;
    let (mut graph, mut runner, log_receiver) = Runner::new::<U3, U3>(RunnerOptions {
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
    unsafe { runner.run(&input_pointers) };
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 2.5);
    assert_eq!(output.read(1, 0), 2.0);
    assert_eq!(output.read(2, 0), 2.75);
}

#[test]
fn multichannel_nodes() {
    let block_size = 16;
    let (mut graph, mut runner, log_receiver) = Runner::new::<U3, U2>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
        ..Default::default()
    });

    let (v0_0, v0_1, v1_0, v1_1, m) = graph.edit(|graph| {
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
    unsafe { runner.run(&input_pointers) };
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 0.625);
    assert_eq!(output.read(1, 0), 5.125);

    graph.edit(|graph| {
        let v0_0 = graph.handle(v0_0).unwrap();
        let v0_1 = graph.handle(v0_1).unwrap();
        let v1_0 = graph.handle(v1_0).unwrap();
        let v1_1 = graph.handle(v1_1).unwrap();
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
    unsafe { runner.run(&input_pointers) };
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 0.625 * 0.5);
    assert_eq!(output.read(1, 0), 5.125 * 0.125);
}

#[test]
fn free_node_when_done() {
    let block_size = 16;
    let (mut graph, mut runner, log_receiver) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
        ..Default::default()
    });
    let asr = graph.push_with_done_action(EnvAsr::new(0.0, 0.0), Done::FreeSelf);
    asr.set(("attack_time", 0.0)).unwrap();
    asr.set(("release_time", 0.0)).unwrap();
    asr.set(("t_restart", PTrigger)).unwrap();
    asr.set(("t_release", PTrigger)).unwrap();
    graph.commit_changes().unwrap();
    assert_eq!(graph.inspection().nodes.len(), 1);
    for _ in 0..10 {
        unsafe {
            runner.run(&[]);
        }
    }
    // Run the code to free old nodes
    graph.commit_changes().unwrap();
    assert_eq!(graph.inspection().nodes.len(), 0);
    assert_eq!(graph.num_nodes_pending_removal(), 1);
    // Apply the new TaskData on the audio thread so that the node can be removed
    unsafe {
        runner.run(&[]);
    }
    // Now the node is removed
    graph.commit_changes().unwrap();
    assert_eq!(graph.num_nodes_pending_removal(), 0);
    assert_eq!(graph.inspection().nodes.len(), 0);
}
#[test]
fn feedback_nodes() {
    let block_size = 16;
    let (mut g, mut runner, log_receiver) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
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
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 1.375);
    // Block 2
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 1.375 * 2.);
    // Block 3
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 1.375 * 3.);
}

#[test]
fn feedback_nodes2() {
    let block_size = 16;
    let (mut g, mut runner, log_receiver) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
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
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 0.125);
    // Block 2
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 0.125 + 1.25);
    // Block 3
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 0.125 + 1.25);
}
#[test]
fn disconnect() {
    let block_size = 16;
    let (mut g, mut runner, _log_receiver) = Runner::<f32>::new::<U0, U1>(RunnerOptions {
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
    g.connect2(&n1, 0, 0, &n2).unwrap();
    g.connect2(&n2, 0, 0, &n3).unwrap();
    g.connect2(&n3, 0, 0, NodeOrGraph::Graph).unwrap();

    g.commit_changes().unwrap();

    // Block 1
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 0.5 + 1.25 + 0.125);

    g.disconnect_output_from_source(&n1, 0).unwrap();
    g.commit_changes().unwrap();

    // Block 2
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 1.25 + 0.125);

    g.disconnect_input_to_sink(0, &n3).unwrap();
    g.commit_changes().unwrap();
    // Block 3
    unsafe {
        runner.run(&[]);
    }
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 0.125);
}
