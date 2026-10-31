//! Chapter 7: adding a composite type, structs.

mod common;

use common::{compile, compile_example, example, run_toyc};
use toy::compiler::{Action, InputKind};

#[test]
fn test_emits_struct_operations() {
    assert_eq!(
        compile_example("struct_codegen.toy", Action::Mlir, false),
        EXPECTED_STRUCT
    );
}

#[test]
fn test_folds_struct_accesses() {
    assert_eq!(
        compile_example("struct_codegen.toy", Action::Mlir, true),
        EXPECTED_STRUCT_OPT
    );
}

#[test]
fn test_folds_nested_struct_accesses() {
    let source = r#"
        func.func @main() {
          %0 = "toy.struct_constant"() {value = [[dense<4.000000e+00> : tensor<2x2xf64>], dense<4.000000e+00> : tensor<2x2xf64>]} : () -> !toy.struct<[!toy.struct<[tensor<*xf64>]>, tensor<*xf64>]>
          %1 = "toy.struct_access"(%0) {index = 0 : i64} : (!toy.struct<[!toy.struct<[tensor<*xf64>]>, tensor<*xf64>]>) -> !toy.struct<[tensor<*xf64>]>
          %2 = "toy.struct_access"(%1) {index = 0 : i64} : (!toy.struct<[tensor<*xf64>]>) -> tensor<*xf64>
          "toy.print"(%2) : (tensor<*xf64>) -> ()
          "toy.return"() : () -> ()
        }
    "#;

    assert_eq!(
        compile(source, InputKind::Mlir, Action::Mlir, true),
        r#"module {
  func.func @main() {
    %0 = "toy.constant"() {value = dense<4.000000e+00> : tensor<2x2xf64>} : () -> tensor<2x2xf64>
    "toy.print"(%0) : (tensor<2x2xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#
    );
}

#[test]
fn test_runs_structs_with_jit() {
    let output = run_toyc(&[
        example("struct_codegen.toy").to_str().unwrap(),
        "--emit=jit",
    ]);

    assert_eq!(
        output,
        "1.000000 16.000000 \n4.000000 25.000000 \n9.000000 36.000000 \n"
    );
}

const EXPECTED_STRUCT: &str = r#"module {
  func.func private @multiply_transpose(%arg0: !toy.struct<[tensor<*xf64>, tensor<*xf64>]>) -> tensor<*xf64> {
    %0 = "toy.struct_access"(%arg0) {index = 0 : i64} : (!toy.struct<[tensor<*xf64>, tensor<*xf64>]>) -> tensor<*xf64>
    %1 = "toy.transpose"(%0) : (tensor<*xf64>) -> tensor<*xf64>
    %2 = "toy.struct_access"(%arg0) {index = 1 : i64} : (!toy.struct<[tensor<*xf64>, tensor<*xf64>]>) -> tensor<*xf64>
    %3 = "toy.transpose"(%2) : (tensor<*xf64>) -> tensor<*xf64>
    %4 = "toy.mul"(%1, %3) : (tensor<*xf64>, tensor<*xf64>) -> tensor<*xf64>
    "toy.return"(%4) : (tensor<*xf64>) -> ()
  }
  func.func @main() {
    %0 = "toy.struct_constant"() {value = [dense<[[1.000000e+00, 2.000000e+00, 3.000000e+00], [4.000000e+00, 5.000000e+00, 6.000000e+00]]> : tensor<2x3xf64>, dense<[[1.000000e+00, 2.000000e+00, 3.000000e+00], [4.000000e+00, 5.000000e+00, 6.000000e+00]]> : tensor<2x3xf64>]} : () -> !toy.struct<[tensor<*xf64>, tensor<*xf64>]>
    %1 = call @multiply_transpose(%0) : (!toy.struct<[tensor<*xf64>, tensor<*xf64>]>) -> tensor<*xf64>
    "toy.print"(%1) : (tensor<*xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#;

const EXPECTED_STRUCT_OPT: &str = r#"module {
  func.func @main() {
    %0 = "toy.constant"() {value = dense<[[1.000000e+00, 2.000000e+00, 3.000000e+00], [4.000000e+00, 5.000000e+00, 6.000000e+00]]> : tensor<2x3xf64>} : () -> tensor<2x3xf64>
    %1 = "toy.transpose"(%0) : (tensor<2x3xf64>) -> tensor<3x2xf64>
    %2 = "toy.mul"(%1, %1) : (tensor<3x2xf64>, tensor<3x2xf64>) -> tensor<3x2xf64>
    "toy.print"(%2) : (tensor<3x2xf64>) -> ()
    "toy.return"() : () -> ()
  }
}
"#;
