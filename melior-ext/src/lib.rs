//! Safe wrappers for parts of the MLIR C API that `melior` does not wrap yet.
//!
//! The Toy tutorial is written against MLIR's C++ API.
//! From Rust we only have the MLIR *C* API, which `melior` wraps.
//! A few features the tutorial needs are available in the C API but not (yet) in `melior`:
//!
//! - attaching traits (terminator, custom verifiers) to operations of a dialect that was defined at runtime with IRDL ([`attach_terminator`], [`attach_verifier`]),
//! - declaring operations free of side effects, that is `Pure` ([`attach_pure`]),
//! - creating and inspecting dynamic types such as `!toy.struct` ([`DynamicTypeDefinition`]),
//! - rewrite patterns with a safe Rust callback ([`op_rewrite_pattern`]), and the greedy pattern driver ([`apply_patterns_greedily`]),
//! - the dialect conversion framework ([`ConversionTarget`], [`TypeConverter`], [`ConversionPatternSet`], [`apply_partial_conversion`]),
//! - translating a module to textual LLVM IR ([`translate_to_llvm_ir`]) and running it with the JIT ([`run_main`]),
//! - smaller helpers: unranked tensor types ([`unranked_tensor_type`]), affine map attributes ([`affine_map_attribute`]), the context of an operation ([`with_context`]), and collecting nested operations ([`nested_operations`]).
//!
//! All `unsafe` code of the project lives in this crate, so the compiler itself (`toy`) can be written in safe Rust.

pub use self::{
    attributes::affine_map_attribute,
    context::with_context,
    conversion::{
        ConversionPatternSet, ConversionTarget, TypeConverter, apply_full_conversion,
        apply_partial_conversion, modify_op_operands,
    },
    diagnostic::emit_error,
    dynamic_op::{attach_terminator, attach_verifier},
    dynamic_type::{DynamicTypeDefinition, dynamic_type_definition, dynamic_type_parameters},
    error::{Error, VerificationError},
    execution::run_main,
    memory_effects::attach_pure,
    rewrite::{Failure, LogicalResult, apply_patterns_greedily, op_rewrite_pattern},
    translation::translate_to_llvm_ir,
    types::{element_type, unranked_tensor_type},
    walk::nested_operations,
};

mod attributes;
mod context;
mod conversion;
mod diagnostic;
mod dynamic_op;
mod dynamic_type;
mod error;
mod execution;
mod memory_effects;
mod rewrite;
mod string;
mod translation;
mod types;
mod walk;
