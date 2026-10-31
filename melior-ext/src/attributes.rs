use melior::ir::{AffineMap, Attribute};
use mlir_sys::mlirAffineMapAttrGet;

/// Returns the attribute wrapping an affine map, such as `affine_map<(d0, d1) -> (d0, d1)>`.
#[must_use]
pub fn affine_map_attribute(map: AffineMap) -> Attribute {
    // SAFETY: The map is valid; the attribute belongs to the map's context.
    unsafe { Attribute::from_raw(mlirAffineMapAttrGet(map.to_raw())) }
}
