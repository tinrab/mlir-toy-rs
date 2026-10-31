//! The dialect conversion framework.
//!
//! Dialect conversion rewrites IR from one set of operations and types to another.
//! It has three parts:
//!
//! - a [`ConversionTarget`], which says which operations are *legal* after the conversion,
//! - a set of conversion patterns ([`ConversionPatternSet`]), which rewrite *illegal* operations into legal ones,
//! - optionally a [`TypeConverter`], which says how types change.
//!
//! A *partial* conversion only requires the illegal operations to be converted and leaves unknown operations alone; a *full* conversion requires every operation to be legal afterwards.

use std::{ffi::c_void, marker::PhantomData, mem, ptr, slice};

use melior::{
    Context, PatternRewriter, RewriterBase,
    ir::{OperationRef, Type, TypeLike, Value, ValueLike, operation::OperationLike},
};
use mlir_sys::{
    MlirConversionConfig, MlirConversionPattern, MlirConversionPatternCallbacks,
    MlirConversionPatternRewriter, MlirConversionTarget, MlirConversionTargetLegality,
    MlirConversionTargetLegality_MLIR_CONVERSION_TARGET_LEGALITY_ILLEGAL,
    MlirConversionTargetLegality_MLIR_CONVERSION_TARGET_LEGALITY_LEGAL,
    MlirFrozenRewritePatternSet, MlirLogicalResult, MlirOperation, MlirRewritePatternSet, MlirType,
    MlirTypeConverter, MlirValue, mlirApplyFullConversion, mlirApplyPartialConversion,
    mlirConversionConfigCreate, mlirConversionConfigDestroy, mlirConversionPatternAsRewritePattern,
    mlirConversionPatternRewriterAsPatternRewriter, mlirConversionTargetAddDynamicallyLegalOp,
    mlirConversionTargetAddIllegalDialect, mlirConversionTargetAddIllegalOp,
    mlirConversionTargetAddLegalDialect, mlirConversionTargetAddLegalOp,
    mlirConversionTargetCreate, mlirConversionTargetDestroy, mlirFreezeRewritePattern,
    mlirFrozenRewritePatternSetDestroy, mlirOpConversionPatternCreate, mlirOperationSetOperands,
    mlirRewritePatternSetAdd, mlirRewritePatternSetCreate, mlirRewritePatternSetDestroy,
    mlirTypeConverterAddConversion, mlirTypeConverterConvertType, mlirTypeConverterCreate,
    mlirTypeConverterDestroy,
};

use crate::{error::Error, rewrite::LogicalResult, string::string_ref};

const SUCCESS: MlirLogicalResult = MlirLogicalResult { value: 1 };
const FAILURE: MlirLogicalResult = MlirLogicalResult { value: 0 };

type LegalityCallback = dyn Fn(OperationRef<'_, '_>) -> bool;

type TypeConversion = dyn for<'c> Fn(Type<'c>) -> Option<Type<'c>>;

/// Describes which operations are legal after a conversion.
pub struct ConversionTarget<'c> {
    raw: MlirConversionTarget,
    /// Dynamic legality callbacks, owned here and borrowed by `raw`.
    callbacks: Vec<Box<LegalityCallback>>,
    _context: PhantomData<&'c Context>,
}

impl<'c> ConversionTarget<'c> {
    #[must_use]
    pub fn new(context: &'c Context) -> Self {
        Self {
            // SAFETY: The context is valid.
            raw: unsafe { mlirConversionTargetCreate(context.to_raw()) },
            callbacks: Vec::new(),
            _context: PhantomData,
        }
    }

    /// Marks all operations of the dialect `name` as legal.
    pub fn add_legal_dialect(&mut self, name: &str) {
        // SAFETY: The target is valid; MLIR copies the name.
        unsafe { mlirConversionTargetAddLegalDialect(self.raw, string_ref(name)) }
    }

    /// Marks all operations of the dialect `name` as illegal.
    pub fn add_illegal_dialect(&mut self, name: &str) {
        // SAFETY: The target is valid; MLIR copies the name.
        unsafe { mlirConversionTargetAddIllegalDialect(self.raw, string_ref(name)) }
    }

