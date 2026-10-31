//! Lowering to the LLVM dialect (chapter 6).
//!
//! After the partial lowering of chapter 5, `main` is a mix of the `affine`, `arith`, `memref`, and `func` dialects, plus `toy.print`.
//! To generate code, everything must be lowered to the LLVM dialect, which MLIR can translate into LLVM IR.
//!
//! MLIR already knows how to lower its own dialects.
//! They lower in several steps (`affine` to `scf` to `cf` to `llvm`), which is called *transitive lowering*.
//! The only thing left for us is `toy.print`, which we lower to a loop nest calling `printf` for each element:
//!
//! ```mlir
//! scf.for %i = %c0 to %c2 step %c1 {
//!   scf.for %j = %c0 to %c3 step %c1 {
//!     %element = memref.load %buffer[%i, %j] : memref<2x3xf64>
//!     llvm.call @printf(%format, %element) vararg(!llvm.func<i32 (ptr, ...)>) : ...
//!   }
//!   llvm.call @printf(%newline) vararg(!llvm.func<i32 (ptr, ...)>) : ...
//! }
//! ```
//!
//! In C++, the tutorial combines our pattern with the patterns of all the builtin lowerings (`populateAffineToStdConversionPatterns` and friends) in a single full conversion.
//! Those `populate` functions are not part of the C API, so we run our pattern first and then the equivalent builtin passes, which is the standard MLIR pipeline for lowering to LLVM.

use melior::{
    Context, RewriterBase,
    dialect::{
        arith, llvm, memref,
        ods::llvm::{AddressOfOperation, GlobalOperationBuilder},
        scf,
    },
    ir::{
        Attribute, Block, BlockLike, BlockRef, Identifier, Location, Operation, OperationRef,
        Region, RegionLike, Type, Value, ValueLike,
        attribute::{
            DenseI32ArrayAttribute, FlatSymbolRefAttribute, IntegerAttribute, StringAttribute,
            TypeAttribute,
        },
        operation::{OperationBuilder, OperationLike},
        r#type::{IntegerType, MemRefType, ShapedTypeLike, TypeId},
    },
    pass::{ExternalPass, Pass, PassManager, conversion, create_external},
};
use melior_ext::{
    ConversionPatternSet, ConversionTarget, Error as MeliorExtError, Failure, LogicalResult,
    TypeConverter, apply_partial_conversion, with_context,
};

use crate::dialect::{PrintOp, ToyOp, dimensions, is_operation};

/// The unique identifier of the pass: the address of this static.
static PASS_ID: u64 = 0;

/// Adds the passes that lower a module produced by the affine lowering of chapter 5 to the LLVM dialect.
pub fn add_lower_to_llvm_passes(pass_manager: &PassManager) {
    // The only remaining operation of the Toy dialect is `toy.print`.
    pass_manager.add_pass(create_lower_print_pass());

    // Then lower the builtin dialects. `affine` lowers to `scf` (structured
    // loops) and `arith`, and `scf` to `cf` (branches between blocks); these
    // and the other dialects then lower to `llvm`. Values whose type changes
    // between two of these steps are connected by temporary
    // `unrealized_conversion_cast` operations, which the last pass removes.
    pass_manager.add_pass(conversion::create_lower_affine());
    pass_manager.add_pass(conversion::create_scf_to_control_flow());
    pass_manager.add_pass(conversion::create_arith_to_llvm());
    pass_manager.add_pass(conversion::create_finalize_mem_ref_to_llvm());
    pass_manager.add_pass(conversion::create_control_flow_to_llvm());
    pass_manager.add_pass(conversion::create_func_to_llvm());
    pass_manager.add_pass(conversion::create_reconcile_unrealized_casts());
}

/// Creates a pass that lowers `toy.print` to calls to `printf`.
#[must_use]
pub fn create_lower_print_pass() -> Pass {
    create_external(
        |module: OperationRef<'_, '_>, pass: ExternalPass<'_>| {
            if with_context(module, lower_print_ops).is_err() {
                pass.signal_failure();
            }
        },
        TypeId::create(&PASS_ID),
        "ToyPrintToLLVMLowering",
        "toy-print-to-llvm",
        "Lower toy.print to calls to printf",
        "builtin.module",
        &[],
    )
}

