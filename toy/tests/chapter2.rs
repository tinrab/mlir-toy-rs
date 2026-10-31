//! Chapter 2: emitting MLIR in the Toy dialect.

mod common;

use common::{compile, compile_example};
use toy::compiler::{Action, CompileError, InputKind, create_context, load_module};

#[test]
fn test_emits_toy_dialect() {
    assert_eq!(
        compile_example("codegen.toy", Action::Mlir, false),
        EXPECTED_CODEGEN
    );
}

#[test]
fn test_emits_scalar_constant_and_reshape() {
    assert_eq!(
        compile_example("scalar.toy", Action::Mlir, false),
        EXPECTED_SCALAR
    );
}

#[test]
fn test_parses_emitted_mlir_back() {
    // The generic form is valid MLIR input for `toyc -x mlir`.
    assert_eq!(
        compile(EXPECTED_CODEGEN, InputKind::Mlir, Action::Mlir, false),
        EXPECTED_CODEGEN
    );
}

#[test]
fn test_rejects_invalid_mlir() {
    // `toy.print` takes an operand and produces no result, and the block has
    // no terminator.
    let source = r#"
        func.func @main() {
          %0 = "toy.print"() : () -> tensor<2x3xf64>
        }
    "#;
    let context = create_context().unwrap();

    assert!(matches!(
        load_module(&context, source, "invalid.mlir", InputKind::Mlir),
        Err(CompileError::InvalidMlir)
    ));
}

#[test]
fn test_reports_semantic_errors() {
    let cases = [
        (
            "def main() { print(b); }",
            "test.toy:1:20: unknown variable 'b'",
        ),
        (
            "def main() { print(f(1)); }",
            "test.toy:1:20: no defined function found for 'f'",
        ),
        (
            "def f(a, b) { return a; } def main() { print(f(1)); }",
            "test.toy:1:46: function 'f' expects 2 arguments, but 1 were given",
        ),
        (
            "def main() { var a = 1; var a = 2; }",
            "test.toy:1:25: variable 'a' is already declared",
        ),
        (
            "def main() { print(1 - 2); }",
            "test.toy:1:24: invalid binary operator '-'",
        ),
    ];

    for (source, expected) in cases {
        let context = create_context().unwrap();
        let error = load_module(&context, source, "test.toy", InputKind::Toy).unwrap_err();

        assert_eq!(error.to_string(), expected, "for {source:?}");
    }
}

const EXPECTED_CODEGEN: &str = r#"module {
  func.func private @multiply_transpose(%arg0: tensor<*xf64>, %arg1: tensor<*xf64>) -> tensor<*xf64> {
    %0 = "toy.transpose"(%arg0) : (tensor<*xf64>) -> tensor<*xf64>
    %1 = "toy.transpose"(%arg1) : (tensor<*xf64>) -> tensor<*xf64>
    %2 = "toy.mul"(%0, %1) : (tensor<*xf64>, tensor<*xf64>) -> tensor<*xf64>
    "toy.return"(%2) : (tensor<*xf64>) -> ()
  }
  func.func @main() {
    %0 = "toy.constant"() {value = dense<[[1.000000e+00, 2.000000e+00, 3.000000e+00], [4.000000e+00, 5.000000e+00, 6.000000e+00]]> : tensor<2x3xf64>} : () -> tensor<2x3xf64>
    %1 = "toy.reshape"(%0) : (tensor<2x3xf64>) -> tensor<2x3xf64>
    %2 = "toy.constant"() {value = dense<[1.000000e+00, 2.000000e+00, 3.000000e+00, 4.000000e+00, 5.000000e+00, 6.000000e+00]> : tensor<6xf64>} : () -> tensor<6xf64>
    %3 = "toy.reshape"(%2) : (tensor<6xf64>) -> tensor<2x3xf64>
    %4 = "toy.cast"(%1) : (tensor<2x3xf64>) -> tensor<*xf64>
    %5 = "toy.cast"(%3) : (tensor<2x3xf64>) -> tensor<*xf64>
    %6 = call @multiply_transpose(%4, %5) : (tensor<*xf64>, tensor<*xf64>) -> tensor<*xf64>
    %7 = "toy.cast"(%3) : (tensor<2x3xf64>) -> tensor<*xf64>
    %8 = "toy.cast"(%1) : (tensor<2x3xf64>) -> tensor<*xf64>
    %9 = call @multiply_transpose(%7, %8) : (tensor<*xf64>, tensor<*xf64>) -> tensor<*xf64>
    "toy.print"(%9) : (tensor<*xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#;

const EXPECTED_SCALAR: &str = r#"module {
  func.func @main() {
    %0 = "toy.constant"() {value = dense<5.500000e+00> : tensor<f64>} : () -> tensor<f64>
    %1 = "toy.reshape"(%0) : (tensor<f64>) -> tensor<2x2xf64>
    "toy.print"(%1) : (tensor<2x2xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#;