    /// Marks the operation `name` as legal.
    pub fn add_legal_op(&mut self, name: &str) {
        // SAFETY: The target is valid; MLIR copies the name.
        unsafe { mlirConversionTargetAddLegalOp(self.raw, string_ref(name)) }
    }

    /// Marks the operation `name` as illegal.
    pub fn add_illegal_op(&mut self, name: &str) {
        // SAFETY: The target is valid; MLIR copies the name.
        unsafe { mlirConversionTargetAddIllegalOp(self.raw, string_ref(name)) }
    }

    /// Marks the operation `name` as legal whenever `is_legal` returns true for an instance of it.
    pub fn add_dynamically_legal_op<F>(&mut self, name: &str, is_legal: F)
    where
        F: Fn(OperationRef<'_, '_>) -> bool + 'static,
    {
        let callback = Box::new(is_legal);
        let user_data = ptr::from_ref::<F>(&callback).cast_mut().cast::<c_void>();

        // SAFETY: The target is valid. `user_data` points to the boxed callback,
        // which `self.callbacks` keeps alive (at a stable address) for as long
        // as the target exists.
        unsafe {
            mlirConversionTargetAddDynamicallyLegalOp(
                self.raw,
                string_ref(name),
                Some(legality::<F>),
                user_data,
            );
        }
        self.callbacks.push(callback);
    }
}

impl Drop for ConversionTarget<'_> {
    fn drop(&mut self) {
        // SAFETY: The target is valid and destroyed only once. The callbacks
        // are dropped afterwards, when the fields are dropped.
        unsafe { mlirConversionTargetDestroy(self.raw) }
    }
}

unsafe extern "C" fn legality<F>(
    operation: MlirOperation,
    user_data: *mut c_void,
) -> MlirConversionTargetLegality
where
    F: Fn(OperationRef<'_, '_>) -> bool,
{
    // SAFETY: `user_data` points to an `F` owned by the conversion target, and
    // MLIR passes a valid operation.
    let (is_legal, operation) =
        unsafe { (&*user_data.cast::<F>(), OperationRef::from_raw(operation)) };

    if is_legal(operation) {
        MlirConversionTargetLegality_MLIR_CONVERSION_TARGET_LEGALITY_LEGAL
    } else {
        MlirConversionTargetLegality_MLIR_CONVERSION_TARGET_LEGALITY_ILLEGAL
    }
}

/// Describes how types change during a conversion.
pub struct TypeConverter {
    raw: MlirTypeConverter,
    /// Conversion callbacks, owned here and borrowed by `raw`.
    conversions: Vec<Box<TypeConversion>>,
}

impl TypeConverter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            // SAFETY: Creating a type converter has no preconditions.
            raw: unsafe { mlirTypeConverterCreate() },
            conversions: Vec::new(),
        }
    }

    /// Registers a conversion function.
    ///
    /// When converting a type, the conversions are tried from the most recently added to the oldest, until one returns `Some`.
    /// So register a catch-all conversion (such as the identity) first.
    pub fn add_conversion<F>(&mut self, conversion: F)
    where
        F: for<'c> Fn(Type<'c>) -> Option<Type<'c>> + 'static,
    {
        let callback = Box::new(conversion);
        let user_data = ptr::from_ref::<F>(&callback).cast_mut().cast::<c_void>();

        // SAFETY: The converter is valid. `user_data` points to the boxed
        // callback, which `self.conversions` keeps alive (at a stable address)
        // for as long as the converter exists.
        unsafe { mlirTypeConverterAddConversion(self.raw, Some(convert_type::<F>), user_data) };
        self.conversions.push(callback);
    }

    /// Converts `type`, or returns `None` if no conversion applies.
    #[must_use]
    pub fn convert_type<'c>(&self, r#type: Type<'c>) -> Option<Type<'c>> {
        // SAFETY: The converter and the type are valid.
        let converted = unsafe { mlirTypeConverterConvertType(self.raw, r#type.to_raw()) };

        // SAFETY: A non-null result is a valid type of the same context.
        (!converted.ptr.is_null()).then(|| unsafe { Type::from_raw(converted) })
    }
}

