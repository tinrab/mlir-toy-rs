//! The abstract syntax tree (AST) of the Toy language.
//!
//! Chapter 1 of the tutorial introduces Toy, a small tensor language:
//!
//! ```toy
//! # User defined generic function that operates on unknown shaped arguments.
//! def multiply_transpose(a, b) {
//!   return transpose(a) * transpose(b);
//! }
//!
//! def main() {
//!   # Define a variable `a` with shape <2, 3>, initialized with the literal
//!   # value. The shape is inferred from the supplied literal.
//!   var a = [[1, 2, 3], [4, 5, 6]];
//!   # b is identical to a, the literal tensor is implicitly reshaped:
//!   # defining new variables is the way to reshape tensors (element count
//!   # must match).
//!   var b<2, 3> = [1, 2, 3, 4, 5, 6];
//!   var c = multiply_transpose(a, b);
//!   print(c);
//! }
//! ```
//!
//! Toy values are tensors of 64-bit floats (up to rank 2 when printed), or structs of values (chapter 7).
//! Functions are generic: their parameters have no shape, and a shape is specialized for every call site later on.
//!
//! The parser in [`crate::parser`] produces this AST, and [`Module::dump`] prints it in the same format as the C++ tutorial (`toyc -emit=ast`).

use std::{
    fmt::{self, Display, Formatter, Write},
    sync::Arc,
};

/// A position in a source file, used for diagnostics and MLIR locations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub file: Arc<str>,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
}

impl Display for Location {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.file, self.line, self.column)
    }
}

/// The declared type of a variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VarType {
    /// A tensor with the given shape.
    /// An empty shape means that the shape is not specified and will be inferred.
    Tensor(Vec<i64>),
    /// A struct, referenced by name (chapter 7).
    Struct(String),
}

/// A binary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    /// Struct member access: `value.member` (chapter 7).
    Access,
}

impl BinaryOperator {
    /// Returns the operator as written in the source.
    #[must_use]
    pub fn symbol(self) -> char {
        match self {
            Self::Add => '+',
            Self::Subtract => '-',
            Self::Multiply => '*',
            Self::Access => '.',
        }
    }
}

/// An expression.
/// In Toy, statements are expressions too.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Number(Number),
    Literal(Literal),
    StructLiteral(StructLiteral),
    Variable(Variable),
    VarDecl(VarDecl),
    Return(Return),
    Binary(Binary),
    Call(Call),
    Print(Print),
}

impl Expr {
    #[must_use]
    pub fn location(&self) -> &Location {
        match self {
            Self::Number(number) => &number.location,
            Self::Literal(literal) => &literal.location,
            Self::StructLiteral(literal) => &literal.location,
            Self::Variable(variable) => &variable.location,
            Self::VarDecl(declaration) => &declaration.location,
            Self::Return(r#return) => &r#return.location,
            Self::Binary(binary) => &binary.location,
            Self::Call(call) => &call.location,
            Self::Print(print) => &print.location,
        }
    }
}

/// A numeric literal like `1.0`.
#[derive(Clone, Debug, PartialEq)]
pub struct Number {
    pub location: Location,
    pub value: f64,
}

/// A tensor literal like `[[1, 2], [3, 4]]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Literal {
    pub location: Location,
    /// The elements at this nesting level: numbers or nested literals.
    pub values: Vec<Expr>,
    /// The shape of the literal, including all nested levels.
    pub dims: Vec<i64>,
}

/// A struct literal like `{[1, 2], 3}` (chapter 7).
#[derive(Clone, Debug, PartialEq)]
pub struct StructLiteral {
    pub location: Location,
    pub values: Vec<Expr>,
}

/// A reference to a variable, like `a`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variable {
    pub location: Location,
    pub name: String,
}

/// A variable declaration, like `var a<2, 3> = ...`.
///
/// Also used for function parameters and struct members, which have no initializer.
#[derive(Clone, Debug, PartialEq)]
pub struct VarDecl {
    pub location: Location,
    pub name: String,
    pub var_type: VarType,
    pub init: Option<Box<Expr>>,
}