fn lower_print_ops<'c>(
    context: &'c Context,
    module: OperationRef<'c, '_>,
) -> Result<(), MeliorExtError> {
    // `toy.print` is illegal, and the operations it lowers to are legal. (The
    // framework checks the legality of the operations a pattern creates.)
    let mut target = ConversionTarget::new(context);
    target.add_illegal_op(PrintOp::NAME);
    for dialect in ["arith", "llvm", "memref", "scf"] {
        target.add_legal_dialect(dialect);
    }

    let mut type_converter = TypeConverter::new();
    type_converter.add_conversion(|r#type| Some(r#type));

    let mut patterns = ConversionPatternSet::new(context);
    patterns.add(PrintOp::NAME, &type_converter, lower_print);

    apply_partial_conversion(module, &target, patterns)
}

/// Lowers `toy.print` to a loop nest calling `printf` on each element of the buffer, and on a newline after each row.
fn lower_print<'c>(
    context: &'c Context,
    operation: OperationRef<'c, '_>,
    operands: &[Value<'c, '_>],
    rewriter: RewriterBase<'c, '_>,
) -> LogicalResult {
    let location = operation.location();
    let [input] = *operands else {
        return Err(Failure);
    };
    let shape = dimensions_of(input)?;
    let module = parent_module(operation).ok_or(Failure)?;

    // Declare the printf function and the strings it prints at the start of
    // the module, unless an earlier print already did.
    let printf = get_or_insert_printf(rewriter, context, module);
    get_or_insert_global_string(rewriter, context, location, "frmt_spec", "%f \0", module);
    get_or_insert_global_string(rewriter, context, location, "nl", "\n\0", module);

    // Continue before the operation being lowered, starting with the addresses
    // of the strings.
    rewriter.set_insertion_point_before(operation);
    let format_specifier = address_of(&rewriter, context, location, "frmt_spec");
    let newline = address_of(&rewriter, context, location, "nl");

    // Create a loop for each of the dimensions within the shape.
    let index = Type::index(context);
    let mut induction_variables = Vec::new();
    for (dimension, &size) in shape.iter().enumerate() {
        let lower_bound = index_constant(&rewriter, context, location, 0);
        let upper_bound = index_constant(&rewriter, context, location, size);
        let step = index_constant(&rewriter, context, location, 1);

        // The body ends with a newline after each of the inner dimensions,
        // and the terminator.
        let block = Block::new(&[(index, location)]);
        if dimension != shape.len() - 1 {
            block.append_operation(call_printf(context, location, printf, &[newline]));
        }
        block.append_operation(scf::r#yield(&[], location));
        let region = Region::new();
        region.append_block(block);

        let r#loop = rewriter.insert(scf::r#for(lower_bound, upper_bound, step, region, location));

        // Continue at the start of the loop's body.
        let body = r#loop
            .region(0)
            .ok()
            .and_then(|region| region.first_block())
            .expect("the loop has a body");
        induction_variables.push(
            body.argument(0)
                .expect("the loop has an induction variable")
                .into(),
        );
        rewriter.set_insertion_point_to_start(body);
    }

    // Generate a call to printf for the current element of the loop.
    let element = rewriter
        .insert(memref::load(input, &induction_variables, location))
        .result(0)
        .expect("a load has a result")
        .into();
    rewriter.insert(call_printf(
        context,
        location,
        printf,
        &[format_specifier, element],
    ));

    // Notify the rewriter that this operation has been removed.
    rewriter.erase_op(operation);
    Ok(())
}

/// Returns the type of `printf`: `i32 (ptr, ...)`.
fn printf_type(context: &Context) -> Type<'_> {
    llvm::r#type::function(
        IntegerType::new(context, 32).into(),
        &[llvm::r#type::pointer(context, 0)],
        true,
    )
}

/// Returns a symbol reference to the `printf` function, inserting its declaration at the start of the module if necessary.
fn get_or_insert_printf<'c>(
    rewriter: RewriterBase<'c, '_>,
    context: &'c Context,
    module: OperationRef<'c, '_>,
) -> FlatSymbolRefAttribute<'c> {
    if lookup_symbol(module, "llvm.func", "printf").is_none() {
        rewriter.set_insertion_point_to_start(module_body(module));
        rewriter.insert(llvm::func(
            context,
            StringAttribute::new(context, "printf"),
            TypeAttribute::new(printf_type(context)),
            Region::new(),
            &[],
            module.location(),
        ));
    }

    FlatSymbolRefAttribute::new(context, "printf")
}

