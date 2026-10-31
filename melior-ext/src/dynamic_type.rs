//! Types of dialects defined at runtime.
//!
//! In C++, a dialect type such as Toy's `StructType` is a class with a storage class that MLIR uniques by its parameters.
//! A type declared in IRDL is a `DynamicType` instead: an instance of a `DynamicTypeDefinition` together with a list of attribute parameters.
//! MLIR uniques it the same way, so two struct types with equal parameters are the same `Type`.

use std::marker::PhantomData;

use melior::{
    Context,
    ir::{Attribute, Type, TypeLike, attribute::AttributeLike},
};
use mlir_sys::{
    MlirAttribute, MlirDynamicTypeDefinition, mlirContextGetOrLoadDialect,
    mlirDialectIsAExtensibleDialect, mlirDynamicTypeDefinitionGetName, mlirDynamicTypeGet,
    mlirDynamicTypeGetNumParams, mlirDynamicTypeGetParam, mlirDynamicTypeGetTypeDef,
    mlirExtensibleDialectLookupTypeDefinition, mlirTypeIsADynamicType,
};

use crate::string::{string_ref, to_string};

/// The definition of a type of an extensible (for example IRDL) dialect.
#[derive(Clone, Copy, Debug)]
pub struct DynamicTypeDefinition<'c> {
    raw: MlirDynamicTypeDefinition,
    _context: PhantomData<&'c Context>,
}

impl<'c> DynamicTypeDefinition<'c> {
    /// Looks up the type `name` (without the dialect prefix) of the dialect `dialect`, loading the dialect if needed.
    ///
    /// Returns `None` if the dialect is unknown, is not extensible, or has no such type.
    #[must_use]
    pub fn lookup(context: &'c Context, dialect: &str, name: &str) -> Option<Self> {
        // SAFETY: The context is valid. The dialect is only cast to an
        // extensible dialect after checking that it is one.
        let raw = unsafe {
            let dialect = mlirContextGetOrLoadDialect(context.to_raw(), string_ref(dialect));
            if dialect.ptr.is_null() || !mlirDialectIsAExtensibleDialect(dialect) {
                return None;
            }
            mlirExtensibleDialectLookupTypeDefinition(dialect, string_ref(name))
        };

        (!raw.ptr.is_null()).then_some(Self {
            raw,
            _context: PhantomData,
        })
    }

    /// Returns the name of the type, without the dialect prefix.
    #[must_use]
    pub fn name(&self) -> String {
        // SAFETY: The definition is valid and its name lives in the context.
        unsafe { to_string(mlirDynamicTypeDefinitionGetName(self.raw)) }
    }

    /// Returns the (uniqued) type with the given parameters.
    ///
    /// The parameters must satisfy the type's constraints (for IRDL, the constraints of its `irdl.parameters`).
    /// MLIR only checks them in builds with assertions enabled.
    #[must_use]
    pub fn instantiate(&self, parameters: &[Attribute<'c>]) -> Type<'c> {
        let mut parameters = parameters
            .iter()
            .map(AttributeLike::to_raw)
            .collect::<Vec<MlirAttribute>>();

        // SAFETY: The definition is valid and the parameters are valid
        // attributes of the same context. MLIR copies the parameter list.
        unsafe {
            Type::from_raw(mlirDynamicTypeGet(
                self.raw,
                parameters.as_mut_ptr(),
                parameters.len().cast_signed(),
            ))
        }
    }
}

impl PartialEq for DynamicTypeDefinition<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.raw.ptr == other.raw.ptr
    }
}

impl Eq for DynamicTypeDefinition<'_> {}

/// Returns the definition of `type`, or `None` if it is not a dynamic type.
#[must_use]
pub fn dynamic_type_definition(r#type: Type<'_>) -> Option<DynamicTypeDefinition<'_>> {
    // SAFETY: The type is valid, and only queried for its definition after
    // checking that it is a dynamic type.
    unsafe {
        mlirTypeIsADynamicType(r#type.to_raw()).then(|| DynamicTypeDefinition {
            raw: mlirDynamicTypeGetTypeDef(r#type.to_raw()),
            _context: PhantomData,
        })
    }
}

/// Returns the parameters of `type`, or `None` if it is not a dynamic type.
#[must_use]
pub fn dynamic_type_parameters(r#type: Type<'_>) -> Option<Vec<Attribute<'_>>> {
    let raw = r#type.to_raw();

    // SAFETY: The type is valid, and only queried for its parameters after
    // checking that it is a dynamic type. Indices are within bounds.
    unsafe {
        mlirTypeIsADynamicType(raw).then(|| {
            (0..mlirDynamicTypeGetNumParams(raw))
                .map(|index| Attribute::from_raw(mlirDynamicTypeGetParam(raw, index)))
                .collect()
        })
    }
}