/// A `return` statement with an optional value.
#[derive(Clone, Debug, PartialEq)]
pub struct Return {
    pub location: Location,
    pub value: Option<Box<Expr>>,
}

/// A binary operation like `a * b`.
#[derive(Clone, Debug, PartialEq)]
pub struct Binary {
    pub location: Location,
    pub operator: BinaryOperator,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

/// A call to a user-defined function or to the builtin `transpose`.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub location: Location,
    pub callee: String,
    pub args: Vec<Expr>,
}

/// A call to the builtin `print`.
#[derive(Clone, Debug, PartialEq)]
pub struct Print {
    pub location: Location,
    pub arg: Box<Expr>,
}

/// The signature of a function: its name and parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct Prototype {
    pub location: Location,
    pub name: String,
    pub params: Vec<VarDecl>,
}

/// A function definition.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub prototype: Prototype,
    pub body: Vec<Expr>,
}

/// A struct definition (chapter 7).
#[derive(Clone, Debug, PartialEq)]
pub struct StructDef {
    pub location: Location,
    pub name: String,
    pub fields: Vec<VarDecl>,
}

/// A top-level definition.
#[derive(Clone, Debug, PartialEq)]
pub enum Record {
    Function(Function),
    Struct(StructDef),
}

/// A Toy source file: a list of function and struct definitions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    pub records: Vec<Record>,
}

impl Module {
    /// Prints the AST in the format of the C++ tutorial's `-emit=ast`.
    #[must_use]
    pub fn dump(&self) -> String {
        let mut dumper = Dumper::default();
        dumper.module(self);
        dumper.output
    }
}

/// Formats a number like C's `printf("%e")`, as LLVM's `raw_ostream` does for doubles: `1.000000e+00`.
pub(crate) fn format_scientific(value: f64) -> String {
    let formatted = format!("{value:.6e}");
    let Some((mantissa, exponent)) = formatted.split_once('e') else {
        // Infinity and NaN have no exponent.
        return formatted;
    };
    let exponent = exponent.parse::<i32>().expect("exponent is an integer");
    let sign = if exponent < 0 { '-' } else { '+' };

    format!("{mantissa}e{sign}{:02}", exponent.abs())
}

/// Prints the AST as an indented tree.
///
/// The format, including its few oddities (such as the location of a struct literal on its own line), follows the C++ `ASTDumper` exactly.
#[derive(Default)]
struct Dumper {
    output: String,
    indent: usize,
}

impl Dumper {
    /// Runs `dump` one indentation level deeper, after printing the indentation (the `INDENT()` macro of the C++ version).
    fn nested(&mut self, dump: impl FnOnce(&mut Self)) {
        self.indent += 1;
        self.write_indent();
        dump(self);
        self.indent -= 1;
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
    }

    fn write(&mut self, text: impl Display) {
        write!(self.output, "{text}").expect("writing to a string never fails");
    }

    fn module(&mut self, module: &Module) {
        self.nested(|this| {
            this.write("Module:\n");
            for record in &module.records {
                match record {
                    Record::Function(function) => this.function(function),
                    Record::Struct(r#struct) => this.struct_def(r#struct),
                }
            }
        });
    }

    fn struct_def(&mut self, r#struct: &StructDef) {
        self.nested(|this| {
            this.write(format_args!(
                "Struct: {} @{}\n",
                r#struct.name, r#struct.location
            ));
            this.nested(|this| {
                this.write("Variables: [\n");
                for field in &r#struct.fields {
                    this.var_decl(field);
                }
                this.write_indent();
                this.write("]\n");
            });
        });
    }

    fn function(&mut self, function: &Function) {
        self.nested(|this| {
            this.write("Function \n");
            this.prototype(&function.prototype);
            this.block(&function.body);
        });
    }

    fn prototype(&mut self, prototype: &Prototype) {
        self.nested(|this| {
            this.write(format_args!(
                "Proto '{}' @{}\n",
                prototype.name, prototype.location
            ));
            this.write_indent();
            let params = prototype
                .params
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>();
            this.write(format_args!("Params: [{}]\n", params.join(", ")));
        });
    }

