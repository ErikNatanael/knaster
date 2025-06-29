use knaster_graph::{Float, math::*, typenum::U1};

pub fn mul<F: Float>() -> MathUGen<F, U1, Mul> {
    MathUGen::new()
}
pub fn add<F: Float>() -> MathUGen<F, U1, Add> {
    MathUGen::new()
}
pub fn sub<F: Float>() -> MathUGen<F, U1, Sub> {
    MathUGen::new()
}
pub fn div<F: Float>() -> MathUGen<F, U1, Div> {
    MathUGen::new()
}
pub fn pow<F: Float>() -> MathUGen<F, U1, Pow> {
    MathUGen::new()
}

pub fn fract<F: Float>() -> Math1UGen<F, Fract> {
    Math1UGen::new()
}
pub fn ceil<F: Float>() -> Math1UGen<F, Ceil> {
    Math1UGen::new()
}
pub fn exp<F: Float>() -> Math1UGen<F, Exp> {
    Math1UGen::new()
}
pub fn trunc<F: Float>() -> Math1UGen<F, Trunc> {
    Math1UGen::new()
}
pub fn floor<F: Float>() -> Math1UGen<F, Floor> {
    Math1UGen::new()
}
pub fn sqrt<F: Float>() -> Math1UGen<F, Sqrt> {
    Math1UGen::new()
}
