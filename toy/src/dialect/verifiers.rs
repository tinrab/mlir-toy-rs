//! Verifiers of the Toy operations.
//!
//! ODS generates verification code for the constraints in `Ops.td` (such as "operand #0 must be a tensor of 64-bit floats"), and an operation with `hasVerifier = 1` adds custom C++ checks in its `verify()` method.
//! IRDL already checks the coarse structure of our operations: the number of operands and results, and their kinds of types.
//! These functions perform the remaining checks, and `crate::dialect::load` attaches them to the operations.
//!
//! MLIR runs these verifiers *before* the IRDL constraints.
//! So when an operation does not have the expected structure, a verifier returns `Ok` and leaves the error to the IRDL constraints, which run next.

use melior::ir::{
    Attribute, OperationRef, Type, TypeLike, ValueLike,
    attribute::{ArrayAttribute, AttributeLike, IntegerAttribute, TypeAttribute},
    operation::OperationLike,
    r#type::{FunctionType, RankedTensorType, ShapedTypeLike},
};
use melior_ext::{VerificationError, element_type};

use crate::dialect::types::{StructType, dimensions};

type VerifyResult = Result<(), VerificationError>;

/// `toy.constant`: the value must be a dense attribute of `f64` values that matches the result type.
pub fn verify_constant(operation: OperationRef) -> VerifyResult {
    let (Some(output), Ok(value)) = (single_result_type(operation), operation.attribute("value"))
    else {
        return Ok(());
    };

    check_f64_tensor("result #0", output)?;
    verify_constant_for_type(output, value)
}

/// `toy.struct_constant`: the value must hold a valid constant for every element of the struct (chapter 7).
pub fn verify_struct_constant(operation: OperationRef) -> VerifyResult {
    let (Some(output), Ok(value)) = (single_result_type(operation), operation.attribute("value"))
    else {
        return Ok(());
    };

    verify_constant_for_type(output, value)
}