impl Default for TypeConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TypeConverter {
    fn drop(&mut self) {
        // SAFETY: The converter is valid and destroyed only once. The callbacks
        // are dropped afterwards, when the fields are dropped.
        unsafe { mlirTypeConverterDestroy(self.raw) }
    }
}

unsafe extern "C" fn convert_type<F>(
    r#type: MlirType,
    converted: *mut MlirType,
    user_data: *mut c_void,
) -> MlirLogicalResult
where
    F: for<'c> Fn(Type<'c>) -> Option<Type<'c>>,
{
    // SAFETY: `user_data` points to an `F` owned by the type converter, and
    // MLIR passes a valid type.
    let (conversion, r#type) = unsafe { (&*user_data.cast::<F>(), Type::from_raw(r#type)) };

    conversion(r#type).map_or(FAILURE, |r#type| {
        // SAFETY: MLIR passes a valid pointer for the result.
        unsafe { *converted = r#type.to_raw() };
        SUCCESS
    })
}

/// A set of conversion patterns, like `RewritePatternSet` filled with `OpConversionPattern`s in C++.
pub struct ConversionPatternSet<'c, 't> {
    raw: MlirRewritePatternSet,
    context: &'c Context,
    _type_converter: PhantomData<&'t TypeConverter>,
}

impl<'c, 't> ConversionPatternSet<'c, 't> {
    #[must_use]
    pub fn new(context: &'c Context) -> Self {
        Self {
            // SAFETY: The context is valid.
            raw: unsafe { mlirRewritePatternSetCreate(context.to_raw()) },
            context,
            _type_converter: PhantomData,
        }
    }

    /// Adds a pattern that converts operations named `root_name`, like subclassing `mlir::OpConversionPattern` in C++.
    ///
    /// `pattern` is the `matchAndRewrite` method.
    /// Besides the context and the operation, it receives the operation's operands *after* conversion (the "adaptor" in C++): if an operand was produced by an operation that has already been converted, this is the replacement value, with its new type.
    /// The pattern must make all IR changes through the rewriter, as the framework may roll them back.
    pub fn add<F>(&mut self, root_name: &str, type_converter: &'t TypeConverter, pattern: F)
    where
        F: for<'x> Fn(
                &'x Context,
                OperationRef<'x, '_>,
                &[Value<'x, '_>],
                RewriterBase<'x, '_>,
            ) -> LogicalResult
            + 'static,
    {
        let callbacks = MlirConversionPatternCallbacks {
            construct: None,
            destruct: Some(destruct_pattern::<F>),
            matchAndRewrite: Some(match_and_rewrite::<F>),
        };
        let user_data = Box::into_raw(Box::new(pattern)).cast::<c_void>();

        // SAFETY: All handles are valid. The pattern owns `user_data` and
        // releases it through `destruct_pattern`. The type converter outlives
        // the pattern set (`'t`). The set takes ownership of the pattern.
        unsafe {
            let pattern = mlirOpConversionPatternCreate(
                string_ref(root_name),
                1,
                self.context.to_raw(),
                type_converter.raw,
                callbacks,
                user_data,
                0,
                ptr::null_mut(),
            );
            mlirRewritePatternSetAdd(self.raw, mlirConversionPatternAsRewritePattern(pattern));
        }
    }

    /// Freezes the set, transferring ownership of the frozen set to the caller.
    fn freeze(self) -> MlirFrozenRewritePatternSet {
        let raw = self.raw;
        mem::forget(self);

        // SAFETY: The set is valid; freezing consumes it.
        unsafe { mlirFreezeRewritePattern(raw) }
    }
}

impl Drop for ConversionPatternSet<'_, '_> {
    fn drop(&mut self) {
        // SAFETY: The set is valid and was not frozen (freezing forgets it).
        unsafe { mlirRewritePatternSetDestroy(self.raw) }
    }
}

unsafe extern "C" fn match_and_rewrite<F>(
    _: MlirConversionPattern,
    operation: MlirOperation,
    operand_count: isize,
    operands: *mut MlirValue,
    rewriter: MlirConversionPatternRewriter,
    user_data: *mut c_void,
) -> MlirLogicalResult
where
    F: for<'c> Fn(
        &'c Context,
        OperationRef<'c, '_>,
        &[Value<'c, '_>],
        RewriterBase<'c, '_>,
    ) -> LogicalResult,
{
    // SAFETY: `user_data` points to the `F` owned by the pattern. MLIR passes a
    // valid operation, `operand_count` valid values, and a valid rewriter, all
    // alive for the duration of this callback.
    let (pattern, operation, operands, rewriter) = unsafe {
        let operands = if operand_count == 0 {
            &[]
        } else {
            slice::from_raw_parts(operands, operand_count as usize)
        };

        (
            &*user_data.cast::<F>(),
            OperationRef::from_raw(operation),
            operands
                .iter()
                .map(|&operand| Value::from_raw(operand))
                .collect::<Vec<_>>(),
            PatternRewriter::from_raw(mlirConversionPatternRewriterAsPatternRewriter(rewriter)),
        )
    };

    let context = operation.context();
    // SAFETY: `context` outlives the call to `pattern`, which cannot keep the
    // reference because of its higher-ranked lifetime.
    let context = unsafe { context.to_ref() };

    match pattern(context, operation, &operands, rewriter.as_rewriter_base()) {
        Ok(()) => SUCCESS,
        Err(_) => FAILURE,
    }
}

