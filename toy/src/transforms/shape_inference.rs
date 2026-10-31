//! The shape inference pass (chapter 4).
//!
//! After inlining, `main` contains all the code, and the shapes of the constants are known.
//! This pass propagates the shapes through the function, replacing generic `tensor<*xf64>` results with ranked tensor types.
//! It is intraprocedural and relies only on the [`ShapeInference`] interface, so it works for any operation that implements it.
//!
//! The algorithm:
//!
//! 1. Build a worklist with all the operations that return a dynamically shaped tensor: these need shape inference.
//! 2. Iterate on the worklist:
//!    1. find an operation to process: the next ready operation in the worklist has all of its operands ranked,
//!    2. if no operation is found, stop,
//!    3. remove the operation from the worklist,
//!    4. infer the types of its results from the operand types.
//! 3. If the worklist is empty, the algorithm succeeded.
//!
//! [`ShapeInference`]: crate::dialect::ShapeInference

use melior::{
    ir::{
        OperationRef, TypeLike, ValueLike,
        operation::{OperationLike, WalkOrder},
        r#type::TypeId,
    },
    pass::{ExternalPass, Pass, create_external},
};
use melior_ext::{Failure, LogicalResult, emit_error, nested_operations};

use crate::dialect::as_shape_inference;

/// The unique identifier of the pass: the address of this static.
static PASS_ID: u64 = 0;

/// Creates a pass that infers the shapes of the tensors in a function.
#[must_use]
pub fn create_shape_inference_pass() -> Pass {
    create_external(
        |function: OperationRef<'_, '_>, pass: ExternalPass<'_>| {
            if infer_shapes(function).is_err() {
                pass.signal_failure();
            }
        },
        TypeId::create(&PASS_ID),
        "ToyShapeInference",
        "toy-shape-inference",
        "Infer the shapes of Toy tensors",
        "func.func",
        &[],
    )
}

/// Infers the shapes in `function`.
/// Errors are reported as diagnostics.
fn infer_shapes(function: OperationRef) -> LogicalResult {
    // Populate the worklist with the operations that need shape inference:
    // these are operations that return a dynamic shape.
    let mut worklist = nested_operations(function, WalkOrder::PreOrder)
        .into_iter()
        .filter(|&operation| returns_dynamic_shape(operation))
        .collect::<Vec<_>>();

    // Iterate on the operations in the worklist until all operations have been
    // inferred or no change happened (fixed point).
    while let Some(index) = worklist
        .iter()
        .position(|&operation| all_operands_inferred(operation))
    {
        let operation = worklist.remove(index);

        // Ask the operation to infer its output shapes.
        let Some(shape_inference) = as_shape_inference(operation) else {
            emit_error(
                operation.location(),
                "unable to infer shape of operation without shape inference interface",
            );
            return Err(Failure);
        };
        shape_inference.infer_shapes();
    }

    // If the worklist isn't empty, this indicates a failure.
    if !worklist.is_empty() {
        emit_error(
            function.location(),
            &format!(
                "shape inference failed, {} operations couldn't be inferred",
                worklist.len()
            ),
        );
        return Err(Failure);
    }

    Ok(())
}

/// Returns whether all operands of `operation` have a ranked tensor type.
fn all_operands_inferred(operation: OperationRef) -> bool {
    operation
        .operands()
        .all(|operand| operand.r#type().is_ranked_tensor())
}

/// Returns whether `operation` has a result that is not a ranked tensor.
fn returns_dynamic_shape(operation: OperationRef) -> bool {
    operation
        .results()
        .any(|result| !result.r#type().is_ranked_tensor())
}
