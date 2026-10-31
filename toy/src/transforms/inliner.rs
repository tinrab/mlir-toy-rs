//! Function inlining (chapter 4).
//!
//! Toy functions are generic: `multiply_transpose(a, b)` works on tensors of any shape.
//! Rather than specializing a copy of each function for each set of argument shapes, the tutorial takes the simple route: it inlines every call into `main`, so that shape inference only has to handle one function.
//!
//! In C++, this uses MLIR's generic inliner pass.
//! The inliner works with any dialect through interfaces: the call implements `CallOpInterface`, the function `CallableOpInterface`, and the Toy dialect provides a `DialectInlinerInterface` that declares Toy operations legal to inline and handles the `toy.return` terminator.
//! Dialect interfaces cannot be attached through the C API, so the generic inliner refuses to inline Toy operations.
//! Instead, this module implements the (simple) inlining Toy needs:
//!
//! 1. Order the functions so that every function comes after the functions it calls.
//!    A recursive function cannot be inlined, as inlining it would never end, so it is an error.
//! 2. In that order, replace each `func.call` with a copy of the callee's body.
//!    Callees are processed first, so their bodies no longer contain calls.
//!
//! Afterwards, the private functions are unused, and MLIR's `symbol-dce` pass deletes them.

use std::collections::HashMap;

use melior::{
    Context, IrRewriter, RewriterBase,
    ir::{
        BlockLike, OperationRef, RegionLike, Value, ValueLike,
        attribute::{FlatSymbolRefAttribute, StringAttribute},
        operation::{OperationLike, WalkOrder},
        r#type::TypeId,
    },
    pass::{ExternalPass, Pass, create_external},
};
use melior_ext::{Failure, LogicalResult, emit_error, nested_operations, with_context};

use crate::dialect::{CastOp, ReturnOp, ToyOp, is_operation};

/// The unique identifier of the pass: the address of this static.
static PASS_ID: u64 = 0;

/// Creates a pass that inlines all function calls of a module.
#[must_use]
pub fn create_inliner_pass() -> Pass {
    create_external(
        |module: OperationRef<'_, '_>, pass: ExternalPass<'_>| {
            if with_context(module, inline_calls).is_err() {
                pass.signal_failure();
            }
        },
        TypeId::create(&PASS_ID),
        "ToyInliner",
        "toy-inline",
        "Inline all calls to Toy functions",
        "builtin.module",
        &[],
    )
}

/// Inlines all calls in `module`.
/// Errors are reported as diagnostics.
fn inline_calls<'c>(context: &'c Context, module: OperationRef<'c, '_>) -> LogicalResult {
    let rewriter = IrRewriter::new(context);
    let rewriter = rewriter.as_rewriter_base();

    // Find the functions of the module, by name.
    let body = module
        .region(0)
        .ok()
        .and_then(|region| region.first_block())
        .ok_or(Failure)?;
    let mut functions = HashMap::new();
    let mut operation = body.first_operation();
    while let Some(function) = operation {
        if is_operation(function, "func.func")
            && let Some(name) = function_name(function)
        {
            functions.insert(name, function);
        }
        operation = function.next_in_block();
    }

    for function in callees_first(&functions)? {
        for call in calls(function) {
            let callee = callee_name(call)
                .and_then(|name| functions.get(&name))
                .copied();
            let Some(callee) = callee else {
                emit_error(call.location(), "call to an unknown function");
                return Err(Failure);
            };
            inline_call(rewriter, call, callee);
        }
    }

    Ok(())
}

/// Returns the functions ordered so that every function comes after the functions it calls (a post-order of the call graph).
fn callees_first<'c, 'a>(
    functions: &HashMap<String, OperationRef<'c, 'a>>,
) -> Result<Vec<OperationRef<'c, 'a>>, Failure> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Visiting,
        Done,
    }

    fn visit<'c, 'a>(
        name: &str,
        functions: &HashMap<String, OperationRef<'c, 'a>>,
        states: &mut HashMap<String, State>,
        order: &mut Vec<OperationRef<'c, 'a>>,
    ) -> LogicalResult {
        let Some(&function) = functions.get(name) else {
            // Calls to unknown functions are reported when inlining.
            return Ok(());
        };
        match states.get(name) {
            Some(State::Done) => return Ok(()),
            Some(State::Visiting) => {
                emit_error(
                    function.location(),
                    &format!("cannot inline the recursive function '{name}'"),
                );
                return Err(Failure);
            }
            None => {}
        }

        states.insert(name.into(), State::Visiting);
        for call in calls(function) {
            if let Some(callee) = callee_name(call) {
                visit(&callee, functions, states, order)?;
            }
        }
        states.insert(name.into(), State::Done);
        order.push(function);
        Ok(())
    }

    // Visit the functions in a deterministic order.
    let mut names = functions.keys().collect::<Vec<_>>();
    names.sort();

    let mut states = HashMap::new();
    let mut order = Vec::new();
    for name in names {
        visit(name, functions, &mut states, &mut order)?;
    }

    Ok(order)
}

/// Replaces `call` with the body of `callee`.
///
/// The callee is cloned first, because other calls may still need it.
/// Then the operations of the clone's body are moved right before the call, with the callee's parameters replaced by the call's arguments.
/// Finally, the callee's `toy.return`, which moved along with the body, gives the values that replace the call's results.
fn inline_call(rewriter: RewriterBase, call: OperationRef, callee: OperationRef) {
    let callee = (*callee).clone();
    let body = callee
        .region(0)
        .ok()
        .and_then(|region| region.first_block())
        .expect("a function has a body");

    // The terminator of the callee gives the returned values. It moves along
    // with the rest of the body.
    let r#return = body
        .terminator()
        .and_then(ReturnOp::cast)
        .expect("a Toy function ends with toy.return");

    let arguments = call.operands().collect::<Vec<_>>();
    rewriter.inline_block_before(body, call, &arguments);

    // Replace the results of the call with the returned values. If a type
    // differs, a cast converts the value to the type the call promised: this
    // is the `materializeCallConversion` hook of the C++ inliner interface.
    rewriter.set_insertion_point_before(call);
    let results = r#return
        .input()
        .into_iter()
        .zip(call.results())
        .map(|(value, result)| -> Value {
            if value.r#type() == result.r#type() {
                value
            } else {
                rewriter
                    .insert(CastOp::build(call.location(), value, result.r#type()))
                    .result(0)
                    .expect("a cast has a result")
                    .into()
            }
        })
        .collect::<Vec<_>>();

    rewriter.replace_op_with_values(call, &results);
    rewriter.erase_op(r#return.operation());
}

/// Returns the `func.call` operations in `function`.
fn calls<'c, 'a>(function: OperationRef<'c, 'a>) -> Vec<OperationRef<'c, 'a>> {
    nested_operations(function, WalkOrder::PreOrder)
        .into_iter()
        .filter(|&operation| is_operation(operation, "func.call"))
        .collect()
}

fn function_name(function: OperationRef) -> Option<String> {
    let name = StringAttribute::try_from(function.attribute("sym_name").ok()?).ok()?;
    Some(name.value().to_owned())
}

fn callee_name(call: OperationRef) -> Option<String> {
    let callee = FlatSymbolRefAttribute::try_from(call.attribute("callee").ok()?).ok()?;
    Some(callee.value().to_owned())
}
