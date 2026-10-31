//! Emission of Toy dialect MLIR from the AST (chapter 2).
//!
//! This walks the AST and emits operations of the Toy dialect, preserving the semantics of the language, so that later passes can perform accurate analyses and transformations based on these high-level semantics.
//!
//! For example, this Toy function:
//!
//! ```toy
//! def multiply_transpose(a, b) {
//!   return transpose(a) * transpose(b);
//! }
//! ```
//!
//! becomes:
//!
//! ```mlir
//! func.func private @multiply_transpose(%arg0: tensor<*xf64>, %arg1: tensor<*xf64>) -> tensor<*xf64> {
//!   %0 = "toy.transpose"(%arg0) : (tensor<*xf64>) -> tensor<*xf64>
//!   %1 = "toy.transpose"(%arg1) : (tensor<*xf64>) -> tensor<*xf64>
//!   %2 = "toy.mul"(%0, %1) : (tensor<*xf64>, tensor<*xf64>) -> tensor<*xf64>
//!   "toy.return"(%2) : (tensor<*xf64>) -> ()
//! }
//! ```
//!
//! Two differences from the C++ tutorial, both due to defining the dialect in IRDL (see [`crate::dialect`]):
//!
//! - Functions are the builtin `func.func` rather than a custom `toy.func`.
//!   The C++ `toy.func` exists to show how to implement the `FunctionOpInterface`, which cannot be attached to an operation from the C API.
//! - Calls are the builtin `func.call` rather than `toy.generic_call`.
//!   IRDL requires all the values of a variadic operand list to have the same type, so it cannot describe `toy.generic_call`, whose arguments can have different shapes.
//!   `func.call` requires the arguments to match the callee signature exactly, so each argument whose shape is more precise than the (generic) parameter is converted with a `toy.cast`.
//!   These are the same casts the C++ inliner creates in chapter 4.

use std::{collections::HashMap, result};

use melior::{
    Context,
    dialect::func,
    ir::{
        Block, BlockLike, Identifier, Location, Module, Operation, Region, RegionLike, Type, Value,
        ValueLike,
        attribute::{
            ArrayAttribute, Attribute, DenseElementsAttribute, FlatSymbolRefAttribute,
            StringAttribute, TypeAttribute,
        },
        operation::OperationLike,
        r#type::{FunctionType, RankedTensorType},
    },
};

use crate::{
    ast::{
        self, Binary, BinaryOperator, Call, Expr, Function, Literal, Prototype, Record, StructDef,
        StructLiteral, VarDecl, VarType,
    },
    dialect::{
        AddOp, CastOp, ConstantOp, MulOp, PrintOp, ReshapeOp, ReturnOp, StructAccessOp,
        StructConstantOp, StructType, ToyOp, TransposeOp, unranked_f64_tensor,
    },
};

/// An error found while emitting MLIR.
#[derive(Debug, thiserror::Error)]
pub enum MlirGenError {
    /// The program is invalid, for example because it uses an unknown variable.
    #[error("{location}: {message}")]
    Semantic {
        location: ast::Location,
        message: String,
    },

    /// The emitted module failed verification.
    /// The details are reported through MLIR's diagnostics.
    #[error("module verification error")]
    Verification,
}

type Result<T> = result::Result<T, MlirGenError>;

/// Emits the MLIR module of a Toy program.
pub fn mlir_gen<'c>(context: &'c Context, module: &ast::Module) -> Result<Module<'c>> {
    let mut generator = ModuleGenerator {
        context,
        module: Module::new(Location::unknown(context)),
        functions: HashMap::new(),
        structs: HashMap::new(),
    };

    for record in &module.records {
        match record {
            Record::Function(function) => generator.function(function)?,
            Record::Struct(r#struct) => generator.struct_def(r#struct)?,
        }
    }

    // Verify the module after we have finished constructing it. This checks
    // the structural properties of the IR and invokes the verifiers of the
    // Toy operations.
    if !generator.module.as_operation().verify() {
        return Err(MlirGenError::Verification);
    }

    Ok(generator.module)
}

/// Emits the functions and structs of a module.
struct ModuleGenerator<'c, 'ast> {
    context: &'c Context,
    /// The module being built.
    /// Functions are appended to its body as they are emitted.
    module: Module<'c>,
    /// The signatures of the functions emitted so far, by name.
    /// A function can only call functions defined before it.
    functions: HashMap<String, FunctionType<'c>>,
    /// The structs defined so far, by name, with their MLIR type.
    structs: HashMap<&'ast str, (StructType<'c>, &'ast StructDef)>,
}

