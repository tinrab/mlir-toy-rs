// The Toy dialect, defined in IRDL.
//
// In the C++ tutorial (chapter 2), the dialect is declared with ODS, the
// Operation Definition Specification, in `Ops.td`. TableGen turns it into C++
// classes that are compiled into the compiler. From Rust we cannot generate and
// compile C++ classes, but MLIR can also define dialects *at runtime*, from a
// description written in IRDL, the IR Definition Language. IRDL is itself an
// MLIR dialect: this file is ordinary MLIR, and `crate::dialect::load` hands it
// to `mlirLoadIRDLDialects`.
//
// IRDL describes the structure of each operation: its operands, results, and
// attributes, together with *constraints* on their types. Behavior that ODS
// expresses with traits, interfaces, or C++ code (verifiers, `Pure`, the
// `Terminator` trait, shape inference, canonicalization patterns) is attached
// from Rust afterwards; see `crate::dialect`.
//
// A note on constraint semantics: in IRDL, every SSA value below is a
// constraint *variable*. All uses of one variable within an operation must be
// satisfied by the *same* type or attribute. That is why each operand and
// result gets its own `irdl.any_of`, and why the shared `irdl.base` constraints
// are also redeclared for every operand: reusing them would require, say,
// both operands of `toy.add` to have exactly the same type.

