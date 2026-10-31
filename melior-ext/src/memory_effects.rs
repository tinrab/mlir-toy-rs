use std::{ffi::c_void, ptr};

use melior::Context;
use mlir_sys::{
    MlirConditionallySpeculatableOpInterfaceCallbacks, MlirMemoryEffectInstancesList,
    MlirMemoryEffectsOpInterfaceCallbacks, MlirOperation, MlirSpeculatability,
    MlirSpeculatability_MlirSpeculatabilitySpeculatable,
    mlirConditionallySpeculatableOpInterfaceAttachFallbackModel,
    mlirMemoryEffectsOpInterfaceAttachFallbackModel,
};

use crate::{error::Error, string::string_ref};

/// Declares the operation `op_name` as `Pure`.
///
/// In ODS, `Pure` is shorthand for two traits: `NoMemoryEffect` (the operation does not read or write memory) and `AlwaysSpeculatable` (it is safe to execute it speculatively).
/// Generic transformations rely on this: the canonicalizer erases a pure operation whose results are unused, and CSE merges identical pure operations.
///
/// Here both are attached as *fallback models* of the corresponding interfaces: an implementation that MLIR stores for the operation and calls through C callbacks.
/// Call this at most once per operation.
pub fn attach_pure(context: &Context, op_name: &str) -> Result<(), Error> {
    if !context.is_registered_operation(op_name) {
        return Err(Error::UnregisteredOperation(op_name.into()));
    }

    let memory_effects = MlirMemoryEffectsOpInterfaceCallbacks {
        construct: None,
        destruct: None,
        getEffects: Some(no_memory_effects),
        userData: ptr::null_mut(),
    };
    let speculatability = MlirConditionallySpeculatableOpInterfaceCallbacks {
        construct: None,
        destruct: None,
        getSpeculatability: Some(always_speculatable),
        userData: ptr::null_mut(),
    };

    // SAFETY: The operation is registered, which both functions require. The
    // callbacks use no user data.
    unsafe {
        mlirMemoryEffectsOpInterfaceAttachFallbackModel(
            context.to_raw(),
            string_ref(op_name),
            memory_effects,
        );
        mlirConditionallySpeculatableOpInterfaceAttachFallbackModel(
            context.to_raw(),
            string_ref(op_name),
            speculatability,
        );
    }

    Ok(())
}

/// Reports an empty list of effects: the operation touches no memory.
unsafe extern "C" fn no_memory_effects(
    _: MlirOperation,
    _: MlirMemoryEffectInstancesList,
    _: *mut c_void,
) {
}

unsafe extern "C" fn always_speculatable(_: MlirOperation, _: *mut c_void) -> MlirSpeculatability {
    MlirSpeculatability_MlirSpeculatabilitySpeculatable
}
