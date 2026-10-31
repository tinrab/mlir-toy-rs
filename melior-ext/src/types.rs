use melior::ir::{Type, TypeLike};
use mlir_sys::{mlirShapedTypeGetElementType, mlirUnrankedTensorTypeGet};

/// Returns the unranked tensor type with the given element type, such as `tensor<*xf64>`.
#[must_use]
pub fn unranked_tensor_type(element_type: Type) -> Type {
    // SAFETY: The element type is valid; the result belongs to its context.
    unsafe { Type::from_raw(mlirUnrankedTensorTypeGet(element_type.to_raw())) }
}

/// Returns the element type of a shaped type (a ranked or unranked tensor, a memref, or a vector), or `None` for other types.
#[must_use]
pub fn element_type(r#type: Type) -> Option<Type> {
    // SAFETY: The type is shaped, so it has an element type.
    r#type
        .is_shaped()
        .then(|| unsafe { Type::from_raw(mlirShapedTypeGetElementType(r#type.to_raw())) })
}