/// Declares the global string `name` with the contents `value` at the start of the module, unless it already exists.
fn get_or_insert_global_string<'c>(
    rewriter: RewriterBase<'c, '_>,
    context: &'c Context,
    location: Location<'c>,
    name: &str,
    value: &str,
    module: OperationRef<'c, '_>,
) {
    if lookup_symbol(module, "llvm.mlir.global", name).is_some() {
        return;
    }

    rewriter.set_insertion_point_to_start(module_body(module));
    let r#type = llvm::r#type::array(
        IntegerType::new(context, 8).into(),
        u32::try_from(value.len()).expect("the string is short"),
    );
    rewriter.insert(
        GlobalOperationBuilder::new(context, location)
            .initializer(Region::new())
            .global_type(TypeAttribute::new(r#type))
            .sym_name(StringAttribute::new(context, name))
            .linkage(Attribute::parse(context, "#llvm.linkage<internal>").expect("valid linkage"))
            .constant(Attribute::unit(context))
            .value(StringAttribute::new(context, value).into())
            .build()
            .into(),
    );
}

/// Returns a pointer to the global `name`.
///
/// With opaque pointers, the address of a string is also the address of its first character, so the `llvm.getelementptr` of the C++ tutorial is not needed.
fn address_of<'c, 'r>(
    rewriter: &'r RewriterBase<'c, '_>,
    context: &'c Context,
    location: Location<'c>,
    name: &str,
) -> Value<'c, 'r> {
    rewriter
        .insert(
            AddressOfOperation::builder(context, location)
                .res(llvm::r#type::pointer(context, 0))
                .global_name(FlatSymbolRefAttribute::new(context, name))
                .build()
                .into(),
        )
        .result(0)
        .expect("addressof has a result")
        .into()
}

/// Builds a call to `printf`.
///
/// The operation is built generically, which shows everything an `llvm.call` holds: the callee, the type of the variadic callee, and the sizes of its two operand groups (the arguments and the, here empty, operand bundles).
fn call_printf<'c>(
    context: &'c Context,
    location: Location<'c>,
    printf: FlatSymbolRefAttribute<'c>,
    arguments: &[Value<'c, '_>],
) -> Operation<'c> {
    OperationBuilder::new("llvm.call", location)
        .add_operands(arguments)
        .add_results(&[IntegerType::new(context, 32).into()])
        .add_attributes(&[
            (Identifier::new(context, "callee"), printf.into()),
            (
                Identifier::new(context, "var_callee_type"),
                TypeAttribute::new(printf_type(context)).into(),
            ),
            (
                Identifier::new(context, "operandSegmentSizes"),
                DenseI32ArrayAttribute::new(
                    context,
                    &[
                        i32::try_from(arguments.len()).expect("printf takes few arguments"),
                        0,
                    ],
                )
                .into(),
            ),
            (
                Identifier::new(context, "op_bundle_sizes"),
                DenseI32ArrayAttribute::new(context, &[]).into(),
            ),
        ])
        .build()
        .expect("valid llvm.call")
}

fn index_constant<'c, 'r>(
    rewriter: &'r RewriterBase<'c, '_>,
    context: &'c Context,
    location: Location<'c>,
    value: i64,
) -> Value<'c, 'r> {
    rewriter
        .insert(arith::constant(
            context,
            IntegerAttribute::new(Type::index(context), value).into(),
            location,
        ))
        .result(0)
        .expect("a constant has a result")
        .into()
}

/// Returns the shape of a buffer, which must be static.
fn dimensions_of(buffer: Value) -> Result<Vec<i64>, Failure> {
    let r#type = MemRefType::try_from(buffer.r#type()).map_err(|_| Failure)?;
    if !r#type.has_static_shape() {
        return Err(Failure);
    }

    Ok(dimensions(r#type)
        .into_iter()
        .map(|size| i64::try_from(size).expect("the shape is static"))
        .collect())
}

/// Returns the module containing `operation`.
fn parent_module<'c, 'a>(operation: OperationRef<'c, 'a>) -> Option<OperationRef<'c, 'a>> {
    let mut parent = operation.parent_operation();
    while let Some(operation) = parent {
        if is_operation(operation, "builtin.module") {
            return Some(operation);
        }
        parent = operation.parent_operation();
    }
    None
}

fn module_body<'c, 'a>(module: OperationRef<'c, 'a>) -> BlockRef<'c, 'a> {
    module
        .region(0)
        .ok()
        .and_then(|region| region.first_block())
        .expect("a module has a body")
}

/// Returns the operation named `operation_name` that defines the symbol `name` in the module, if any.
fn lookup_symbol<'c, 'a>(
    module: OperationRef<'c, 'a>,
    operation_name: &str,
    name: &str,
) -> Option<OperationRef<'c, 'a>> {
    let mut operation = module_body(module).first_operation();
    while let Some(current) = operation {
        let is_symbol = is_operation(current, operation_name)
            && current
                .attribute("sym_name")
                .ok()
                .and_then(|symbol| StringAttribute::try_from(symbol).ok())
                .is_some_and(|symbol| symbol.value() == name);
        if is_symbol {
            return Some(current);
        }
        operation = current.next_in_block();
    }
    None
}
