//! Typed wrappers for the operations of the Toy dialect.
//!
//! For every operation in `Ops.td`, ODS generates a C++ class: a thin handle around `mlir::Operation*` with a `build` method (called through `ConstantOp::create(builder, ...)`) and named accessors such as `getLhs()`.
//! The structs below play the same role for our IRDL-defined operations: `AddOp::build` creates an `add` operation, and `AddOp::cast(operation)` views a generic operation as an `AddOp`, like `llvm::dyn_cast<AddOp>(op)` in C++.
//!
//! The accessors assume a verified operation, as their C++ counterparts do.

use melior::{
    Context,
    ir::{
        Identifier, Location, Operation, OperationRef, Type, Value, ValueLike,
        attribute::{ArrayAttribute, AttributeLike, DenseElementsAttribute, IntegerAttribute},
        operation::{OperationBuilder, OperationLike, OperationResult},
        r#type::{IntegerType, RankedTensorType},
    },
};

use crate::dialect::types::{StructType, unranked_f64_tensor};

/// Common interface of the Toy operation wrappers.
pub trait ToyOp<'c: 'a, 'a>: Copy {
    /// The full name of the operation, such as `toy.add`.
    const NAME: &'static str;

    /// Wraps `operation` without checking its name.
    fn from_operation_unchecked(operation: OperationRef<'c, 'a>) -> Self;

    /// Returns the wrapped operation.
    fn operation(&self) -> OperationRef<'c, 'a>;

    /// Returns `operation` as this operation type, or `None` if it is a different operation (like `llvm::dyn_cast<Op>(op)`).
    #[must_use]
    fn cast(operation: OperationRef<'c, 'a>) -> Option<Self> {
        is_operation(operation, Self::NAME).then(|| Self::from_operation_unchecked(operation))
    }

    /// Returns the operation of this type that defines `value`, or `None` if it is defined by something else (like `value.getDefiningOp<Op>()`).
    #[must_use]
    fn defining(value: Value<'c, 'a>) -> Option<Self> {
        let result = OperationResult::try_from(value).ok()?;
        Self::cast(result.owner())
    }
}

/// Returns whether `operation` is named `name`.
#[must_use]
pub fn is_operation(operation: OperationRef, name: &str) -> bool {
    operation.name().as_string_ref().as_str() == Ok(name)
}

/// Declares the wrapper struct of an operation and implements [`ToyOp`] for it: the boilerplate that ODS would generate.
macro_rules! toy_op {
    ($(#[$attribute:meta])* $name:ident = $op_name:literal) => {
        $(#[$attribute])*
        #[derive(Clone, Copy, Debug)]
        pub struct $name<'c, 'a> {
            operation: OperationRef<'c, 'a>,
        }

        impl<'c: 'a, 'a> ToyOp<'c, 'a> for $name<'c, 'a> {
            const NAME: &'static str = $op_name;

            fn from_operation_unchecked(operation: OperationRef<'c, 'a>) -> Self {
                Self { operation }
            }

            fn operation(&self) -> OperationRef<'c, 'a> {
                self.operation
            }
        }
    };
}

toy_op! {
    /// `toy.constant`: turns a literal into an SSA value.
    ///
    /// ```mlir
    /// %0 = "toy.constant"() {value = dense<[1.0, 2.0]> : tensor<2xf64>} : () -> tensor<2xf64>
    /// ```
    ConstantOp = "toy.constant"
}

impl<'c: 'a, 'a> ConstantOp<'c, 'a> {
    /// Builds a constant holding `value`; its type is the type of `value`.
    #[must_use]
    pub fn build(
        context: &'c Context,
        location: Location<'c>,
        value: DenseElementsAttribute<'c>,
    ) -> Operation<'c> {
        let r#type = value.r#type();
        Self::build_with_type(context, location, r#type, value)
    }

    /// Builds a constant holding `value` with the result type `type`, which can be less precise than the type of `value` (such as `tensor<*xf64>`).
    #[must_use]
    pub fn build_with_type(
        context: &'c Context,
        location: Location<'c>,
        r#type: Type<'c>,
        value: DenseElementsAttribute<'c>,
    ) -> Operation<'c> {
        OperationBuilder::new(Self::NAME, location)
            .add_attributes(&[(Identifier::new(context, "value"), value.into())])
            .add_results(&[r#type])
            .build()
            .expect("valid toy.constant")
    }

    /// Builds a constant scalar: a tensor of rank 0, `tensor<f64>`.
    #[must_use]
    pub fn build_scalar(context: &'c Context, location: Location<'c>, value: f64) -> Operation<'c> {
        let r#type = RankedTensorType::new(&[], Type::float64(context), None);
        Self::build(
            context,
            location,
            DenseElementsAttribute::f64_values(r#type.into(), &[value]),
        )
    }

    /// Returns the attribute holding the data.
    #[must_use]
    pub fn value(&self) -> DenseElementsAttribute<'c> {
        let value = self
            .operation
            .attribute("value")
            .expect("toy.constant has a value");
        DenseElementsAttribute::try_from(value).expect("value is a dense elements attribute")
    }

    #[must_use]
    pub fn output(&self) -> Value<'c, 'a> {
        result(self.operation)
    }
}