unsafe extern "C" fn destruct_pattern<F>(user_data: *mut c_void) {
    // SAFETY: `user_data` is the `Box<F>` created by `ConversionPatternSet::add`,
    // and MLIR calls this exactly once when it destroys the pattern.
    drop(unsafe { Box::from_raw(user_data.cast::<F>()) });
}

/// Converts the illegal operations nested in `operation`, leaving operations the target does not know about untouched.
pub fn apply_partial_conversion(
    operation: OperationRef,
    target: &ConversionTarget,
    patterns: ConversionPatternSet,
) -> Result<(), Error> {
    apply_conversion(operation, target, patterns, mlirApplyPartialConversion)
}

/// Converts all operations nested in `operation`, failing if any operation is not legal afterwards.
pub fn apply_full_conversion(
    operation: OperationRef,
    target: &ConversionTarget,
    patterns: ConversionPatternSet,
) -> Result<(), Error> {
    apply_conversion(operation, target, patterns, mlirApplyFullConversion)
}

type ApplyConversion = unsafe extern "C" fn(
    MlirOperation,
    MlirConversionTarget,
    MlirFrozenRewritePatternSet,
    MlirConversionConfig,
) -> MlirLogicalResult;

fn apply_conversion(
    operation: OperationRef,
    target: &ConversionTarget,
    patterns: ConversionPatternSet,
    apply: ApplyConversion,
) -> Result<(), Error> {
    let patterns = patterns.freeze();

    // SAFETY: All handles are valid. The C API only borrows the frozen
    // patterns and the configuration, so both are destroyed afterwards.
    let result = unsafe {
        let config = mlirConversionConfigCreate();
        let result = apply(operation.to_raw(), target.raw, patterns, config);
        mlirConversionConfigDestroy(config);
        mlirFrozenRewritePatternSetDestroy(patterns);
        result
    };

    if result.value != 0 {
        Ok(())
    } else {
        Err(Error::Conversion)
    }
}

/// Replaces the operands of `operation` in place, notifying the rewriter.
///
/// This corresponds to `rewriter.modifyOpInPlace(op, [&] { op->setOperands(operands); })` in C++.
/// A conversion pattern must not modify an operation behind the rewriter's back, as the framework could not roll the change back.
pub fn modify_op_operands(rewriter: RewriterBase, operation: OperationRef, operands: &[Value]) {
    let operands = operands.iter().map(ValueLike::to_raw).collect::<Vec<_>>();

    rewriter.start_op_modification(operation);
    // SAFETY: The operation and the values are valid; MLIR copies the list.
    unsafe {
        mlirOperationSetOperands(
            operation.to_raw(),
            operands.len().cast_signed(),
            operands.as_ptr(),
        );
    }
    rewriter.finalize_op_modification(operation);
}
