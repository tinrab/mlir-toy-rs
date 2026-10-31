use melior::{
    Context, GreedyRewriteDriverConfig, PatternRewriter, RewritePattern, RewritePatternSet,
    RewriterBase, create_op_rewrite_pattern,
    ir::{OperationRef, operation::OperationLike},
};
use mlir_sys::{mlirApplyPatternsAndFoldGreedilyWithOp, mlirFrozenRewritePatternSetDestroy};

/// The result of an operation that can fail without further explanation, like `mlir::LogicalResult` in C++.
///
/// A rewrite pattern returns `Err(Failure)` when it does not match, which tells the driver to try other patterns.
pub type LogicalResult = Result<(), Failure>;

/// The failure case of [`LogicalResult`], like `mlir::failure()` in C++.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure;

/// Creates a rewrite pattern that matches operations named `root_name`, like subclassing `mlir::OpRewritePattern` in C++.
///
/// `pattern` is the `matchAndRewrite` method: given the context, it inspects the operation and, if it matches, rewrites the IR through the rewriter and returns `Ok(())`.
/// All IR changes must go through the rewriter so that the pattern driver can track them.
/// `benefit` orders patterns that match the same operation, and `generated_names` lists the operations the pattern may create.
pub fn op_rewrite_pattern<F>(
    context: &Context,
    root_name: &str,
    benefit: u32,
    generated_names: &[&str],
    pattern: F,
) -> RewritePattern
where
    F: for<'c> Fn(&'c Context, OperationRef<'c, '_>, RewriterBase<'c, '_>) -> LogicalResult
        + 'static,
{
    create_op_rewrite_pattern(
        root_name,
        benefit,
        context,
        move |_, operation, rewriter| {
            // SAFETY: MLIR passes a valid operation and pattern rewriter that
            // stay alive for the duration of this callback.
            let (operation, rewriter) = unsafe {
                (
                    OperationRef::from_raw(operation),
                    PatternRewriter::from_raw(rewriter),
                )
            };
            let context = operation.context();
            // SAFETY: `context` outlives the call to `pattern`, which cannot
            // keep the reference because of its higher-ranked lifetime.
            let context = unsafe { context.to_ref() };

            pattern(context, operation, rewriter.as_rewriter_base()).is_ok()
        },
        generated_names,
    )
}

/// Applies `patterns` to the regions of `operation` until a fixed point is reached, like `mlir::applyPatternsGreedily` in C++.
///
/// Besides running the patterns, the greedy driver folds operations and erases operations that are trivially dead (unused and free of side effects).
/// Returns `Err(Failure)` if no fixed point was reached.
pub fn apply_patterns_greedily<'c>(
    operation: OperationRef<'c, '_>,
    patterns: RewritePatternSet<'c>,
    config: &GreedyRewriteDriverConfig,
) -> LogicalResult {
    let patterns = patterns.freeze().into_raw();

    // SAFETY: The operation, the frozen pattern set, and the configuration are
    // valid. The C API only borrows the frozen set, so it is destroyed here.
    let result = unsafe {
        let result =
            mlirApplyPatternsAndFoldGreedilyWithOp(operation.to_raw(), patterns, config.to_raw());
        mlirFrozenRewritePatternSetDestroy(patterns);
        result
    };

    if result.value != 0 {
        Ok(())
    } else {
        Err(Failure)
    }
}
