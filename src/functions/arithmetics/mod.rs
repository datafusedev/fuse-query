// Copyright 2020 The FuseQuery Authors.
//
// Code is licensed under AGPL License, Version 3.0.

mod arithmetic_test;

mod arithmetic;
mod arithmetic_add;
mod arithmetic_div;
mod arithmetic_modulo;
mod arithmetic_mul;
mod arithmetic_sub;

pub use arithmetic::ArithmeticFunction;
pub use arithmetic_add::ArithmeticPlusFunction;
pub use arithmetic_div::ArithmeticDivFunction;
pub use arithmetic_modulo::ArithmeticModuloFunction;
pub use arithmetic_mul::ArithmeticMulFunction;
pub use arithmetic_sub::ArithmeticMinusFunction;
