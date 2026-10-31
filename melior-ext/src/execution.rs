use std::{ffi::c_void, mem, ptr};

use melior::ir::{
    BlockLike, Module, TypeLike,
    attribute::{StringAttribute, TypeAttribute},
    operation::OperationLike,
};
use mlir_sys::{
    mlirExecutionEngineCreate, mlirExecutionEngineDestroy, mlirExecutionEngineLookupPacked,
    mlirLLVMFunctionTypeGetNumInputs, mlirLLVMFunctionTypeGetReturnType,
    mlirLLVMFunctionTypeIsVarArg, mlirLLVMVoidTypeGet, mlirTypeEqual, mlirTypeGetContext,
};

use crate::{error::Error, string::string_ref};

/// JIT-compiles a module in the LLVM dialect and calls its `main` function, which must take no arguments and return nothing.
///
/// The JIT resolves external functions, such as `printf`, in the current process.
/// `optimization_level` (0 to 3) selects the LLVM optimizations.
/// The translations to LLVM IR must have been registered with the context.
pub fn run_main(module: &Module, optimization_level: u32) -> Result<(), Error> {
    // Calling a function through `invokePacked` with the wrong number of
    // arguments is undefined behavior, so check the signature first.
    if !has_void_main(module) {
        return Err(Error::InvalidMain);
    }

    // SAFETY: The module is valid, and no shared libraries are passed. The
    // engine is checked before use and destroyed after the call. The packed
    // wrapper of a function takes an array of pointers to its arguments and
    // results; `main` has neither, so the array is empty.
    unsafe {
        let engine = mlirExecutionEngineCreate(
            module.to_raw(),
            optimization_level.min(3).cast_signed(),
            0,
            ptr::null(),
            false,
            false,
        );
        if engine.ptr.is_null() {
            return Err(Error::JitCompilation);
        }

        // The engine wraps every function `f` into `_mlir_f(void **arguments)`,
        // which unpacks the arguments and calls `f`. This is what
        // `ExecutionEngine::invokePacked("main")` calls in C++.
        let wrapper = mlirExecutionEngineLookupPacked(engine, string_ref("main"));
        let result = if wrapper.is_null() {
            Err(Error::JitInvocation)
        } else {
            let wrapper =
                mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut *mut c_void)>(wrapper);
            let mut arguments: [*mut c_void; 0] = [];
            wrapper(arguments.as_mut_ptr());
            Ok(())
        };

        mlirExecutionEngineDestroy(engine);
        result
    }
}

/// Returns whether the module contains `llvm.func @main()` returning `void`.
fn has_void_main(module: &Module) -> bool {
    let mut operation = module.body().first_operation();

    while let Some(function) = operation {
        operation = function.next_in_block();

        let is_main = function.name().as_string_ref().as_str() == Ok("llvm.func")
            && function
                .attribute("sym_name")
                .ok()
                .and_then(|name| StringAttribute::try_from(name).ok())
                .is_some_and(|name| name.value() == "main");
        if !is_main {
            continue;
        }

        let Some(r#type) = function
            .attribute("function_type")
            .ok()
            .and_then(|r#type| TypeAttribute::try_from(r#type).ok())
            .map(|r#type| r#type.value())
        else {
            return false;
        };

        let r#type = r#type.to_raw();
        // SAFETY: The type of an `llvm.func` is an LLVM function type.
        return unsafe {
            let void = mlirLLVMVoidTypeGet(mlirTypeGetContext(r#type));
            mlirLLVMFunctionTypeGetNumInputs(r#type) == 0
                && !mlirLLVMFunctionTypeIsVarArg(r#type)
                && mlirTypeEqual(mlirLLVMFunctionTypeGetReturnType(r#type), void)
        };
    }

    false
}