/// Checks that `value` is a valid constant for a value of type `type`.
fn verify_constant_for_type(r#type: Type, value: Attribute) -> VerifyResult {
    if r#type.is_tensor() {
        // The value must be a dense attribute of floats.
        if !value.is_dense_fp_elements() {
            return Err(VerificationError::from(format!(
                "constant of tensor type must be initialized by a dense float elements \
                 attribute, got {value}"
            )));
        }

        // If the result type is ranked, the shape must match the shape of the
        // attribute holding the data.
        let (Ok(result_type), Ok(value_type)) = (
            RankedTensorType::try_from(r#type),
            RankedTensorType::try_from(value.r#type()),
        ) else {
            return Ok(());
        };

        if value_type.rank() != result_type.rank() {
            return Err(VerificationError::from(format!(
                "return type must match the one of the attached value attribute: {} != {}",
                value_type.rank(),
                result_type.rank()
            )));
        }
        for (dimension, (value_size, result_size)) in dimensions(value_type)
            .into_iter()
            .zip(dimensions(result_type))
            .enumerate()
        {
            if value_size != result_size {
                return Err(VerificationError::from(format!(
                    "return type shape mismatches its attribute at dimension {dimension}: \
                     {value_size} != {result_size}"
                )));
            }
        }

        return Ok(());
    }

    let Some(struct_type) = StructType::from_type(r#type) else {
        return Err(VerificationError::from(format!(
            "constant must have a tensor or struct type, got {type}"
        )));
    };

    // A struct constant is an array with one value per element.
    let element_types = struct_type.element_types();
    let values = ArrayAttribute::try_from(value)
        .ok()
        .filter(|values| values.len() == element_types.len())
        .ok_or_else(|| {
            format!(
                "constant of struct type must be initialized by an array attribute with the \
                 same number of elements, got {value}"
            )
        })?;

    for (index, element_type) in element_types.into_iter().enumerate() {
        let element = values.element(index).expect("index is in bounds");
        verify_constant_for_type(element_type, element)?;
    }

    Ok(())
}

/// `toy.add` and `toy.mul`: both operands and the result are `f64` tensors.
pub fn verify_binary(operation: OperationRef) -> VerifyResult {
    let (Ok(lhs), Ok(rhs), Some(output)) = (
        operation.operand(0),
        operation.operand(1),
        single_result_type(operation),
    ) else {
        return Ok(());
    };

    check_f64_tensor("operand #0", lhs.r#type())?;
    check_f64_tensor("operand #1", rhs.r#type())?;
    check_f64_tensor("result #0", output)
}

/// `toy.cast`: the types must be compatible, which is what the C++ version's `CastOpInterface::areCastCompatible` checks.
pub fn verify_cast(operation: OperationRef) -> VerifyResult {
    let (Some(input), Some(output)) = (
        single_operand_type(operation),
        single_result_type(operation),
    ) else {
        return Ok(());
    };

    check_f64_tensor("operand #0", input)?;
    check_f64_tensor("result #0", output)?;

    if are_cast_compatible(input, output) {
        Ok(())
    } else {
        Err(VerificationError::from(format!(
            "operand type '{input}' and result type '{output}' are cast incompatible"
        )))
    }
}

/// Returns whether a `toy.cast` can convert `input` to `output`: both must be tensors with the same element type and, if both are ranked, the same shape.
pub fn are_cast_compatible(input: Type, output: Type) -> bool {
    if !input.is_tensor() || !output.is_tensor() || element_type(input) != element_type(output) {
        return false;
    }

    // The shape is required to match only if both types are ranked.
    !input.is_ranked_tensor() || !output.is_ranked_tensor() || input == output
}

/// `toy.transpose`: when both shapes are known, the result's shape is the input's shape reversed.
pub fn verify_transpose(operation: OperationRef) -> VerifyResult {
    let (Some(input), Some(output)) = (
        single_operand_type(operation),
        single_result_type(operation),
    ) else {
        return Ok(());
    };

    check_f64_tensor("operand #0", input)?;
    check_f64_tensor("result #0", output)?;

    let (Ok(input), Ok(output)) = (
        RankedTensorType::try_from(input),
        RankedTensorType::try_from(output),
    ) else {
        return Ok(());
    };

    if dimensions(input)
        .into_iter()
        .eq(dimensions(output).into_iter().rev())
    {
        Ok(())
    } else {
        Err("expected result shape to be a transpose of the input".into())
    }
}

/// `toy.reshape`: the result is a statically shaped `f64` tensor with as many elements as the input.
///
/// A reshape of a single element fills the whole result with it, which is how `var a<2, 2> = 5.5;` works.
pub fn verify_reshape(operation: OperationRef) -> VerifyResult {
    let (Some(input), Some(output)) = (
        single_operand_type(operation),
        single_result_type(operation),
    ) else {
        return Ok(());
    };

    check_f64_tensor("operand #0", input)?;

    let Some(output) = RankedTensorType::try_from(output)
        .ok()
        .filter(|output| output.has_static_shape() && output.element().is_f64())
    else {
        return Err(VerificationError::from(format!(
            "result #0 must be statically shaped tensor of 64-bit float values, but got \
             '{output}'"
        )));
    };

    let Ok(input) = RankedTensorType::try_from(input) else {
        return Ok(());
    };
    if !input.has_static_shape() {
        return Ok(());
    }

    let input_size = dimensions(input).into_iter().product::<u64>();
    let output_size = dimensions(output).into_iter().product::<u64>();
    if input_size == output_size || input_size == 1 {
        Ok(())
    } else {
        Err(VerificationError::from(format!(
            "cannot reshape a tensor of {input_size} elements into a tensor of {output_size} \
             elements"
        )))
    }
}

/// `toy.print`: the input is an `f64` tensor or, after the lowering of chapter 5, an `f64` memref.
pub fn verify_print(operation: OperationRef) -> VerifyResult {
    let Some(input) = single_operand_type(operation) else {
        return Ok(());
    };

    let is_f64 = element_type(input).is_some_and(|element| element.is_f64());
    if (input.is_tensor() || input.is_mem_ref()) && is_f64 {
        Ok(())
    } else {
        Err(VerificationError::from(format!(
            "operand #0 must be tensor of 64-bit float values or memref of 64-bit float \
             values, but got '{input}'"
        )))
    }
}

/// `toy.return`: it must be in a function, and the returned value must match the function's result type.
pub fn verify_return(operation: OperationRef) -> VerifyResult {
    // We know the parent is a function thanks to the `HasParent<"FuncOp">`
    // trait in C++; we check it here.
    let function_type = operation
        .parent_operation()
        .filter(|parent| parent.name().as_string_ref().as_str() == Ok("func.func"))
        .and_then(|function| function.attribute("function_type").ok())
        .and_then(|r#type| TypeAttribute::try_from(r#type).ok())
        .and_then(|r#type| FunctionType::try_from(r#type.value()).ok())
        .ok_or("expects parent op 'func.func'")?;

    // The operand count must match the function's result count.
    let operands = operation.operands().collect::<Vec<_>>();
    if operands.len() > 1 {
        return Err("expects at most 1 return operand".into());
    }
    if operands.len() != function_type.result_count() {
        return Err(VerificationError::from(format!(
            "does not return the same number of values ({}) as the enclosing function ({})",
            operands.len(),
            function_type.result_count()
        )));
    }

    // If there is no value, we are done.
    let Some(input) = operands.first() else {
        return Ok(());
    };
    let input = input.r#type();
    check_toy_type("operand #0", input)?;

    // The value's type must match the function's result type, unless one of
    // them is an unranked tensor.
    let result = function_type
        .result(0)
        .expect("the function has one result");
    if input == result || input.is_unranked_tensor() || result.is_unranked_tensor() {
        Ok(())
    } else {
        Err(VerificationError::from(format!(
            "type of return operand ({input}) doesn't match function result type ({result})"
        )))
    }
}

/// `toy.struct_access`: the index is within the struct, and the result type is the type of that element (chapter 7).
pub fn verify_struct_access(operation: OperationRef) -> VerifyResult {
    let (Some(input), Some(output), Ok(index)) = (
        single_operand_type(operation),
        single_result_type(operation),
        operation.attribute("index"),
    ) else {
        return Ok(());
    };
    let (Some(input), Ok(index)) = (
        StructType::from_type(input),
        IntegerAttribute::try_from(index),
    ) else {
        return Ok(());
    };

    let element_types = input.element_types();
    let Some(&element_type) = usize::try_from(index.value())
        .ok()
        .and_then(|index| element_types.get(index))
    else {
        return Err("index should be within the range of the input struct type".into());
    };

    if output == element_type {
        Ok(())
    } else {
        Err("must have the same result type as the struct element referred to by the index".into())
    }
}

/// Checks the ODS constraint `F64Tensor`: a ranked or unranked tensor of 64-bit floats.
fn check_f64_tensor(name: &str, r#type: Type) -> VerifyResult {
    if r#type.is_tensor() && element_type(r#type).is_some_and(|element| element.is_f64()) {
        Ok(())
    } else {
        Err(VerificationError::from(format!(
            "{name} must be tensor of 64-bit float values, but got '{type}'"
        )))
    }
}

/// Checks the ODS constraint `Toy_Type`: an `f64` tensor or a Toy struct.
fn check_toy_type(name: &str, r#type: Type) -> VerifyResult {
    if StructType::from_type(r#type).is_some() {
        Ok(())
    } else {
        check_f64_tensor(name, r#type)
    }
}

fn single_operand_type<'c>(operation: OperationRef<'c, '_>) -> Option<Type<'c>> {
    (operation.operand_count() == 1).then(|| {
        operation
            .operand(0)
            .expect("the operation has an operand")
            .r#type()
    })
}

fn single_result_type<'c>(operation: OperationRef<'c, '_>) -> Option<Type<'c>> {
    (operation.result_count() == 1).then(|| {
        operation
            .result(0)
            .expect("the operation has a result")
            .r#type()
    })
}