    fn block(&mut self, body: &[Expr]) {
        self.nested(|this| {
            this.write("Block {\n");
            for expr in body {
                this.expr(expr);
            }
            this.write_indent();
            this.write("} // Block\n");
        });
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Number(number) => self.nested(|this| {
                this.write(format_args!(
                    "{} @{}\n",
                    format_scientific(number.value),
                    number.location
                ));
            }),
            Expr::Literal(literal) => self.nested(|this| {
                this.write("Literal: ");
                this.literal(expr);
                this.write(format_args!(" @{}\n", literal.location));
            }),
            Expr::StructLiteral(literal) => self.nested(|this| {
                this.write("Struct Literal: ");
                for value in &literal.values {
                    this.expr(value);
                }
                this.write_indent();
                this.write(format_args!(" @{}\n", literal.location));
            }),
            Expr::Variable(variable) => self.nested(|this| {
                this.write(format_args!(
                    "var: {} @{}\n",
                    variable.name, variable.location
                ));
            }),
            Expr::VarDecl(declaration) => self.var_decl(declaration),
            Expr::Return(r#return) => self.nested(|this| {
                this.write("Return\n");
                match &r#return.value {
                    Some(value) => this.expr(value),
                    None => this.nested(|this| this.write("(void)\n")),
                }
            }),
            Expr::Binary(binary) => self.nested(|this| {
                this.write(format_args!(
                    "BinOp: {} @{}\n",
                    binary.operator.symbol(),
                    binary.location
                ));
                this.expr(&binary.lhs);
                this.expr(&binary.rhs);
            }),
            Expr::Call(call) => self.nested(|this| {
                this.write(format_args!(
                    "Call '{}' [ @{}\n",
                    call.callee, call.location
                ));
                for arg in &call.args {
                    this.expr(arg);
                }
                this.write_indent();
                this.write("]\n");
            }),
            Expr::Print(print) => self.nested(|this| {
                this.write(format_args!("Print [ @{}\n", print.location));
                this.expr(&print.arg);
                this.write_indent();
                this.write("]\n");
            }),
        }
    }

    fn var_decl(&mut self, declaration: &VarDecl) {
        self.nested(|this| {
            this.write(format_args!("VarDecl {}", declaration.name));
            this.var_type(&declaration.var_type);
            this.write(format_args!(" @{}\n", declaration.location));
            if let Some(init) = &declaration.init {
                this.expr(init);
            }
        });
    }

    /// Prints a type: the struct name or the shape between `<` and `>`.
    fn var_type(&mut self, var_type: &VarType) {
        match var_type {
            VarType::Tensor(shape) => self.write(format_args!("<{}>", join(shape))),
            VarType::Struct(name) => self.write(format_args!("<{name}>")),
        }
    }

    /// Prints a literal with its dimensions spelled out at every level, so `[[1, 2], [3, 4]]` prints as `<2, 2>[ <2>[ 1, 2], <2>[ 3, 4]]`.
    fn literal(&mut self, expr: &Expr) {
        match expr {
            Expr::Number(number) => self.write(format_scientific(number.value)),
            Expr::Literal(literal) => {
                self.write(format_args!("<{}>[ ", join(&literal.dims)));
                for (index, value) in literal.values.iter().enumerate() {
                    if index > 0 {
                        self.write(", ");
                    }
                    self.literal(value);
                }
                self.write("]");
            }
            _ => unreachable!("literals only contain numbers and literals"),
        }
    }
}

fn join(values: &[i64]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formats_numbers_like_printf() {
        assert_eq!(format_scientific(1.0), "1.000000e+00");
        assert_eq!(format_scientific(5.5), "5.500000e+00");
        assert_eq!(format_scientific(0.0), "0.000000e+00");
        assert_eq!(format_scientific(123_456.0), "1.234560e+05");
        assert_eq!(format_scientific(0.001), "1.000000e-03");
        assert_eq!(format_scientific(-2.5), "-2.500000e+00");
    }
}
