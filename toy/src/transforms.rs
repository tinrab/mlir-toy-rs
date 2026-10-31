//! The passes of the Toy compiler.
//!
//! - [`create_canonicalizer_pass`]: Toy-specific simplifications (chapter 3).
//! - [`create_inliner_pass`]: inline all calls into `main` (chapter 4).
//! - [`create_shape_inference_pass`]: infer the shapes of all tensors (chapter 4).
//! - [`create_lower_to_affine_pass`]: lower Toy to loops over buffers (chapter 5).
//! - [`add_lower_to_llvm_passes`]: lower everything to the LLVM dialect (chapter 6).

pub use self::{
    canonicalize::{canonicalization_patterns, create_canonicalizer_pass},
    inliner::create_inliner_pass,
    lower_to_affine::create_lower_to_affine_pass,
    lower_to_llvm::{add_lower_to_llvm_passes, create_lower_print_pass},
    shape_inference::create_shape_inference_pass,
};

mod canonicalize;
mod inliner;
mod lower_to_affine;
mod lower_to_llvm;
mod shape_inference;
