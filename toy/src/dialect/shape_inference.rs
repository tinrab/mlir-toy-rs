//! The shape inference interface (chapter 4).
//!
//! Toy functions are generic: their parameters are `tensor<*xf64>`.
//! Once all calls are inlined into `main`, the shapes of all values can be inferred, starting from the constants.
//! In C++, the tutorial defines an *operation interface* for this in ODS (`ShapeInferenceOpInterface`): operations declare that they implement it, and the shape inference pass works with any operation through the interface, without knowing the concrete operations.
//!
//! Here, the interface is the [`ShapeInference`] trait, and [`as_shape_inference`] plays the role of `dyn_cast<ShapeInference>(op)`.

use melior::ir::{
    OperationRef, ValueLike,
    attribute::AttributeLike,
    r#type::{RankedTensorType, ShapedTypeLike},
};

use crate::dialect::{
    ops::{AddOp, CastOp, ConstantOp, MulOp, ToyOp, TransposeOp},
    types::dimensions,
};

/// An operation that can compute its result types from its operand types.
pub trait ShapeInference {
    /// Infers the result types from the operand types, and updates the results in place.
    /// All operands must have ranked types.
    fn infer_shapes(&self);
}

/// Returns `operation` as a [`ShapeInference`] implementation, or `None` if it does not implement the interface.
#[must_use]
pub fn as_shape_inference<'c: 'a, 'a>(
    operation: OperationRef<'c, 'a>,
) -> Option<Box<dyn ShapeInference + 'a>> {
    fn boxed<'c: 'a, 'a, T: ToyOp<'c, 'a> + ShapeInference + 'a>(
        operation: OperationRef<'c, 'a>,
    ) -> Option<Box<dyn ShapeInference + 'a>> {
        T::cast(operation).map(|operation| Box::new(operation) as Box<dyn ShapeInference>)
    }

    boxed::<ConstantOp>(operation)
        .or_else(|| boxed::<AddOp>(operation))
        .or_else(|| boxed::<MulOp>(operation))
        .or_else(|| boxed::<CastOp>(operation))
        .or_else(|| boxed::<TransposeOp>(operation))
}

/// The result has the type of the constant data.
impl ShapeInference for ConstantOp<'_, '_> {
    fn infer_shapes(&self) {
        self.output().set_type(self.value().r#type());
    }
}

/// The result has the type of the operands, which are expected to match.
impl ShapeInference for AddOp<'_, '_> {
    fn infer_shapes(&self) {
        self.output().set_type(self.lhs().r#type());
    }
}

/// The result has the type of the operands, which are expected to match.
impl ShapeInference for MulOp<'_, '_> {
    fn infer_shapes(&self) {
        self.output().set_type(self.lhs().r#type());
    }
}

/// A cast does not change the shape, so the result has the input's type.
/// The cast then converts a type to itself and canonicalization removes it.
impl ShapeInference for CastOp<'_, '_> {
    fn infer_shapes(&self) {
        self.output().set_type(self.input().r#type());
    }
}

/// The result's shape is the input's shape reversed.
impl ShapeInference for TransposeOp<'_, '_> {
    fn infer_shapes(&self) {
        let input = RankedTensorType::try_from(self.input().r#type())
            .expect("shape inference runs once the operands are ranked");
        let mut shape = dimensions(input);
        shape.reverse();

        self.output()
            .set_type(RankedTensorType::new(&shape, input.element(), None).into());
    }
}