irdl.dialect @toy {
  // A struct type (chapter 7), such as `!toy.struct<[tensor<*xf64>, f64]>`.
  // Its single parameter is the array of element types. (A C++ dialect can
  // define a custom syntax that drops the brackets; IRDL types print their
  // parameters generically.)
  irdl.type @struct {
    %element_types = irdl.base "#builtin.array"
    irdl.parameters(element_types: %element_types)
  }

  // %0 = toy.constant dense<[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]> : tensor<2x3xf64>
  //
  // Turns a literal into an SSA value. The data is attached to the operation as
  // the `value` attribute.
  irdl.operation @constant {
    %value = irdl.base "#builtin.dense_typed_elements"
    irdl.attributes {"value" = %value}

    %ranked = irdl.base "!builtin.tensor"
    %unranked = irdl.base "!builtin.unranked_tensor"
    %output = irdl.any_of(%ranked, %unranked)
    irdl.results(output: %output)
  }

  // %2 = toy.add %0, %1 : tensor<*xf64>
  //
  // Element-wise addition. The shapes of the operands are expected to match.
  irdl.operation @add {
    %lhs_ranked = irdl.base "!builtin.tensor"
    %lhs_unranked = irdl.base "!builtin.unranked_tensor"
    %lhs = irdl.any_of(%lhs_ranked, %lhs_unranked)
    %rhs_ranked = irdl.base "!builtin.tensor"
    %rhs_unranked = irdl.base "!builtin.unranked_tensor"
    %rhs = irdl.any_of(%rhs_ranked, %rhs_unranked)
    irdl.operands(lhs: %lhs, rhs: %rhs)

    %output_ranked = irdl.base "!builtin.tensor"
    %output_unranked = irdl.base "!builtin.unranked_tensor"
    %output = irdl.any_of(%output_ranked, %output_unranked)
    irdl.results(output: %output)
  }

  // %2 = toy.mul %0, %1 : tensor<*xf64>
  //
  // Element-wise multiplication. The shapes of the operands are expected to
  // match.
  irdl.operation @mul {
    %lhs_ranked = irdl.base "!builtin.tensor"
    %lhs_unranked = irdl.base "!builtin.unranked_tensor"
    %lhs = irdl.any_of(%lhs_ranked, %lhs_unranked)
    %rhs_ranked = irdl.base "!builtin.tensor"
    %rhs_unranked = irdl.base "!builtin.unranked_tensor"
    %rhs = irdl.any_of(%rhs_ranked, %rhs_unranked)
    irdl.operands(lhs: %lhs, rhs: %rhs)

    %output_ranked = irdl.base "!builtin.tensor"
    %output_unranked = irdl.base "!builtin.unranked_tensor"
    %output = irdl.any_of(%output_ranked, %output_unranked)
    irdl.results(output: %output)
  }

  // %1 = toy.cast %0 : tensor<2x3xf64> to tensor<*xf64>
  //
  // Converts a tensor to an equivalent type without changing any data: both
  // types must have the same element type and, if both are ranked, the same
  // shape (chapter 4).
  irdl.operation @cast {
    %input_ranked = irdl.base "!builtin.tensor"
    %input_unranked = irdl.base "!builtin.unranked_tensor"
    %input = irdl.any_of(%input_ranked, %input_unranked)
    irdl.operands(input: %input)

    %output_ranked = irdl.base "!builtin.tensor"
    %output_unranked = irdl.base "!builtin.unranked_tensor"
    %output = irdl.any_of(%output_ranked, %output_unranked)
    irdl.results(output: %output)
  }

  // %1 = toy.transpose(%0 : tensor<2x3xf64>) to tensor<3x2xf64>
  irdl.operation @transpose {
    %input_ranked = irdl.base "!builtin.tensor"
    %input_unranked = irdl.base "!builtin.unranked_tensor"
    %input = irdl.any_of(%input_ranked, %input_unranked)
    irdl.operands(input: %input)

    %output_ranked = irdl.base "!builtin.tensor"
    %output_unranked = irdl.base "!builtin.unranked_tensor"
    %output = irdl.any_of(%output_ranked, %output_unranked)
    irdl.results(output: %output)
  }

  // %1 = toy.reshape(%0 : tensor<6xf64>) to tensor<2x3xf64>
  //
  // Changes the shape of a tensor without changing the number of elements.
  // The result always has a static shape.
  irdl.operation @reshape {
    %input_ranked = irdl.base "!builtin.tensor"
    %input_unranked = irdl.base "!builtin.unranked_tensor"
    %input = irdl.any_of(%input_ranked, %input_unranked)
    irdl.operands(input: %input)

    %output = irdl.base "!builtin.tensor"
    irdl.results(output: %output)
  }

  // toy.print %0 : tensor<2x3xf64>
  //
  // Prints a tensor and produces no results. It also accepts a memref so that
  // it can stay in the IR after the tensors have been lowered to buffers
  // (chapter 5).
  irdl.operation @print {
    %ranked = irdl.base "!builtin.tensor"
    %unranked = irdl.base "!builtin.unranked_tensor"
    %memref = irdl.base "!builtin.memref"
    %input = irdl.any_of(%ranked, %unranked, %memref)
    irdl.operands(input: %input)
  }

  // toy.return %0 : tensor<*xf64>
  //
  // Returns from a function, with an optional value. This is the terminator of
  // Toy functions.
  irdl.operation @return {
    %ranked = irdl.base "!builtin.tensor"
    %unranked = irdl.base "!builtin.unranked_tensor"
    %struct = irdl.base @toy::@struct
    %input = irdl.any_of(%ranked, %unranked, %struct)
    irdl.operands(input: optional %input)
  }

  // %0 = toy.struct_constant [dense<...> : tensor<2x3xf64>, ...]
  //        : !toy.struct<[tensor<*xf64>, ...]>
  //
  // Turns a literal struct value into an SSA value. The value is an array of
  // the elements' constant values (chapter 7).
  irdl.operation @struct_constant {
    %value = irdl.base "#builtin.array"
    irdl.attributes {"value" = %value}

    %output = irdl.base @toy::@struct
    irdl.results(output: %output)
  }

  // %1 = toy.struct_access %0[1] : !toy.struct<[...]> -> tensor<*xf64>
  //
  // Accesses the element of a struct at a constant index (chapter 7).
  irdl.operation @struct_access {
    %index = irdl.base "#builtin.integer"
    irdl.attributes {"index" = %index}

    %input = irdl.base @toy::@struct
    irdl.operands(input: %input)

    %output_ranked = irdl.base "!builtin.tensor"
    %output_unranked = irdl.base "!builtin.unranked_tensor"
    %output_struct = irdl.base @toy::@struct
    %output = irdl.any_of(%output_ranked, %output_unranked, %output_struct)
    irdl.results(output: %output)
  }
}
