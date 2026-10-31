use std::ffi::{CStr, c_char};

use melior::ir::Module;
use mlir_sys::{LLVMContextRef, LLVMModuleRef, mlirTranslateModuleToLLVMIR};

use crate::error::Error;

// Functions of the LLVM C API. They are part of the LLVM libraries that
// `mlir-sys` already links, but it does not generate bindings for them.
unsafe extern "C" {
    fn LLVMContextCreate() -> LLVMContextRef;

    fn LLVMContextDispose(context: LLVMContextRef);

    fn LLVMDisposeModule(module: LLVMModuleRef);

    fn LLVMPrintModuleToString(module: LLVMModuleRef) -> *mut c_char;

    fn LLVMDisposeMessage(message: *mut c_char);
}

/// Translates a module in the LLVM dialect into textual LLVM IR.
///
/// The translations must have been registered with the context first (see `melior::utility::register_all_llvm_translations`).
/// Fails if the module contains operations that cannot be translated.
pub fn translate_to_llvm_ir(module: &Module) -> Result<String, Error> {
    // SAFETY: The LLVM context is created and disposed here, and the module is
    // only printed if the translation succeeded. The printed string is copied
    // before it is disposed.
    unsafe {
        let context = LLVMContextCreate();
        let llvm_module = mlirTranslateModuleToLLVMIR(module.as_operation().to_raw(), context);

        let result = if llvm_module.is_null() {
            Err(Error::LlvmTranslation)
        } else {
            let message = LLVMPrintModuleToString(llvm_module);
            let ir = CStr::from_ptr(message).to_string_lossy().into_owned();
            LLVMDisposeMessage(message);
            LLVMDisposeModule(llvm_module);
            Ok(ir)
        };

        LLVMContextDispose(context);
        result
    }
}
