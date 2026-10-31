//! Partial lowering of Toy to the affine dialect (chapter 5).
//!
//! Now that the shapes are known, the computationally intensive Toy operations can be lowered to loops over buffers:
//!
//! - tensors (immutable values) become memrefs (buffers) allocated with `memref.alloc`,
//! - `toy.add`, `toy.mul`, and `toy.transpose` become nests of `affine.for` loops that load elements, compute with the `arith` dialect, and store the results,
//! - `toy.constant` stores each element of the constant into a buffer,
//! - `toy.return` becomes `func.return`.
//!
//! The affine dialect represents loops whose bounds and memory accesses are affine functions of the loop indices, which enables optimizations such as loop fusion (enabled with `--opt`).
//!
//! The lowering is *partial*: `toy.print` stays as it is (it accepts memrefs for this purpose) and is lowered with the rest in chapter 6. This shows that several dialects can coexist in the same function.
//!
//! The lowering uses MLIR's *dialect conversion* framework: we declare which operations are legal after the conversion (the *conversion target*) and provide *conversion patterns* that rewrite the illegal ones.
//! The framework applies the patterns until everything illegal is gone, or fails and undoes all changes.

use melior::{
    Context, RewriterBase,
    dialect::{arith, func, memref, ods::affine},
    ir::{
        AffineMap, Attribute, Block, BlockLike, Location, Operation, OperationRef, Region,
        RegionLike, Type, TypeLike, Value, ValueLike,
        attribute::{FloatAttribute, IntegerAttribute},
        operation::OperationLike,
        r#type::{MemRefType, RankedTensorType, ShapedTypeLike, TypeId},
    },
    pass::{ExternalPass, Pass, create_external},
};
use melior_ext::{
    ConversionPatternSet, ConversionTarget, Error as MeliorExtError, Failure, LogicalResult,
    TypeConverter, affine_map_attribute, apply_partial_conversion, modify_op_operands,
    with_context,
};

use crate::dialect::{AddOp, ConstantOp, MulOp, PrintOp, ReturnOp, ToyOp, TransposeOp, dimensions};

/// The unique identifier of the pass: the address of this static.
static PASS_ID: u64 = 0;

/// Creates a pass that lowers the Toy operations, except `toy.print`, to the affine, arith, func, and memref dialects.
#[must_use]
pub fn create_lower_to_affine_pass() -> Pass {
    create_external(
        |module: OperationRef<'_, '_>, pass: ExternalPass<'_>| {
            if with_context(module, lower_to_affine).is_err() {
                pass.signal_failure();
            }
        },
        TypeId::create(&PASS_ID),
        "ToyToAffineLowering",
        "toy-to-affine",
        "Lower Toy operations to the affine dialect",
        "builtin.module",
        &[],
    )
}

