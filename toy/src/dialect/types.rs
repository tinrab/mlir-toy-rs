//! The types of the Toy dialect.
//!
//! Toy values are tensors of `f64`, which use MLIR's builtin tensor types: `tensor<2x3xf64>` when the shape is known and `tensor<*xf64>` when it is not.
//! Chapter 7 adds the dialect's only custom type, a struct.

use melior::{
    Context,
    ir::{
        Type, TypeLike,
        attribute::{ArrayAttribute, TypeAttribute},
        r#type::{DimSize, ShapedTypeLike},
    },
};
use melior_ext::{
    DynamicTypeDefinition, dynamic_type_definition, dynamic_type_parameters, unranked_tensor_type,
};

/// Returns `tensor<*xf64>`, the type of a Toy value whose shape is not known yet.
#[must_use]
pub fn unranked_f64_tensor(context: &Context) -> Type<'_> {
    unranked_tensor_type(Type::float64(context))
}

/// The value MLIR uses for a dynamic dimension (`ShapedType::kDynamic`).
const DYNAMIC_DIMENSION: u64 = i64::MIN as u64;

/// Returns the dimensions of a ranked shaped type (a ranked tensor or a memref), like `getShape()` in C++.
///
/// Toy only creates static shapes, but MLIR input could contain dynamic dimensions (`?`); they are returned as MLIR's dynamic marker, so that the result can be passed back to `RankedTensorType::new`.
pub fn dimensions<'c>(r#type: impl ShapedTypeLike<'c> + Copy) -> Vec<u64> {
    (0..r#type.rank())
        .map(|dimension| match r#type.dim_size(dimension) {
            Ok(DimSize::Static(size)) => size,
            Ok(DimSize::Dynamic) => DYNAMIC_DIMENSION,
            Err(_) => unreachable!("dimension is within the rank"),
        })
        .collect()
}

/// A Toy struct type, such as `!toy.struct<[tensor<*xf64>, tensor<*xf64>]>`.
///
/// In C++ (chapter 7), `StructType` is a class deriving from `mlir::Type::TypeBase` with a `StructTypeStorage` holding the element types; MLIR uniques instances by their elements.
/// Our struct type is declared in IRDL instead (see `toy.irdl.mlir`): a dynamic type with one parameter, the array of element types.
/// This wrapper gives it the same interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructType<'c> {
    r#type: Type<'c>,
}

impl<'c> StructType<'c> {
    /// Returns the struct type with the given element types.
    /// There must be at least one element, and each must be a tensor or a struct.
    #[must_use]
    pub fn new(context: &'c Context, element_types: &[Type<'c>]) -> Self {
        assert!(
            !element_types.is_empty(),
            "a struct must have at least one element"
        );

        let definition = DynamicTypeDefinition::lookup(context, "toy", "struct")
            .expect("the toy dialect must be loaded");
        let element_types = element_types
            .iter()
            .map(|&r#type| TypeAttribute::new(r#type).into())
            .collect::<Vec<_>>();

        Self {
            r#type: definition.instantiate(&[ArrayAttribute::new(context, &element_types).into()]),
        }
    }

    /// Returns `type` as a struct type, or `None` if it is not one.
    #[must_use]
    pub fn from_type(r#type: Type<'c>) -> Option<Self> {
        let definition = dynamic_type_definition(r#type)?;
        let is_toy_struct =
            definition.name() == "struct" && r#type.dialect().namespace().ok() == Some("toy");

        is_toy_struct.then_some(Self { r#type })
    }

    /// Returns the types of the struct's elements.
    #[must_use]
    pub fn element_types(&self) -> Vec<Type<'c>> {
        let parameters =
            dynamic_type_parameters(self.r#type).expect("a struct type is a dynamic type");
        let elements = ArrayAttribute::try_from(parameters[0])
            .expect("the IRDL definition requires an array parameter");

        (0..elements.len())
            .map(|index| {
                let element = elements.element(index).expect("index is in bounds");
                TypeAttribute::try_from(element)
                    .expect("struct elements are types")
                    .value()
            })
            .collect()
    }

    /// Returns the underlying MLIR type.
    #[must_use]
    pub fn r#type(&self) -> Type<'c> {
        self.r#type
    }
}

impl<'c> From<StructType<'c>> for Type<'c> {
    fn from(r#type: StructType<'c>) -> Self {
        r#type.r#type
    }
}
