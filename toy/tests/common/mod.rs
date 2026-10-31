//! Helpers shared by the chapter tests.

#![allow(dead_code, reason = "each test uses a subset of the helpers")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use toy::compiler::{
    Action, InputKind, create_context, emit_llvm_ir, load_module, print_module, run_pipeline,
};

/// Returns the path of an example program in the repository's `examples` directory.
pub fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples")
        .join(name)
}

/// Compiles Toy or MLIR `source` up to `action` and returns the printed output, as `toyc --emit=<action>` would.
pub fn compile(source: &str, kind: InputKind, action: Action, optimize: bool) -> String {
    let context = create_context().expect("the context can be created");
    let mut module = load_module(&context, source, "test.toy", kind).expect("the input is valid");
    run_pipeline(&context, &mut module, action, optimize).expect("the pipeline succeeds");

    match action {
        Action::Llvm => emit_llvm_ir(&module).expect("the module translates to LLVM IR"),
        _ => print_module(&module),
    }
}

/// Compiles an example program up to `action`.
pub fn compile_example(name: &str, action: Action, optimize: bool) -> String {
    let source = fs::read_to_string(example(name)).expect("the example exists");
    compile(&source, InputKind::Toy, action, optimize)
}

/// Runs the `toyc` binary and returns its standard output.
/// The JIT prints with C's `printf`, so its output can only be captured from another process.
pub fn run_toyc(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_toyc"))
        .args(arguments)
        .output()
        .expect("toyc runs");

    assert!(
        output.status.success(),
        "toyc {arguments:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the output is UTF-8")
}
