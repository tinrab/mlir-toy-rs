//! Canonicalization of Toy operations (chapter 3, extended in chapters 4 and 7).
//!
//! Toy's high-level operations make some optimizations easy to express as local *rewrite patterns*.
//! For example, transposing a tensor twice gives the original tensor back:
//!
//! ```toy
//! def transpose_transpose(x) {
//!   return transpose(transpose(x));
//! }
//! ```
//!
//! Generic compiler infrastructure would need to prove this from loops and memory accesses; on Toy operations it is a simple pattern: `transpose(transpose(x)) -> x`.
//!
//! In C++, an operation registers such patterns as its *canonicalization patterns* (`hasCanonicalizer = 1` and `getCanonicalizationPatterns`), and MLIR's canonicalizer pass collects the patterns of all operations.
//! Some are written in C++ (`SimplifyRedundantTranspose`), others declaratively with DRR, the Declarative Rewrite Rules in `ToyCombine.td`.
//! A *folder* (`hasFolder = 1`) is a simpler kind of pattern that computes a replacement value or constant.
//!
//! Our patterns are Rust closures registered with [`op_rewrite_pattern`], and [`create_canonicalizer_pass`] applies them with the same greedy driver the canonicalizer uses.
//! The driver also erases operations that are *trivially dead*: unused results of `Pure` operations.
//! That is why we declared most Toy operations as `Pure` (see [`crate::dialect::load`]).

use melior::{
    Context, GreedyRewriteDriverConfig, RewritePatternSet, RewriterBase,
    ir::{
        OperationRef, Type, Value, ValueLike,
        attribute::{ArrayAttribute, DenseElementsAttribute},
        operation::OperationLike,
        r#type::{RankedTensorType, TypeId},
    },
    pass::{ExternalPass, Pass, create_external},
};
use melior_ext::{
    Failure, LogicalResult, apply_patterns_greedily, op_rewrite_pattern, with_context,
};

use crate::dialect::{
    CastOp, ConstantOp, ReshapeOp, StructAccessOp, StructConstantOp, StructType, ToyOp,
    TransposeOp, dimensions,
};

/// The unique identifier of the pass: the address of this static.
static PASS_ID: u64 = 0;

/// Creates a pass that applies the Toy canonicalization patterns to a function until a fixed point is reached.
#[must_use]
pub fn create_canonicalizer_pass() -> Pass {
    create_external(
        |function: OperationRef<'_, '_>, _: ExternalPass<'_>| {
            with_context(function, |context, function| {
                // Like MLIR's canonicalizer, the pass succeeds even if the
                // patterns did not converge within the iteration limit.
                let _converged = apply_patterns_greedily(
                    function,
                    canonicalization_patterns(context),
                    &GreedyRewriteDriverConfig::new(),
                );
            });
        },
        TypeId::create(&PASS_ID),
        "ToyCanonicalizer",
        "toy-canonicalize",
        "Simplify Toy operations with their canonicalization patterns",
        "func.func",
        &[],
    )
}

/// Returns the canonicalization patterns of all Toy operations.
pub fn canonicalization_patterns(context: &Context) -> RewritePatternSet<'_> {
    let patterns = RewritePatternSet::new(context);

    // `toy.transpose` (chapter 3).
    patterns.add(op_rewrite_pattern(
        context,
        TransposeOp::NAME,
        1,
        &[],
        simplify_redundant_transpose,
    ));

    // `toy.reshape` (chapter 3, DRR patterns in the C++ version).
    patterns.add(op_rewrite_pattern(
        context,
        ReshapeOp::NAME,
        1,
        &[ReshapeOp::NAME],
        reshape_reshape,
    ));
    patterns.add(op_rewrite_pattern(
        context,
        ReshapeOp::NAME,
        1,
        &[],
        redundant_reshape,
    ));
    patterns.add(op_rewrite_pattern(
        context,
        ReshapeOp::NAME,
        1,
        &[ConstantOp::NAME],
        fold_constant_reshape,
    ));

    // `toy.cast` (chapter 4, the folder of `CastOpInterface` in C++).
    patterns.add(op_rewrite_pattern(
        context,
        CastOp::NAME,
        1,
        &[],
        fold_identity_cast,
    ));

    // `toy.struct_access` (chapter 7, a folder in C++).
    patterns.add(op_rewrite_pattern(
        context,
        StructAccessOp::NAME,
        1,
        &[ConstantOp::NAME, StructConstantOp::NAME],
        fold_struct_access,
    ));

    patterns
}