toy_op! {
    /// `toy.add`: element-wise addition of two tensors.
    AddOp = "toy.add"
}

impl<'c: 'a, 'a> AddOp<'c, 'a> {
    /// Builds an addition.
    /// Its result type is `tensor<*xf64>` until shape inference refines it.
    #[must_use]
    pub fn build(
        context: &'c Context,
        location: Location<'c>,
        lhs: Value<'c, '_>,
        rhs: Value<'c, '_>,
    ) -> Operation<'c> {
        build_binary(context, Self::NAME, location, lhs, rhs)
    }

    #[must_use]
    pub fn lhs(&self) -> Value<'c, 'a> {
        operand(self.operation, 0)
    }

    #[must_use]
    pub fn rhs(&self) -> Value<'c, 'a> {
        operand(self.operation, 1)
    }

    #[must_use]
    pub fn output(&self) -> Value<'c, 'a> {
        result(self.operation)
    }
}

toy_op! {
    /// `toy.mul`: element-wise multiplication of two tensors.
    MulOp = "toy.mul"
}

impl<'c: 'a, 'a> MulOp<'c, 'a> {
    /// Builds a multiplication.
    /// Its result type is `tensor<*xf64>` until shape inference refines it.
    #[must_use]
    pub fn build(
        context: &'c Context,
        location: Location<'c>,
        lhs: Value<'c, '_>,
        rhs: Value<'c, '_>,
    ) -> Operation<'c> {
        build_binary(context, Self::NAME, location, lhs, rhs)
    }

    #[must_use]
    pub fn lhs(&self) -> Value<'c, 'a> {
        operand(self.operation, 0)
    }

    #[must_use]
    pub fn rhs(&self) -> Value<'c, 'a> {
        operand(self.operation, 1)
    }

    #[must_use]
    pub fn output(&self) -> Value<'c, 'a> {
        result(self.operation)
    }
}

fn build_binary<'c>(
    context: &'c Context,
    name: &str,
    location: Location<'c>,
    lhs: Value<'c, '_>,
    rhs: Value<'c, '_>,
) -> Operation<'c> {
    OperationBuilder::new(name, location)
        .add_operands(&[lhs, rhs])
        .add_results(&[unranked_f64_tensor(context)])
        .build()
        .expect("valid binary operation")
}

toy_op! {
    /// `toy.cast`: converts a tensor to an equivalent tensor type, such as `tensor<2x3xf64>` to `tensor<*xf64>`, without changing the data.
    CastOp = "toy.cast"
}

impl<'c: 'a, 'a> CastOp<'c, 'a> {
    #[must_use]
    pub fn build(location: Location<'c>, input: Value<'c, '_>, r#type: Type<'c>) -> Operation<'c> {
        OperationBuilder::new(Self::NAME, location)
            .add_operands(&[input])
            .add_results(&[r#type])
            .build()
            .expect("valid toy.cast")
    }

    #[must_use]
    pub fn input(&self) -> Value<'c, 'a> {
        operand(self.operation, 0)
    }

    #[must_use]
    pub fn output(&self) -> Value<'c, 'a> {
        result(self.operation)
    }
}

toy_op! {
    /// `toy.transpose`: transposes a tensor.
    TransposeOp = "toy.transpose"
}

impl<'c: 'a, 'a> TransposeOp<'c, 'a> {
    /// Builds a transpose.
    /// Its result type is `tensor<*xf64>` until shape inference refines it.
    #[must_use]
    pub fn build(
        context: &'c Context,
        location: Location<'c>,
        input: Value<'c, '_>,
    ) -> Operation<'c> {
        OperationBuilder::new(Self::NAME, location)
            .add_operands(&[input])
            .add_results(&[unranked_f64_tensor(context)])
            .build()
            .expect("valid toy.transpose")
    }

    #[must_use]
    pub fn input(&self) -> Value<'c, 'a> {
        operand(self.operation, 0)
    }

    #[must_use]
    pub fn output(&self) -> Value<'c, 'a> {
        result(self.operation)
    }
}

toy_op! {
    /// `toy.reshape`: changes the shape of a tensor, keeping its elements.
    ReshapeOp = "toy.reshape"
}