fn lower_to_affine<'c>(
    context: &'c Context,
    module: OperationRef<'c, '_>,
) -> Result<(), MeliorExtError> {
    // The first thing to define is the conversion target: the final target
    // for this lowering. These are the dialects that are legal after it.
    let mut target = ConversionTarget::new(context);
    for dialect in ["affine", "arith", "builtin", "func", "memref"] {
        target.add_legal_dialect(dialect);
    }

    // The Toy dialect is illegal, so that the conversion fails if any Toy
    // operation is *not* converted. Since we want a partial lowering,
    // `toy.print` is explicitly kept, but only once its operand has been
    // updated from a tensor to a memref.
    target.add_illegal_dialect("toy");
    target.add_dynamically_legal_op(PrintOp::NAME, |operation| {
        operation
            .operands()
            .all(|operand| !operand.r#type().is_tensor())
    });

    // The type converter describes how types change: ranked tensors become
    // memrefs, and other types stay the same. Conversions are tried from the
    // last registered one, so the identity goes first.
    let mut type_converter = TypeConverter::new();
    type_converter.add_conversion(|r#type| Some(r#type));
    type_converter.add_conversion(|r#type| {
        RankedTensorType::try_from(r#type)
            .ok()
            .map(|r#type| convert_tensor_to_memref(r#type).into())
    });

    // Now that the conversion target has been defined, we need to provide the
    // patterns that lower the Toy operations.
    let mut patterns = ConversionPatternSet::new(context);
    patterns.add(
        AddOp::NAME,
        &type_converter,
        |context, operation, operands, rewriter| {
            lower_binary(context, operation, operands, rewriter, arith::addf)
        },
    );
    patterns.add(
        MulOp::NAME,
        &type_converter,
        |context, operation, operands, rewriter| {
            lower_binary(context, operation, operands, rewriter, arith::mulf)
        },
    );
    patterns.add(ConstantOp::NAME, &type_converter, lower_constant);
    patterns.add(TransposeOp::NAME, &type_converter, lower_transpose);
    patterns.add(PrintOp::NAME, &type_converter, lower_print);
    patterns.add(ReturnOp::NAME, &type_converter, lower_return);

    // With the target and the patterns defined, we can attempt the
    // conversion. It fails if any illegal operation was not converted.
    apply_partial_conversion(module, &target, patterns)
}

/// Converts a ranked tensor type into the corresponding memref type.
fn convert_tensor_to_memref(r#type: RankedTensorType) -> MemRefType {
    let shape = dimensions(r#type)
        .into_iter()
        // A dynamic dimension keeps MLIR's dynamic marker.
        .map(u64::cast_signed)
        .collect::<Vec<_>>();
    MemRefType::new(r#type.element(), &shape, None, None)
}

/// Lowers `toy.add` and `toy.mul`: at each index, load the elements of both operands and combine them with `build`, which creates `arith.addf` or `arith.mulf`.
fn lower_binary<'c>(
    context: &'c Context,
    operation: OperationRef<'c, '_>,
    operands: &[Value<'c, '_>],
    rewriter: RewriterBase<'c, '_>,
    build: for<'x, 'y> fn(Value<'c, 'x>, Value<'c, 'y>, Location<'c>) -> Operation<'c>,
) -> LogicalResult {
    let location = operation.location();
    // The operands are already converted: they are the buffers of the inputs.
    let [lhs, rhs] = *operands else {
        return Err(Failure);
    };

    lower_op_to_loops(context, operation, &rewriter, |indices| {
        // Generate loads for the elements of the operands at the inner loop.
        let lhs = load(&rewriter, context, location, lhs, indices);
        let rhs = load(&rewriter, context, location, rhs, indices);

        // Create the binary operation performed on the loaded values.
        result(rewriter.insert(build(lhs, rhs, location)))
    })
}

/// Lowers `toy.transpose`: the element at `[i, j]` is the input's element at `[j, i]`, so the loads use the indices in reverse order.
fn lower_transpose<'c>(
    context: &'c Context,
    operation: OperationRef<'c, '_>,
    operands: &[Value<'c, '_>],
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let location = operation.location();
    let [input] = *operands else {
        return Err(Failure);
    };

    lower_op_to_loops(context, operation, &rewriter, |indices| {
        let reversed = indices.iter().rev().copied().collect::<Vec<_>>();
        load(&rewriter, context, location, input, &reversed)
    })
}

/// Lowers `toy.constant`: allocate a buffer and store each element of the constant into it.
fn lower_constant<'c>(
    context: &'c Context,
    operation: OperationRef<'c, '_>,
    _: &[Value<'c, '_>],
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let constant = ConstantOp::cast(operation).ok_or(Failure)?;
    let location = operation.location();
    let value = constant.value();

    let tensor_type =
        RankedTensorType::try_from(constant.output().r#type()).map_err(|_| Failure)?;
    let alloc = insert_alloc_and_dealloc(
        &rewriter,
        context,
        location,
        convert_tensor_to_memref(tensor_type),
        operation,
    );

    // We will generate constant indices up to the largest dimension. Create
    // them up front to avoid large amounts of redundant operations.
    if !tensor_type.has_static_shape() {
        return Err(Failure);
    }
    let shape = dimensions(tensor_type)
        .into_iter()
        .map(|size| usize::try_from(size).expect("the shape is static"))
        .collect::<Vec<_>>();
    let largest_dimension = shape.iter().copied().max().unwrap_or(0);
    let index_constants = (0..largest_dimension)
        .map(|index| {
            result(
                rewriter.insert(arith::constant(
                    context,
                    IntegerAttribute::new(
                        Type::index(context),
                        i64::try_from(index).expect("the shape is static"),
                    )
                    .into(),
                    location,
                )),
            )
        })
        .collect::<Vec<_>>();

    // The constant is multidimensional, so store each element at its index,
    // in row-major order. A rank-0 tensor has a single element and no index.
    let element_count = shape.iter().product::<usize>();
    for element in 0..element_count {
        let mut position = vec![0; shape.len()];
        let mut remainder = element;
        for (dimension, &size) in shape.iter().enumerate().rev() {
            position[dimension] = remainder % size;
            remainder /= size;
        }
        let indices = position
            .into_iter()
            .map(|index| index_constants[index])
            .collect::<Vec<_>>();

        let element_value = value.f64_element(element).map_err(|_| Failure)?;
        let element_value = result(rewriter.insert(arith::constant(
            context,
            FloatAttribute::new(context, Type::float64(context), element_value).into(),
            location,
        )));
        rewriter.insert(
            affine::store(
                context,
                element_value,
                alloc,
                &indices,
                identity_map(context, indices.len()),
                location,
            )
            .into(),
        );
    }

    // Replace this operation with the generated buffer.
    rewriter.replace_op_with_values(operation, &[alloc]);
    Ok(())
}

/// `toy.print` is not lowered in this pass, but its operand must be updated to the converted value: the buffer instead of the tensor.
#[expect(
    clippy::unnecessary_wraps,
    reason = "conversion patterns return whether they matched"
)]
fn lower_print<'c>(
    _: &'c Context,
    operation: OperationRef<'c, '_>,
    operands: &[Value<'c, '_>],
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    modify_op_operands(rewriter, operation, operands);
    Ok(())
}

/// Lowers `toy.return` to `func.return`.
/// All calls have been inlined, so only `main` remains, which returns nothing.
fn lower_return<'c>(
    _: &'c Context,
    operation: OperationRef<'c, '_>,
    operands: &[Value<'c, '_>],
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    if !operands.is_empty() {
        return Err(Failure);
    }

    let r#return = rewriter.insert(func::r#return(&[], operation.location()));
    rewriter.replace_op_with_operation(operation, r#return);
    Ok(())
}

/// Lowers an operation producing a tensor to a loop nest that computes each element with `compute_element` and stores it into a new buffer, which replaces the operation's result.
fn lower_op_to_loops<'c, 'r>(
    context: &'c Context,
    operation: OperationRef<'c, '_>,
    rewriter: &'r RewriterBase<'c, '_>,
    compute_element: impl FnOnce(&[Value<'c, 'r>]) -> Value<'c, 'r>,
) -> LogicalResult {
    let location = operation.location();
    // The shapes are known after shape inference; tensors of unknown shape
    // cannot be lowered.
    let result_type = operation.result(0).map_err(|_| Failure)?.r#type();
    let tensor_type = RankedTensorType::try_from(result_type).map_err(|_| Failure)?;
    if !tensor_type.has_static_shape() {
        return Err(Failure);
    }

    // Insert an allocation and deallocation for the result of this operation.
    let alloc = insert_alloc_and_dealloc(
        rewriter,
        context,
        location,
        convert_tensor_to_memref(tensor_type),
        operation,
    );

    // Create a nest of affine loops, with one loop per dimension of the shape.
    // The innermost body computes the value to store at the current indices.
    build_affine_loop_nest(
        rewriter,
        context,
        location,
        &dimensions(tensor_type),
        |indices| {
            let value = compute_element(indices);
            rewriter.insert(
                affine::store(
                    context,
                    value,
                    alloc,
                    indices,
                    identity_map(context, indices.len()),
                    location,
                )
                .into(),
            );
        },
    );

    // Replace this operation with the generated buffer.
    rewriter.set_insertion_point_before(operation);
    rewriter.replace_op_with_values(operation, &[alloc]);
    Ok(())
}

/// Inserts an allocation of a buffer of type `type` at the beginning of the block of `operation`, and its deallocation at the end of the block.
/// This is fine because Toy functions have no control flow.
fn insert_alloc_and_dealloc<'c, 'r>(
    rewriter: &'r RewriterBase<'c, '_>,
    context: &'c Context,
    location: Location<'c>,
    r#type: MemRefType<'c>,
    operation: OperationRef<'c, '_>,
) -> Value<'c, 'r> {
    let block = operation.block().expect("the operation is in a block");

    let alloc = rewriter.insert(memref::alloc(context, r#type, &[], &[], None, location));
    let first = block.first_operation().expect("the block is not empty");
    if first != alloc {
        rewriter.move_op_before(alloc, first);
    }
    let alloc = result(alloc);

    let dealloc = rewriter.insert(memref::dealloc(alloc, location));
    let terminator = block.terminator().expect("the block has a terminator");
    rewriter.move_op_before(dealloc, terminator);

    // Continue inserting before the operation being lowered.
    rewriter.set_insertion_point_before(operation);
    alloc
}

/// Builds a nest of affine loops, one per dimension of `shape`, each counting from 0 to the size of its dimension, like `affine::buildAffineLoopNest` in C++.
/// `body` builds the body of the innermost loop through the rewriter, given the loop induction variables.
fn build_affine_loop_nest<'c, 'r>(
    rewriter: &'r RewriterBase<'c, '_>,
    context: &'c Context,
    location: Location<'c>,
    shape: &[u64],
    body: impl FnOnce(&[Value<'c, 'r>]),
) {
    let index = Type::index(context);
    let mut induction_variables = Vec::new();

    for &size in shape {
        let block = Block::new(&[(index, location)]);
        block.append_operation(affine::r#yield(context, &[], location).into());
        let region = Region::new();
        region.append_block(block);

        let r#loop = rewriter.insert(
            affine::r#for(
                context,
                &[],
                &[],
                &[],
                &[],
                region,
                affine_map_attribute(AffineMap::constant(context, 0)),
                affine_map_attribute(AffineMap::constant(
                    context,
                    i64::try_from(size).expect("the shape is static"),
                )),
                IntegerAttribute::new(index, 1),
                location,
            )
            .into(),
        );

        // Continue inside the loop, before its terminator.
        let loop_body = r#loop
            .region(0)
            .ok()
            .and_then(|region| region.first_block())
            .expect("the loop has a body");
        induction_variables.push(
            loop_body
                .argument(0)
                .expect("the loop has an induction variable")
                .into(),
        );
        rewriter.set_insertion_point_to_start(loop_body);
    }

    body(&induction_variables);
}

/// Loads the element of `buffer` at `indices` with `affine.load`.
fn load<'c, 'r>(
    rewriter: &'r RewriterBase<'c, '_>,
    context: &'c Context,
    location: Location<'c>,
    buffer: Value<'c, '_>,
    indices: &[Value<'c, '_>],
) -> Value<'c, 'r> {
    let element_type = MemRefType::try_from(buffer.r#type())
        .expect("the operand was converted to a memref")
        .element();

    result(
        rewriter.insert(
            affine::load(
                context,
                element_type,
                buffer,
                indices,
                identity_map(context, indices.len()),
                location,
            )
            .into(),
        ),
    )
}

/// Returns `affine_map<(d0, ..., dn) -> (d0, ..., dn)>`: the indices of an affine access are used as they are.
fn identity_map(context: &Context, rank: usize) -> Attribute<'_> {
    affine_map_attribute(AffineMap::multi_dim_identity(context, rank))
}

fn result<'c, 'a>(operation: OperationRef<'c, 'a>) -> Value<'c, 'a> {
    operation
        .result(0)
        .expect("the operation has a result")
        .into()
}
