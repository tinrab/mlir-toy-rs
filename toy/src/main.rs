//! `toyc`: the Toy compiler.
//!
//! ```sh
//! toyc examples/ast.toy --emit=ast          # chapter 1
//! toyc examples/codegen.toy --emit=mlir     # chapter 2
//! toyc examples/codegen.toy --emit=mlir --opt # chapters 3 and 4
//! toyc examples/codegen.toy --emit=mlir-affine # chapter 5
//! toyc examples/codegen.toy --emit=jit      # chapter 6
//! ```

use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Context as _, bail};
use clap::{Parser, ValueEnum};
use toy::{
    compiler::{
        Action, InputKind, create_context, emit_llvm_ir, load_module, print_module, run_jit,
        run_pipeline,
    },
    parser::parse,
};

/// The Toy compiler.
#[derive(Debug, Parser)]
#[command(name = "toyc")]
struct Arguments {
    /// The input file, or `-` for the standard input.
    #[arg(default_value = "-")]
    input: PathBuf,

    /// The language of the input.
    /// Defaults to MLIR for files ending in `.mlir`, and to Toy otherwise.
    #[arg(short = 'x', value_enum)]
    input_kind: Option<InputKindArgument>,

    /// The kind of output desired.
    #[arg(long, value_enum)]
    emit: Option<Emit>,

    /// Enable optimizations.
    #[arg(long)]
    opt: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum InputKindArgument {
    /// Load the input file as Toy source.
    Toy,
    /// Load the input file as MLIR.
    Mlir,
}

impl From<InputKindArgument> for InputKind {
    fn from(kind: InputKindArgument) -> Self {
        match kind {
            InputKindArgument::Toy => Self::Toy,
            InputKindArgument::Mlir => Self::Mlir,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Emit {
    /// Output the AST dump.
    Ast,
    /// Output the MLIR dump.
    Mlir,
    /// Output the MLIR dump after affine lowering.
    MlirAffine,
    /// Output the MLIR dump after LLVM lowering.
    MlirLlvm,
    /// Output the LLVM IR dump.
    Llvm,
    /// JIT the code and run it by invoking the main function.
    Jit,
}

impl From<Emit> for Action {
    fn from(emit: Emit) -> Self {
        match emit {
            Emit::Ast => Self::Ast,
            Emit::Mlir => Self::Mlir,
            Emit::MlirAffine => Self::MlirAffine,
            Emit::MlirLlvm => Self::MlirLlvm,
            Emit::Llvm => Self::Llvm,
            Emit::Jit => Self::Jit,
        }
    }
}

fn main() -> ExitCode {
    match run(&Arguments::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &Arguments) -> anyhow::Result<()> {
    let Some(emit) = arguments.emit else {
        bail!("no action specified (parsing only?), use --emit=<action>");
    };
    let action = Action::from(emit);

    let source = read_input(&arguments.input)?;
    let file_name = arguments.input.display().to_string();
    let is_mlir_file = arguments
        .input
        .extension()
        .is_some_and(|extension| extension == "mlir");
    let input_kind = arguments.input_kind.map_or_else(
        || {
            if is_mlir_file {
                InputKind::Mlir
            } else {
                InputKind::Toy
            }
        },
        InputKind::from,
    );

    // Chapter 1: the AST does not involve MLIR at all.
    if action == Action::Ast {
        if input_kind == InputKind::Mlir {
            bail!("can't dump a Toy AST when the input is MLIR");
        }
        print!("{}", parse(&source, &file_name)?.dump());
        return Ok(());
    }

    // Otherwise, we are compiling with or to MLIR.
    let context = create_context()?;
    let mut module = load_module(&context, &source, &file_name, input_kind)?;
    run_pipeline(&context, &mut module, action, arguments.opt)?;

    match action {
        Action::Ast => unreachable!("handled above"),
        Action::Mlir | Action::MlirAffine | Action::MlirLlvm => {
            println!("{}", print_module(&module));
        }
        Action::Llvm => print!("{}", emit_llvm_ir(&module)?),
        Action::Jit => run_jit(&module, arguments.opt)?,
    }

    Ok(())
}

fn read_input(path: &Path) -> anyhow::Result<String> {
    if path == Path::new("-") {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .context("could not read the standard input")?;
        Ok(source)
    } else {
        fs::read_to_string(path)
            .with_context(|| format!("could not open input file {}", path.display()))
    }
}