impl<'c: 'a, 'a> ReshapeOp<'c, 'a> {
    /// Builds a reshape to `type`, which must be statically shaped.
    #[must_use]
    pub fn build(location: Location<'c>, input: Value<'c, '_>, r#type: Type<'c>) -> Operation<'c> {
        OperationBuilder::new(Self::NAME, location)
            .add_operands(&[input])
            .add_results(&[r#type])
            .build()
            .expect("valid toy.reshape")
    }

    #[must_use]
    pub fn input(&self) -> Value<'c, 'a> {
        operand(self.operation, 0)
    }

    #[must_use]
    pub fn output(&self) -> Value<'c, 'a> {
        result(self.operation)
    }
}

toy_op! {
    /// `toy.print`: prints a tensor (or, after partial lowering, a memref).
    PrintOp = "toy.print"
}

impl<'c: 'a, 'a> PrintOp<'c, 'a> {
    #[must_use]
    pub fn build(location: Location<'c>, input: Value<'c, '_>) -> Operation<'c> {
        OperationBuilder::new(Self::NAME, location)
            .add_operands(&[input])
            .build()
            .expect("valid toy.print")
    }

    #[must_use]
    pub fn input(&self) -> Value<'c, 'a> {
        operand(self.operation, 0)
    }
}

toy_op! {
    /// `toy.return`: returns from a function, optionally with a value.
    /// This is the terminator of Toy functions.
    ReturnOp = "toy.return"
}

impl<'c: 'a, 'a> ReturnOp<'c, 'a> {
    #[must_use]
    pub fn build(location: Location<'c>, input: Option<Value<'c, '_>>) -> Operation<'c> {
        OperationBuilder::new(Self::NAME, location)
            .add_operands(input.as_slice())
            .build()
            .expect("valid toy.return")
    }

    /// Returns the returned value, if any.
    #[must_use]
    pub fn input(&self) -> Option<Value<'c, 'a>> {
        self.operation.operand(0).ok()
    }
}

toy_op! {
    /// `toy.struct_constant`: turns a literal struct into an SSA value (chapter 7).
    /// The value is an array with the constant value of each element: a dense elements attribute for a tensor, or a nested array for a struct.
    StructConstantOp = "toy.struct_constant"
}

impl<'c: 'a, 'a> StructConstantOp<'c, 'a> {
    #[must_use]
    pub fn build(
        context: &'c Context,
        location: Location<'c>,
        r#type: StructType<'c>,
        value: ArrayAttribute<'c>,
    ) -> Operation<'c> {
        OperationBuilder::new(Self::NAME, location)
            .add_attributes(&[(Identifier::new(context, "value"), value.into())])
            .add_results(&[r#type.into()])
            .build()
            .expect("valid toy.struct_constant")
    }

    #[must_use]
    pub fn value(&self) -> ArrayAttribute<'c> {
        let value = self
            .operation
            .attribute("value")
            .expect("toy.struct_constant has a value");
        ArrayAttribute::try_from(value).expect("value is an array")
    }

    #[must_use]
    pub fn output(&self) -> Value<'c, 'a> {
        result(self.operation)
    }
}

toy_op! {
    /// `toy.struct_access`: reads the element at a constant index of a struct (chapter 7).
    StructAccessOp = "toy.struct_access"
}

impl<'c: 'a, 'a> StructAccessOp<'c, 'a> {
    /// Builds an access of the element `index` of `input`, which must be a struct with more than `index` elements.
    #[must_use]
    pub fn build(
        context: &'c Context,
        location: Location<'c>,
        input: Value<'c, '_>,
        index: usize,
    ) -> Operation<'c> {
        let struct_type = StructType::from_type(input.r#type()).expect("input is a struct");
        let element_types = struct_type.element_types();
        assert!(index < element_types.len(), "struct index is out of bounds");

        let index_attribute = IntegerAttribute::new(
            IntegerType::new(context, 64).into(),
            i64::try_from(index).expect("the index is within the struct"),
        );
        OperationBuilder::new(Self::NAME, location)
            .add_operands(&[input])
            .add_attributes(&[(Identifier::new(context, "index"), index_attribute.into())])
            .add_results(&[element_types[index]])
            .build()
            .expect("valid toy.struct_access")
    }

    #[must_use]
    pub fn input(&self) -> Value<'c, 'a> {
        operand(self.operation, 0)
    }

    #[must_use]
    pub fn index(&self) -> usize {
        let index = self
            .operation
            .attribute("index")
            .expect("toy.struct_access has an index");
        let index = IntegerAttribute::try_from(index)
            .expect("index is an integer")
            .value();
        usize::try_from(index).expect("the verifier checked the index")
    }

    #[must_use]
    pub fn output(&self) -> Value<'c, 'a> {
        result(self.operation)
    }
}

fn operand<'c: 'a, 'a>(operation: OperationRef<'c, 'a>, index: usize) -> Value<'c, 'a> {
    operation
        .operand(index)
        .expect("verified operations have their operands")
}

fn result<'c: 'a, 'a>(operation: OperationRef<'c, 'a>) -> Value<'c, 'a> {
    operation
        .result(0)
        .expect("verified operations have their result")
        .into()
}
