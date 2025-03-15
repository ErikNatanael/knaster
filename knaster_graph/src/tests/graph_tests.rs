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
    let (mut graph, mut runner) = Runner::new::<U3, U3>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });

    // Connect input 1 to 0, 2, to 1
    graph.connect_input_to_output(1, 0, false).unwrap();
    graph.connect_input_to_output(2, 1, false).unwrap();
    graph.commit_changes().unwrap();

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
    let (mut graph, mut runner) = Runner::new::<U3, U3>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });

    // Connect input 1 to 0, 2, to 1
    graph.connect_input_to_output(0, 1, false).unwrap();
    graph.connect_input_to_output(0, 2, false).unwrap();
    let g0 = graph.push(TestInPlusParamUGen::new());
    let g1 = graph.push(TestInPlusParamUGen::new());
    g0.set(("number", 0.75)).unwrap();
    g1.set(("number", 0.5)).unwrap();
    graph.connect_node_to_output(&g0, 0, 2, true).unwrap();
    graph.connect_input_to_node(&g1, 2, 0, false).unwrap();
    graph.connect_node_to_output(&g1, 0, 0, false).unwrap();
    graph.commit_changes().unwrap();

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
    let (mut graph, mut runner) = Runner::new::<U3, U2>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
    });

    let v0_0 = graph.push(TestNumUGen::new(0.125));
    let v0_1 = graph.push(TestNumUGen::new(1.));
    let v1_0 = graph.push(TestNumUGen::new(0.5));
    let v1_1 = graph.push(TestNumUGen::new(4.125));
    // two channel output
    let m = graph.push(MathUGen::<f64, U2, Add>::new());
    // Connect input 1 to 0, 2, to 1
    graph.connect_nodes(&v0_0, &m, 0, 0, false).unwrap();
    graph.connect_nodes(&v0_1, &m, 0, 1, false).unwrap();
    graph.connect_nodes(&v1_0, &m, 0, 2, false).unwrap();
    graph.connect_nodes(&v1_1, &m, 0, 3, false).unwrap();
    graph.connect_node_to_output(&m, 0, 0, false).unwrap();
    graph.connect_node_to_output(&m, 1, 1, false).unwrap();
    graph.commit_changes().unwrap();

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

    // Change the graph so that the output of m is multiplied by 0.5 and 0.125 respectively, but using two different nodes
    let m2 = graph.push(MathUGen::<f64, U1, Mul>::new());
    let m3 = graph.push(MathUGen::<f64, U1, Mul>::new());
    graph.connect_nodes(&m, &m2, 0, 0, false).unwrap();
    graph.connect_nodes(&m, &m3, 1, 0, false).unwrap();
    graph.connect_nodes(&v1_0, &m2, 0, 1, false).unwrap();
    graph.connect_nodes(&v0_0, &m3, 0, 1, false).unwrap();
    // These should replace the previous input edges to the graph outputs
    graph.connect_node_to_output(&m2, 0, 0, false).unwrap();
    graph.connect_node_to_output(&m3, 0, 1, false).unwrap();
    graph.commit_changes().unwrap();
    unsafe { runner.run(&input_pointers) };
    let output = runner.output_block();
    assert_eq!(output.read(0, 0), 0.625 * 0.5);
    assert_eq!(output.read(1, 0), 5.125 * 0.125);
}

#[test]
fn free_node_when_done() {
    let block_size = 16;
    let (mut graph, mut runner) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
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
    assert!(graph.inspection().nodes[0].pending_removal);
    // Apply the new TaskData on the audio thread so that the node can be removed
    unsafe {
        runner.run(&[]);
    }
    // Now the node is removed
    graph.commit_changes().unwrap();
    assert_eq!(graph.inspection().nodes.len(), 0);
}
