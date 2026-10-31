//! Chapter 6: lowering to LLVM, code generation, and the JIT.

mod common;

use common::{compile_example, example, run_toyc};
use toy::compiler::Action;

/// Runs an example with the JIT and returns what it printed.
fn run_jit(name: &str, optimize: bool) -> String {
    let path = example(name);
    let path = path.to_str().expect("the path is UTF-8");
    let mut arguments = vec![path, "--emit=jit"];
    if optimize {
        arguments.push("--opt");
    }
    run_toyc(&arguments)
}

#[test]
fn test_runs_with_jit() {
    assert_eq!(
        run_jit("jit.toy", false),
        "1.000000 2.000000 \n3.000000 4.000000 \n"
    );
}

#[test]
fn test_runs_multiply_transpose_with_jit() {
    let expected = "1.000000 16.000000 \n4.000000 25.000000 \n9.000000 36.000000 \n";

    assert_eq!(run_jit("codegen.toy", false), expected);
    assert_eq!(run_jit("codegen.toy", true), expected);
}

#[test]
fn test_runs_broadcast_scalar_with_jit() {
    assert_eq!(
        run_jit("scalar.toy", false),
        "5.500000 5.500000 \n5.500000 5.500000 \n"
    );
}

#[test]
fn test_prints_one_dimensional_tensor_on_one_line() {
    let output = run_toyc(&[
        example("trivial_reshape.toy").to_str().unwrap(),
        "--emit=jit",
    ]);

    assert_eq!(output, "1.000000 \n2.000000 \n");
}

#[test]
fn test_lowers_everything_to_llvm_dialect() {
    let module = compile_example("codegen.toy", Action::MlirLlvm, false);

    assert!(
        !module.contains("toy."),
        "no Toy operation remains:\n{module}"
    );
    assert!(
        !module.contains("affine."),
        "no affine operation remains:\n{module}"
    );
    assert!(
        module.contains("llvm.func @printf(!llvm.ptr, ...) -> i32"),
        "{module}"
    );
    assert!(module.contains("llvm.call @printf"), "{module}");
}

#[test]
fn test_emits_llvm_ir() {
    let ir = compile_example("codegen.toy", Action::Llvm, true);

    // The declaration carries debug information (`declare !dbg !7 i32 ...`).
    assert!(ir.contains("i32 @printf(ptr, ...)"), "{ir}");
    assert!(ir.contains("define void @main()"), "{ir}");
    assert!(
        ir.contains(r#"@frmt_spec = internal constant [4 x i8] c"%f \00""#),
        "{ir}"
    );
}