impl<'c, 'ast> ModuleGenerator<'c, 'ast> {
    /// Converts a Toy location into an MLIR location.
    fn location(&self, location: &ast::Location) -> Location<'c> {
        Location::new(
            self.context,
            &location.file,
            location.line as usize,
            location.column as usize,
        )
    }

    /// Records the MLIR type of a struct definition.
    fn struct_def(&mut self, r#struct: &'ast StructDef) -> Result<()> {
        if self.structs.contains_key(r#struct.name.as_str()) {
            return Err(error(
                &r#struct.location,
                format!("struct type with name '{}' already exists", r#struct.name),
            ));
        }

        let mut element_types = Vec::new();
        for field in &r#struct.fields {
            if field.init.is_some() {
                return Err(error(
                    &field.location,
                    "variables within a struct definition must not have initializers",
                ));
            }
            if matches!(&field.var_type, VarType::Tensor(shape) if !shape.is_empty()) {
                return Err(error(
                    &field.location,
                    "variables within a struct definition must not have a shape",
                ));
            }
            element_types.push(self.var_type(&field.var_type, &field.location)?);
        }

        let r#type = StructType::new(self.context, &element_types);
        self.structs.insert(&r#struct.name, (r#type, r#struct));
        Ok(())
    }

    /// Emits a function and appends it to the module.
    fn function(&mut self, function: &'ast Function) -> Result<()> {
        let Prototype {
            location,
            name,
            params,
        } = &function.prototype;

        // The parameters are generic: their shape is unknown and will be
        // specialized for each call later. Struct parameters have their struct
        // type.
        let param_types = params
            .iter()
            .map(|param| self.var_type(&param.var_type, &param.location))
            .collect::<Result<Vec<_>>>()?;
        let block = Block::new(
            &param_types
                .iter()
                .zip(params)
                .map(|(&r#type, param)| (r#type, self.location(&param.location)))
                .collect::<Vec<_>>(),
        );

        // Emit the body. Each function has a new scope of variables, starting
        // with the parameters.
        let result_types = {
            let mut body = FunctionGenerator {
                module: self,
                block: &block,
                variables: HashMap::new(),
            };
            for (index, param) in params.iter().enumerate() {
                let argument = block.argument(index).expect("one argument per parameter");
                body.declare(param, argument.into())?;
            }
            body.block_body(&function.body)?;

            // Return implicitly if the body has no return statement. Otherwise,
            // if the return statement has a value, it gives the function's
            // result type.
            block.terminator().and_then(ReturnOp::cast).map_or_else(
                || {
                    block.append_operation(ReturnOp::build(self.location(location), None));
                    Vec::new()
                },
                |r#return| {
                    r#return
                        .input()
                        .map(|value| value.r#type())
                        .into_iter()
                        .collect()
                },
            )
        };

        let function_type = FunctionType::new(self.context, &param_types, &result_types);
        let region = Region::new();
        region.append_block(block);

        // Functions other than `main` are private, so that they can be removed
        // once they have been inlined.
        let attributes = if name == "main" {
            Vec::new()
        } else {
            vec![(
                Identifier::new(self.context, "sym_visibility"),
                StringAttribute::new(self.context, "private").into(),
            )]
        };

        self.module.body().append_operation(func::func(
            self.context,
            StringAttribute::new(self.context, name),
            TypeAttribute::new(function_type.into()),
            region,
            &attributes,
            self.location(location),
        ));
        self.functions.insert(name.clone(), function_type);
        Ok(())
    }

    /// Returns the MLIR type of a declared variable type.
    fn var_type(&self, var_type: &VarType, location: &ast::Location) -> Result<Type<'c>> {
        match var_type {
            VarType::Tensor(shape) => Ok(self.tensor_type(shape)),
            VarType::Struct(name) => self
                .structs
                .get(name.as_str())
                .map(|&(r#type, _)| r#type.into())
                .ok_or_else(|| error(location, format!("unknown struct type '{name}'"))),
        }
    }

    /// Returns the tensor type with the given shape, or an unranked tensor type if the shape is empty (unknown).
    fn tensor_type(&self, shape: &[i64]) -> Type<'c> {
        if shape.is_empty() {
            unranked_f64_tensor(self.context)
        } else {
            let shape = shape.iter().map(|&size| size as u64).collect::<Vec<_>>();
            RankedTensorType::new(&shape, Type::float64(self.context), None).into()
        }
    }
}

/// A variable in scope: its value and its declaration.
#[derive(Clone, Copy)]
struct Variable<'c, 'b, 'ast> {
    value: Value<'c, 'b>,
    declaration: &'ast VarDecl,
}

/// Emits the body of a function into its entry block.
struct FunctionGenerator<'g, 'c, 'ast, 'b> {
    module: &'g ModuleGenerator<'c, 'ast>,
    /// The block of the function.
    /// Operations are appended at its end: this is the "insertion point" of the C++ `OpBuilder`.
    block: &'b Block<'c>,
    /// The symbol table: the variables in scope, by name.
    /// Toy only has one scope per function, so a single map suffices.
    variables: HashMap<&'ast str, Variable<'c, 'b, 'ast>>,
}

impl<'c, 'ast, 'b> FunctionGenerator<'_, 'c, 'ast, 'b> {
    fn context(&self) -> &'c Context {
        self.module.context
    }

    fn location(&self, location: &ast::Location) -> Location<'c> {
        self.module.location(location)
    }

    /// Declares a variable in the current scope.
    fn declare(&mut self, declaration: &'ast VarDecl, value: Value<'c, 'b>) -> Result<()> {
        if self.variables.contains_key(declaration.name.as_str()) {
            return Err(error(
                &declaration.location,
                format!("variable '{}' is already declared", declaration.name),
            ));
        }

        self.variables
            .insert(&declaration.name, Variable { value, declaration });
        Ok(())
    }

    /// Appends `operation` to the block and returns its only result.
    fn append_with_result(&self, operation: Operation<'c>) -> Value<'c, 'b> {
        self.block
            .append_operation(operation)
            .result(0)
            .expect("the operation has a result")
            .into()
    }

    /// Emits a list of statements.
    fn block_body(&mut self, body: &'ast [Expr]) -> Result<()> {
        for expr in body {
            // Declarations, returns, and prints can only appear as statements,
            // not nested in expressions.
            match expr {
                Expr::VarDecl(declaration) => {
                    self.var_decl(declaration)?;
                }
                Expr::Return(r#return) => {
                    let value = match &r#return.value {
                        Some(value) => Some(self.expr(value)?),
                        None => None,
                    };
                    self.block.append_operation(ReturnOp::build(
                        self.location(&r#return.location),
                        value,
                    ));
                    // Anything after a return statement is unreachable.
                    return Ok(());
                }
                Expr::Print(print) => {
                    let value = self.expr(&print.arg)?;
                    self.block
                        .append_operation(PrintOp::build(self.location(&print.location), value));
                }
                // A call used as a statement may return nothing.
                Expr::Call(call) => {
                    self.call(call)?;
                }
                _ => {
                    self.expr(expr)?;
                }
            }
        }

        Ok(())
    }

    /// Emits a variable declaration: emits its initializer and records the value in the symbol table, so that later expressions can refer to it.
    fn var_decl(&mut self, declaration: &'ast VarDecl) -> Result<Value<'c, 'b>> {
        let init = declaration.init.as_ref().ok_or_else(|| {
            error(
                &declaration.location,
                "missing initializer in variable declaration",
            )
        })?;
        let mut value = self.expr(init)?;

        match &declaration.var_type {
            // A struct variable must be initialized with a value of its type.
            VarType::Struct(_) => {
                let r#type = self
                    .module
                    .var_type(&declaration.var_type, &declaration.location)?;
                if r#type != value.r#type() {
                    return Err(error(
                        &declaration.location,
                        format!(
                            "struct type of initializer is different than the variable \
                             declaration: got {}, but expected {type}",
                            value.r#type()
                        ),
                    ));
                }
            }
            // A tensor variable declared with a specific shape reshapes its
            // initializer. The reshape is optimized away later when possible.
            VarType::Tensor(shape) if !shape.is_empty() => {
                value = self.append_with_result(ReshapeOp::build(
                    self.location(&declaration.location),
                    value,
                    self.module.tensor_type(shape),
                ));
            }
            VarType::Tensor(_) => {}
        }

        self.declare(declaration, value)?;
        Ok(value)
    }

    /// Emits an expression and returns its value.
    fn expr(&mut self, expr: &'ast Expr) -> Result<Value<'c, 'b>> {
        match expr {
            Expr::Number(number) => Ok(self.append_with_result(ConstantOp::build_scalar(
                self.context(),
                self.location(&number.location),
                number.value,
            ))),
            Expr::Literal(literal) => Ok(self.literal(literal)),
            Expr::StructLiteral(literal) => Ok(self.struct_literal(literal)),
            Expr::Variable(variable) => self
                .variables
                .get(variable.name.as_str())
                .map(|variable| variable.value)
                .ok_or_else(|| {
                    error(
                        &variable.location,
                        format!("unknown variable '{}'", variable.name),
                    )
                }),
            Expr::Binary(binary) => self.binary(binary),
            Expr::Call(call) => self.call(call)?.ok_or_else(|| {
                error(
                    &call.location,
                    format!("function '{}' does not return a value", call.callee),
                )
            }),
            Expr::VarDecl(declaration) => self.var_decl(declaration),
            Expr::Return(r#return) => Err(error(
                &r#return.location,
                "return statements cannot be used as expressions",
            )),
            Expr::Print(print) => Err(error(
                &print.location,
                "print statements cannot be used as expressions",
            )),
        }
    }

    /// Emits a tensor literal as a `toy.constant`.
    /// The data is flattened into a dense elements attribute: an attribute is how MLIR attaches constant data to an operation.
    ///
    /// For example, `var a<2, 3> = [[1, 2, 3], [4, 5, 6]];` becomes:
    ///
    /// ```mlir
    /// %0 = "toy.constant"() {value = dense<[[1.000000e+00, 2.000000e+00, 3.000000e+00],
    ///   [4.000000e+00, 5.000000e+00, 6.000000e+00]]> : tensor<2x3xf64>}
    ///   : () -> tensor<2x3xf64>
    /// ```
    fn literal(&self, literal: &Literal) -> Value<'c, 'b> {
        self.append_with_result(ConstantOp::build(
            self.context(),
            self.location(&literal.location),
            self.literal_attribute(literal),
        ))
    }

    /// Returns the data of a tensor literal as a dense elements attribute.
    fn literal_attribute(&self, literal: &Literal) -> DenseElementsAttribute<'c> {
        let mut data = Vec::new();
        collect_data(&Expr::Literal(literal.clone()), &mut data);

        DenseElementsAttribute::f64_values(self.module.tensor_type(&literal.dims), &data)
    }

    /// Returns the data of a number as a dense elements attribute of rank 0.
    fn number_attribute(&self, value: f64) -> DenseElementsAttribute<'c> {
        let r#type = RankedTensorType::new(&[], Type::float64(self.context()), None);
        DenseElementsAttribute::f64_values(r#type.into(), &[value])
    }

    /// Emits a struct literal as a `toy.struct_constant`, whose value is an array of the elements' constant values.
    fn struct_literal(&self, literal: &StructLiteral) -> Value<'c, 'b> {
        let (value, r#type) = self.struct_attribute(literal);
        self.append_with_result(StructConstantOp::build(
            self.context(),
            self.location(&literal.location),
            r#type,
            value,
        ))
    }

    /// Returns the constant value of a struct literal, with its struct type.
    ///
    /// Tensor elements have the generic type `tensor<*xf64>`, like the members of a declared struct.
    fn struct_attribute(&self, literal: &StructLiteral) -> (ArrayAttribute<'c>, StructType<'c>) {
        let (values, types): (Vec<Attribute>, Vec<Type>) = literal
            .values
            .iter()
            .map(|value| match value {
                Expr::Number(number) => (
                    Attribute::from(self.number_attribute(number.value)),
                    unranked_f64_tensor(self.context()),
                ),
                Expr::Literal(literal) => (
                    Attribute::from(self.literal_attribute(literal)),
                    unranked_f64_tensor(self.context()),
                ),
                Expr::StructLiteral(literal) => {
                    let (value, r#type) = self.struct_attribute(literal);
                    (Attribute::from(value), Type::from(r#type))
                }
                _ => unreachable!("the parser only creates literals in struct literals"),
            })
            .unzip();

        (
            ArrayAttribute::new(self.context(), &values),
            StructType::new(self.context(), &types),
        )
    }

    /// Emits a binary operation.
    fn binary(&mut self, binary: &'ast Binary) -> Result<Value<'c, 'b>> {
        // First emit the operations for each side of the operation before
        // emitting the operation itself. For example, if the expression is
        // `a + foo(a)`, the left-hand side is the value of `a`, already in the
        // symbol table, and the right-hand side emits a call to `foo`.
        let lhs = self.expr(&binary.lhs)?;
        let location = self.location(&binary.location);

        // A member access needs the index of the member instead of a value for
        // the right-hand side.
        if binary.operator == BinaryOperator::Access {
            let index = self
                .member_index(binary)
                .ok_or_else(|| error(&binary.location, "invalid access into struct expression"))?;
            return Ok(self.append_with_result(StructAccessOp::build(
                self.context(),
                location,
                lhs,
                index,
            )));
        }

        let rhs = self.expr(&binary.rhs)?;
        let operation = match binary.operator {
            BinaryOperator::Add => AddOp::build(self.context(), location, lhs, rhs),
            BinaryOperator::Multiply => MulOp::build(self.context(), location, lhs, rhs),
            operator => {
                return Err(error(
                    &binary.location,
                    format!("invalid binary operator '{}'", operator.symbol()),
                ));
            }
        };

        Ok(self.append_with_result(operation))
    }

    /// Returns the definition of the struct that `expr` evaluates to, if it can be determined: `expr` must be a struct variable or a member access that yields a struct.
    fn struct_for(&self, expr: &Expr) -> Option<&'ast StructDef> {
        let struct_name = match expr {
            Expr::Variable(variable) => {
                let declaration = self.variables.get(variable.name.as_str())?.declaration;
                &declaration.var_type
            }
            Expr::Binary(Binary {
                operator: BinaryOperator::Access,
                lhs,
                rhs,
                ..
            }) => {
                // The name of the member is the right-hand side.
                let Expr::Variable(member) = rhs.as_ref() else {
                    return None;
                };
                let parent = self.struct_for(lhs)?;
                let field = parent
                    .fields
                    .iter()
                    .find(|field| field.name == member.name)?;
                &field.var_type
            }
            _ => return None,
        };

        let VarType::Struct(name) = struct_name else {
            return None;
        };
        self.module
            .structs
            .get(name.as_str())
            .map(|&(_, definition)| definition)
    }

    /// Returns the index of the member accessed by `access`.
    fn member_index(&self, access: &Binary) -> Option<usize> {
        let definition = self.struct_for(&access.lhs)?;
        let Expr::Variable(member) = access.rhs.as_ref() else {
            return None;
        };

        definition
            .fields
            .iter()
            .position(|field| field.name == member.name)
    }

    /// Emits a call.
    /// `transpose` is a builtin with its own operation; other names are user-defined functions.
    /// Returns the call's result, if any.
    fn call(&mut self, call: &'ast Call) -> Result<Option<Value<'c, 'b>>> {
        let location = self.location(&call.location);

        // Emit the arguments first.
        let mut args = Vec::new();
        for arg in &call.args {
            args.push(self.expr(arg)?);
        }

        if call.callee == "transpose" {
            let [arg] = args[..] else {
                return Err(error(
                    &call.location,
                    "toy.transpose does not accept multiple arguments",
                ));
            };
            return Ok(Some(self.append_with_result(TransposeOp::build(
                self.context(),
                location,
                arg,
            ))));
        }

        // Otherwise, this is a call to a user-defined function.
        let function_type = *self.module.functions.get(&call.callee).ok_or_else(|| {
            error(
                &call.location,
                format!("no defined function found for '{}'", call.callee),
            )
        })?;
        if args.len() != function_type.input_count() {
            return Err(error(
                &call.location,
                format!(
                    "function '{}' expects {} arguments, but {} were given",
                    call.callee,
                    function_type.input_count(),
                    args.len()
                ),
            ));
        }

        // Convert the arguments to the generic parameter types.
        for (index, arg) in args.iter_mut().enumerate() {
            let param_type = function_type.input(index).expect("index is in bounds");
            if arg.r#type() != param_type {
                *arg = self.append_with_result(CastOp::build(location, *arg, param_type));
            }
        }

        let result_types = (0..function_type.result_count())
            .map(|index| function_type.result(index).expect("index is in bounds"))
            .collect::<Vec<_>>();
        let operation = self.block.append_operation(func::call(
            self.context(),
            FlatSymbolRefAttribute::new(self.context(), &call.callee),
            &args,
            &result_types,
            location,
        ));

        Ok(operation.result(0).ok().map(Value::from))
    }
}

/// Collects the numbers of a nested tensor literal in a flat list, so that `[[1, 2], [3, 4]]` gives `[1, 2, 3, 4]`.
fn collect_data(expr: &Expr, data: &mut Vec<f64>) {
    match expr {
        Expr::Literal(literal) => {
            for value in &literal.values {
                collect_data(value, data);
            }
        }
        Expr::Number(number) => data.push(number.value),
        _ => unreachable!("tensor literals only contain numbers and literals"),
    }
}

fn error(location: &ast::Location, message: impl Into<String>) -> MlirGenError {
    MlirGenError::Semantic {
        location: location.clone(),
        message: message.into(),
    }
}
