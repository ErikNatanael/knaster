#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/00_no_params.rs");
    t.pass("tests/01_simple_param.rs");
    t.pass("tests/02_parameter_types.rs");
    t.pass("tests/03_process_block.rs");
    t.pass("tests/04_random_lin.rs");
    t.pass("tests/05_more_params.rs");
    t.pass("tests/06_generic_channels.rs");
    t.pass("tests/07_process_block_no_input.rs");
    t.pass("tests/08_envasr.rs");
    //t.compile_fail("tests/07-unrecognized-pattern.rs");
}