/// `transpose(transpose(x)) -> x`
///
/// This is the `SimplifyRedundantTranspose` pattern of the C++ tutorial.
fn simplify_redundant_transpose<'c>(
    _: &'c Context,
    operation: OperationRef<'c, '_>,
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let transpose = TransposeOp::cast(operation).ok_or(Failure)?;

    // Look through the input of the current transpose: is it defined by
    // another transpose? If not, there is no match.
    let input = TransposeOp::defining(transpose.input()).ok_or(Failure)?;

    // Otherwise, we have a redundant transpose. Use the rewriter: all changes
    // to the IR go through it, so that the driver can track them.
    replace_with_value(rewriter, operation, input.input());
    Ok(())
}

/// `reshape(reshape(x)) -> reshape(x)`
fn reshape_reshape<'c>(
    _: &'c Context,
    operation: OperationRef<'c, '_>,
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let reshape = ReshapeOp::cast(operation).ok_or(Failure)?;
    let input = ReshapeOp::defining(reshape.input()).ok_or(Failure)?;

    // The driver sets the insertion point right before the matched operation.
    let replacement = rewriter.insert(ReshapeOp::build(
        operation.location(),
        input.input(),
        reshape.output().r#type(),
    ));
    rewriter.replace_op_with_operation(operation, replacement);
    Ok(())
}

/// `reshape(x) -> x` when the reshape does not change the type.
fn redundant_reshape<'c>(
    _: &'c Context,
    operation: OperationRef<'c, '_>,
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let reshape = ReshapeOp::cast(operation).ok_or(Failure)?;
    if reshape.input().r#type() != reshape.output().r#type() {
        return Err(Failure);
    }

    replace_with_value(rewriter, operation, reshape.input());
    Ok(())
}

/// `reshape(constant(x)) -> constant(x')`, where `x'` is the data of `x` with the new shape.
fn fold_constant_reshape<'c>(
    context: &'c Context,
    operation: OperationRef<'c, '_>,
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let reshape = ReshapeOp::cast(operation).ok_or(Failure)?;
    let constant = ConstantOp::defining(reshape.input()).ok_or(Failure)?;
    let value = reshape_constant(constant.value(), reshape.output().r#type()).ok_or(Failure)?;

    let replacement = rewriter.insert(ConstantOp::build(context, operation.location(), value));
    rewriter.replace_op_with_operation(operation, replacement);
    Ok(())
}

/// Returns `value` with the shape of `type`, or `None` if the number of elements differs.
///
/// A splat (all elements equal, such as a scalar) fills any shape, which is how `var a<2, 2> = 5.5;` works.
fn reshape_constant<'c>(
    value: DenseElementsAttribute<'c>,
    r#type: Type<'c>,
) -> Option<DenseElementsAttribute<'c>> {
    if value.is_splat() {
        return Some(DenseElementsAttribute::splat(r#type, value.splat_value()));
    }

    let element_count = dimensions(RankedTensorType::try_from(r#type).ok()?)
        .into_iter()
        .product::<u64>();
    (element_count == value.len() as u64).then(|| value.reshape(r#type))
}

/// `cast(x) -> x` when the cast does not change the type, which is the case once shape inference has run.
fn fold_identity_cast<'c>(
    _: &'c Context,
    operation: OperationRef<'c, '_>,
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let cast = CastOp::cast(operation).ok_or(Failure)?;
    if cast.input().r#type() != cast.output().r#type() {
        return Err(Failure);
    }

    replace_with_value(rewriter, operation, cast.input());
    Ok(())
}

/// `struct_access(struct_constant([a, b]), 1) -> constant(b)`
///
/// In C++, this is the folder of `StructAccessOp`: it returns the element attribute, and the dialect's `materializeConstant` hook turns the attribute back into a `toy.constant` or a `toy.struct_constant`.
fn fold_struct_access<'c>(
    context: &'c Context,
    operation: OperationRef<'c, '_>,
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let access = StructAccessOp::cast(operation).ok_or(Failure)?;
    let constant = StructConstantOp::defining(access.input()).ok_or(Failure)?;

    let location = operation.location();
    let r#type = access.output().r#type();
    let element = constant
        .value()
        .element(access.index())
        .map_err(|_| Failure)?;

    let replacement = if let Some(struct_type) = StructType::from_type(r#type) {
        let value = ArrayAttribute::try_from(element).map_err(|_| Failure)?;
        rewriter.insert(StructConstantOp::build(
            context,
            location,
            struct_type,
            value,
        ))
    } else {
        let value = DenseElementsAttribute::try_from(element).map_err(|_| Failure)?;
        rewriter.insert(ConstantOp::build_with_type(
            context, location, r#type, value,
        ))
    };
    rewriter.replace_op_with_operation(operation, replacement);
    Ok(())
}

fn replace_with_value(rewriter: RewriterBase, operation: OperationRef, value: Value) {
    rewriter.replace_op_with_values(operation, &[value]);
}
