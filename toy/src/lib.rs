//! The Toy language of the [MLIR tutorial], in Rust, built on [melior].
//!
//! The tutorial builds a compiler for Toy, a small tensor language, in seven chapters.
//! Each module of this crate corresponds to a part of the tutorial:
//!
//! | Chapter | Topic                                   | Modules                              |
//! | ------- | --------------------------------------- | ------------------------------------ |
//! | 1       | The Toy language and its AST            | [`lexer`], [`parser`], [`ast`]       |
//! | 2       | Emitting basic MLIR                     | [`dialect`], [`mlir_gen`]            |
//! | 3       | High-level, language-specific analysis  | `transforms::canonicalize`           |
//! | 4       | Generic transformations with interfaces | `transforms::inliner`, `transforms::shape_inference`, `dialect::shape_inference` |
//! | 5       | Partial lowering to the affine dialect  | `transforms::lower_to_affine`        |
//! | 6       | Lowering to LLVM and code generation    | `transforms::lower_to_llvm`, [`compiler`] |
//! | 7       | Adding a composite type (structs)       | spread over the modules above        |
//!
//! [`compiler`] ties everything together into the pipeline that the `toyc` binary runs.
//!
//! [MLIR tutorial]: https://mlir.llvm.org/docs/Tutorials/Toy/
//! [melior]: https://github.com/mlir-rs/melior

pub mod ast;
pub mod compiler;
pub mod dialect;
pub mod lexer;
pub mod mlir_gen;
pub mod parser;
pub mod transforms;
