//! The compiler driver: loading a program and running the pass pipeline.
//!
//! This is the `toyc.cpp` of the tutorial, minus the command line parsing, which lives in the `toyc` binary.
//! Each chapter extends the pipeline:
//!
//! | Chapter | Output (`--emit`) | Pipeline                                         |
//! | ------- | ----------------- | ------------------------------------------------ |
//! | 1       | `ast`             | parse only                                       |
//! | 2       | `mlir`            | emit the Toy dialect                             |
//! | 3, 4    | `mlir --opt`      | inline, canonicalize, infer shapes               |
//! | 5       | `mlir-affine`     | partially lower to the affine dialect            |
//! | 6       | `mlir-llvm`       | lower to the LLVM dialect                        |
//! | 6       | `llvm`, `jit`     | translate to LLVM IR or run with the JIT         |

use melior::{
    Context,
    dialect::DialectRegistry,
    ir::Module,
    pass::{PassManager, affine, llvm, transform},
    utility::{register_all_dialects, register_all_llvm_translations},
};
use melior_ext::{Error as MeliorExtError, run_main, translate_to_llvm_ir};

use crate::{
    dialect::{self, DialectError},
    mlir_gen::{MlirGenError, mlir_gen},
    parser::{ParseError, parse},
    transforms::{
        add_lower_to_llvm_passes, create_canonicalizer_pass, create_inliner_pass,
        create_lower_to_affine_pass, create_shape_inference_pass,
    },
};

/// How far to compile a program, from the least to the most lowered output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    /// Print the AST (chapter 1).
    Ast,
    /// Print the MLIR in the Toy dialect (chapter 2).
    Mlir,
    /// Print the MLIR after partial lowering to the affine dialect (chapter 5).
    MlirAffine,
    /// Print the MLIR after lowering to the LLVM dialect (chapter 6).
    MlirLlvm,
    /// Print the LLVM IR (chapter 6).
    Llvm,
    /// Run `main` with the JIT (chapter 6).
    Jit,
}

/// The language of the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    /// Toy source code.
    Toy,
    /// MLIR, for example the output of `toyc --emit=mlir`.
    Mlir,
}

/// An error that stops the compilation.
///
/// Errors found by MLIR (verification errors, pass failures) are reported through MLIR's diagnostics, which print to standard error by default.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    MlirGen(#[from] MlirGenError),

    #[error(transparent)]
    Dialect(#[from] DialectError),

    #[error("failed to parse the MLIR input")]
    InvalidMlir,

    #[error("failed to run the pass pipeline")]
    Pipeline,

    #[error("failed to emit LLVM IR")]
    LlvmTranslation(#[source] MeliorExtError),

    #[error("failed to run the program with the JIT")]
    Jit(#[source] MeliorExtError),
}

/// Creates an MLIR context with all the dialects the compiler uses.
pub fn create_context() -> Result<Context, CompileError> {
    // Register the dialects of MLIR, so that MLIR input can use them and
    // passes can load them as needed.
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);
    let context = Context::new();
    context.append_dialect_registry(&registry);
    context.load_all_available_dialects();

    // Register the translation to LLVM IR, which must happen before we can
    // JIT-compile or emit LLVM IR.
    register_all_llvm_translations(&context);

    // Load our dialect in this context.
    dialect::load(&context)?;

    Ok(context)
}

/// Loads a program into an MLIR module, from Toy source or from MLIR.
pub fn load_module<'c>(
    context: &'c Context,
    source: &str,
    file_name: &str,
    kind: InputKind,
) -> Result<Module<'c>, CompileError> {
    match kind {
        InputKind::Toy => Ok(mlir_gen(context, &parse(source, file_name)?)?),
        InputKind::Mlir => Module::parse(context, source).ok_or(CompileError::InvalidMlir),
    }
}

/// Runs the passes needed for `action` on `module`.
/// `optimize` enables the optimizations of chapters 3 and 4 and, when lowering, of chapter 5.
pub fn run_pipeline(
    context: &Context,
    module: &mut Module,
    action: Action,
    optimize: bool,
) -> Result<(), CompileError> {
    let pass_manager = PassManager::new(context);

    let is_lowering_to_affine = action >= Action::MlirAffine;
    let is_lowering_to_llvm = action >= Action::MlirLlvm;

    if optimize || is_lowering_to_affine {
        // Inline all functions into main and then delete them.
        pass_manager.add_pass(create_inliner_pass());
        pass_manager.add_pass(transform::create_symbol_dce_pass());

        // Now that there is only one function, we can infer the shapes of each
        // of the operations.
        let function_pass_manager = pass_manager.nested_under("func.func");
        function_pass_manager.add_pass(create_canonicalizer_pass());
        function_pass_manager.add_pass(create_shape_inference_pass());
        function_pass_manager.add_pass(create_canonicalizer_pass());
        function_pass_manager.add_pass(transform::create_cse_pass());
    }

    if is_lowering_to_affine {
        // Partially lower the Toy dialect.
        pass_manager.add_pass(create_lower_to_affine_pass());

        // Add a few cleanups post lowering.
        let function_pass_manager = pass_manager.nested_under("func.func");
        function_pass_manager.add_pass(transform::create_canonicalizer_pass());
        function_pass_manager.add_pass(transform::create_cse_pass());

        // Add optimizations if enabled.
        if optimize {
            function_pass_manager.add_pass(affine::create_affine_loop_fusion());
            function_pass_manager.add_pass(affine::create_affine_scalar_replacement());
        }
    }

    if is_lowering_to_llvm {
        // Finish lowering the Toy IR to the LLVM dialect.
        add_lower_to_llvm_passes(&pass_manager);
        // This is necessary to have line tables emitted and a basic debugger
        // working.
        pass_manager.add_pass(llvm::create_di_scope_for_llvm_func_op_pass());
    }

    pass_manager.run(module).map_err(|_| CompileError::Pipeline)
}

/// Translates a module in the LLVM dialect to LLVM IR.
pub fn emit_llvm_ir(module: &Module) -> Result<String, CompileError> {
    translate_to_llvm_ir(module).map_err(CompileError::LlvmTranslation)
}

/// JIT-compiles a module in the LLVM dialect and runs its `main` function.
pub fn run_jit(module: &Module, optimize: bool) -> Result<(), CompileError> {
    let optimization_level = if optimize { 3 } else { 0 };
    run_main(module, optimization_level).map_err(CompileError::Jit)
}

/// Returns the textual form of a module.
#[must_use]
pub fn print_module(module: &Module) -> String {
    module.as_operation().to_string()
}
