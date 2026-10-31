//! Chapter 4: inlining and shape inference, through interfaces.

mod common;

use common::{compile, compile_example};
use toy::compiler::{Action, InputKind};

#[test]
fn test_inlines_calls_and_infers_shapes() {
    assert_eq!(
        compile_example("codegen.toy", Action::Mlir, true),
        EXPECTED_CODEGEN
    );
}

#[test]
fn test_infers_shapes_of_mlir_input() {
    // The same program as `codegen.toy`, as MLIR (`toyc --emit=mlir`).
    let source = compile_example("codegen.toy", Action::Mlir, false);

    assert_eq!(
        compile(&source, InputKind::Mlir, Action::Mlir, true),
        EXPECTED_CODEGEN
    );
}

#[test]
fn test_broadcasts_scalar() {
    assert_eq!(
        compile_example("scalar.toy", Action::Mlir, true),
        EXPECTED_SCALAR
    );
}

const EXPECTED_CODEGEN: &str = r#"module {
  func.func @main() {
    %0 = "toy.constant"() {value = dense<[[1.000000e+00, 2.000000e+00, 3.000000e+00], [4.000000e+00, 5.000000e+00, 6.000000e+00]]> : tensor<2x3xf64>} : () -> tensor<2x3xf64>
    %1 = "toy.transpose"(%0) : (tensor<2x3xf64>) -> tensor<3x2xf64>
    %2 = "toy.mul"(%1, %1) : (tensor<3x2xf64>, tensor<3x2xf64>) -> tensor<3x2xf64>
    "toy.print"(%2) : (tensor<3x2xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#;

const EXPECTED_SCALAR: &str = r#"module {
  func.func @main() {
    %0 = "toy.constant"() {value = dense<5.500000e+00> : tensor<2x2xf64>} : () -> tensor<2x2xf64>
    "toy.print"(%0) : (tensor<2x2xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#;
