use melior::ir::{
    OperationRef,
    operation::{OperationLike, WalkOrder, WalkResult},
};
use mlir_sys::MlirOperation;

/// Returns `operation` and all operations nested in it, in the given order.
///
/// `OperationLike::walk` hands each operation to a callback with a lifetime limited to the call, so they cannot be collected for later modification.
/// The operations returned here borrow from `operation` instead.
#[must_use]
pub fn nested_operations<'c, 'a>(
    operation: OperationRef<'c, 'a>,
    order: WalkOrder,
) -> Vec<OperationRef<'c, 'a>> {
    let mut operations = Vec::<MlirOperation>::new();
    operation.walk(order, |operation| {
        operations.push(operation.to_raw());
        WalkResult::Advance
    });

    operations
        .into_iter()
        // SAFETY: Each operation is nested in `operation`, so it is owned by
        // the same IR as `operation` and lives as long as it (`'a`).
        .map(|operation| unsafe { OperationRef::from_raw(operation) })
        .collect()
}
