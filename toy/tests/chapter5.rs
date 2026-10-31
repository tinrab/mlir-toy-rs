//! Chapter 5: partial lowering to the affine dialect.

mod common;

use common::compile_example;
use toy::compiler::Action;

#[test]
fn test_lowers_to_affine_loops() {
    assert_eq!(
        compile_example("codegen.toy", Action::MlirAffine, false),
        EXPECTED_AFFINE
    );
}

#[test]
fn test_fuses_loops_when_optimizing() {
    // The transpose and the multiplication are fused into one loop nest, and
    // the intermediate buffer is gone.
    assert_eq!(
        compile_example("codegen.toy", Action::MlirAffine, true),
        EXPECTED_AFFINE_OPT
    );
}

const EXPECTED_AFFINE: &str = r#"module {
  func.func @main() {
    %cst = arith.constant 6.000000e+00 : f64
    %cst_0 = arith.constant 5.000000e+00 : f64
    %cst_1 = arith.constant 4.000000e+00 : f64
    %cst_2 = arith.constant 3.000000e+00 : f64
    %cst_3 = arith.constant 2.000000e+00 : f64
    %cst_4 = arith.constant 1.000000e+00 : f64
    %alloc = memref.alloc() : memref<3x2xf64>
    %alloc_5 = memref.alloc() : memref<3x2xf64>
    %alloc_6 = memref.alloc() : memref<2x3xf64>
    affine.store %cst_4, %alloc_6[0, 0] : memref<2x3xf64>
    affine.store %cst_3, %alloc_6[0, 1] : memref<2x3xf64>
    affine.store %cst_2, %alloc_6[0, 2] : memref<2x3xf64>
    affine.store %cst_1, %alloc_6[1, 0] : memref<2x3xf64>
    affine.store %cst_0, %alloc_6[1, 1] : memref<2x3xf64>
    affine.store %cst, %alloc_6[1, 2] : memref<2x3xf64>
    affine.for %arg0 = 0 to 3 {
      affine.for %arg1 = 0 to 2 {
        %0 = affine.load %alloc_6[%arg1, %arg0] : memref<2x3xf64>
        affine.store %0, %alloc_5[%arg0, %arg1] : memref<3x2xf64>
      }
    }
    affine.for %arg0 = 0 to 3 {
      affine.for %arg1 = 0 to 2 {
        %0 = affine.load %alloc_5[%arg0, %arg1] : memref<3x2xf64>
        %1 = arith.mulf %0, %0 : f64
        affine.store %1, %alloc[%arg0, %arg1] : memref<3x2xf64>
      }
    }
    "toy.print"(%alloc) : (memref<3x2xf64>) -> ()
    memref.dealloc %alloc_6 : memref<2x3xf64>
    memref.dealloc %alloc_5 : memref<3x2xf64>
    memref.dealloc %alloc : memref<3x2xf64>
    return
  }
}
"#;

const EXPECTED_AFFINE_OPT: &str = r#"module {
  func.func @main() {
    %cst = arith.constant 6.000000e+00 : f64
    %cst_0 = arith.constant 5.000000e+00 : f64
    %cst_1 = arith.constant 4.000000e+00 : f64
    %cst_2 = arith.constant 3.000000e+00 : f64
    %cst_3 = arith.constant 2.000000e+00 : f64
    %cst_4 = arith.constant 1.000000e+00 : f64
    %alloc = memref.alloc() : memref<3x2xf64>
    %alloc_5 = memref.alloc() : memref<2x3xf64>
    affine.store %cst_4, %alloc_5[0, 0] : memref<2x3xf64>
    affine.store %cst_3, %alloc_5[0, 1] : memref<2x3xf64>
    affine.store %cst_2, %alloc_5[0, 2] : memref<2x3xf64>
    affine.store %cst_1, %alloc_5[1, 0] : memref<2x3xf64>
    affine.store %cst_0, %alloc_5[1, 1] : memref<2x3xf64>
    affine.store %cst, %alloc_5[1, 2] : memref<2x3xf64>
    affine.for %arg0 = 0 to 3 {
      affine.for %arg1 = 0 to 2 {
        %0 = affine.load %alloc_5[%arg1, %arg0] : memref<2x3xf64>
        %1 = arith.mulf %0, %0 : f64
        affine.store %1, %alloc[%arg0, %arg1] : memref<3x2xf64>
      }
    }
    "toy.print"(%alloc) : (memref<3x2xf64>) -> ()
    memref.dealloc %alloc_5 : memref<2x3xf64>
    memref.dealloc %alloc : memref<3x2xf64>
    return
  }
}
"#;
