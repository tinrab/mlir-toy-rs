use std::slice;

use mlir_sys::MlirStringRef;

/// Borrows a Rust string as an `MlirStringRef`.
///
/// The returned value points into `string` and must not outlive it.
/// MLIR string references carry an explicit length and need no terminating NUL.
pub fn string_ref(string: &str) -> MlirStringRef {
    MlirStringRef {
        data: string.as_ptr().cast(),
        length: string.len(),
    }
}

/// Copies an `MlirStringRef` into an owned Rust string.
///
/// # Safety
///
/// `string` must point to `length` readable bytes.
pub unsafe fn to_string(string: MlirStringRef) -> String {
    if string.data.is_null() {
        return String::new();
    }

    // SAFETY: The caller guarantees that `data` points to `length` bytes.
    let bytes = unsafe { slice::from_raw_parts(string.data.cast::<u8>(), string.length) };
    String::from_utf8_lossy(bytes).into_owned()
}
