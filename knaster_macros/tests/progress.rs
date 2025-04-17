#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/00_no_params.rs");
    t.pass("tests/01_simple_param.rs");
    t.pass("tests/02_parameter_types.rs");
    //t.compile_fail("tests/07-unrecognized-pattern.rs");
}
