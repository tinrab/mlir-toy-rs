use melior::{
    Context,
    ir::{OperationRef, operation::OperationLike},
};

/// Calls `f` with the context that owns `operation`, like `op->getContext()` in C++.
///
/// Most `melior` constructors (of types, attributes, and locations) take a `&Context`, while IR objects only hold a `ContextRef`.
/// A `&Context` can be derived from a `ContextRef` only for as long as the `ContextRef` exists, so the context is lent to a callback rather than returned.
pub fn with_context<R>(
    operation: OperationRef,
    f: impl for<'c> FnOnce(&'c Context, OperationRef<'c, 'c>) -> R,
) -> R {
    let context = operation.context();

    // SAFETY: `context` outlives the call to `f`, and the higher-ranked
    // lifetime prevents `f` from keeping the reference (or any IR object
    // tagged with its lifetime) afterwards. The operation belongs to this
    // context.
    let (context, operation) =
        unsafe { (context.to_ref(), OperationRef::from_raw(operation.to_raw())) };
    f(context, operation)
}
