//! Chapter 3: high-level, language-specific optimizations.

mod common;

use common::{compile, compile_example};
use toy::compiler::{Action, InputKind};

#[test]
fn test_simplifies_redundant_transpose() {
    assert_eq!(
        compile_example("transpose_transpose.toy", Action::Mlir, true),
        EXPECTED_TRANSPOSE_TRANSPOSE
    );
}

#[test]
fn test_simplifies_transpose_in_function() {
    // No inlining is involved here: the canonicalization pattern alone
    // removes both transposes.
    assert_eq!(
        compile(TRANSPOSE_FUNCTION, InputKind::Mlir, Action::Mlir, true),
        EXPECTED_TRANSPOSE_FUNCTION
    );
}

#[test]
fn test_folds_trivial_reshapes() {
    assert_eq!(
        compile_example("trivial_reshape.toy", Action::Mlir, true),
        EXPECTED_TRIVIAL_RESHAPE
    );
}

const EXPECTED_TRANSPOSE_TRANSPOSE: &str = r#"module {
  func.func @main() {
    %0 = "toy.constant"() {value = dense<[[1.000000e+00, 2.000000e+00, 3.000000e+00], [4.000000e+00, 5.000000e+00, 6.000000e+00]]> : tensor<2x3xf64>} : () -> tensor<2x3xf64>
    "toy.print"(%0) : (tensor<2x3xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#;

const TRANSPOSE_FUNCTION: &str = r#"func.func @transpose_transpose(%arg0: tensor<2x3xf64>) -> tensor<2x3xf64> {
  %0 = "toy.transpose"(%arg0) : (tensor<2x3xf64>) -> tensor<3x2xf64>
  %1 = "toy.transpose"(%0) : (tensor<3x2xf64>) -> tensor<2x3xf64>
  "toy.return"(%1) : (tensor<2x3xf64>) -> ()
}
"#;

const EXPECTED_TRANSPOSE_FUNCTION: &str = r#"module {
  func.func @transpose_transpose(%arg0: tensor<2x3xf64>) -> tensor<2x3xf64> {
    "toy.return"(%arg0) : (tensor<2x3xf64>) -> ()
  }
}
"#;

const EXPECTED_TRIVIAL_RESHAPE: &str = r#"module {
  func.func @main() {
    %0 = "toy.constant"() {value = dense<[[1.000000e+00], [2.000000e+00]]> : tensor<2x1xf64>} : () -> tensor<2x1xf64>
    "toy.print"(%0) : (tensor<2x1xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#;
