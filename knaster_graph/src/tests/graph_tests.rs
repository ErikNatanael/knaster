use knaster_core::{typenum::U3, Block};

use crate::{
    graph::GraphSettings, handle::HandleTrait, runner::Runner, tests::utils::TestInPlusParamGen,
};

#[test]
fn graph_inputs_to_outputs() {
    let block_size = 16;
    let (mut graph, mut runner) = Runner::new::<U3, U3>(GraphSettings {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
        ..Default::default()
    });

    // Connect input 1 to 0, 2, to 1
    graph.connect_input_to_output(1, 0, 2, false).unwrap();
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
    let (mut graph, mut runner) = Runner::new::<U3, U3>(GraphSettings {
        block_size,
        sample_rate: 48000,
        ring_buffer_size: 50,
        ..Default::default()
    });

    // Connect input 1 to 0, 2, to 1
    graph.connect_input_to_output(0, 1, 2, false).unwrap();
    let g0 = graph.push(TestInPlusParamGen::new()).unwrap();
    let g1 = graph.push(TestInPlusParamGen::new()).unwrap();
    g0.set(("number", 0.75)).unwrap();
    g1.set(("number", 0.5)).unwrap();
    graph.connect_node_to_output(&g0, 0, 2, 1, true).unwrap();
    graph.connect_input_to_node(&g1, 2, 0, 1, false).unwrap();
    graph.connect_node_to_output(&g1, 0, 0, 1, false).unwrap();
    graph.commit_changes().unwrap();

    let input_allocation = vec![2.0; 16 * 3];
    let input_pointers = [
        input_allocation.as_ptr(),
        unsafe { input_allocation.as_ptr().add(block_size) },
        unsafe { input_allocation.as_ptr().add(block_size * 2) },
    ];
    unsafe { runner.run(&input_pointers) };
    let output = runner.output_block();
    dbg!(output.channel_as_slice(0));
    dbg!(output.channel_as_slice(1));
    dbg!(output.channel_as_slice(2));
    assert_eq!(output.read(0, 0), 2.5);
    assert_eq!(output.read(1, 0), 2.0);
    assert_eq!(output.read(2, 0), 2.75);
}
