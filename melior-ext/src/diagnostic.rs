use std::ffi::CString;

use melior::ir::Location;
use mlir_sys::mlirEmitError;

/// Emits an error diagnostic at `location` through the context's diagnostic engine.
///
/// This corresponds to `mlir::emitError(location) << message` in C++.
/// The diagnostic goes to the handlers attached to the context or, if there are none, is printed to standard error.
pub fn emit_error(location: Location, message: &str) {
    // The C API takes a NUL-terminated string, so interior NULs are replaced.
    let message = CString::new(message.replace('\0', " ")).expect("NUL bytes were replaced");

    // SAFETY: `location` is a valid location and `message` is a valid
    // NUL-terminated string that outlives the call.
    unsafe { mlirEmitError(location.to_raw(), message.as_ptr()) }
}
