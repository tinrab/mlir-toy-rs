//! The parser of the Toy language (chapter 1).
//!
//! This is a simple recursive descent parser that produces a well-formed AST from the tokens of the [`Lexer`].
//! It performs no semantic checks or symbol resolution: for example, code can reference an undeclared variable and still parse successfully.
//! Those checks happen when emitting MLIR.

use std::result;

use crate::{
    ast::{
        Binary, BinaryOperator, Call, Expr, Function, Literal, Location, Module, Number, Print,
        Prototype, Record, Return, StructDef, StructLiteral, VarDecl, VarType, Variable,
    },
    lexer::{Lexer, Token},
};

/// A syntax error.
#[derive(Debug, thiserror::Error)]
#[error("{location}: parse error: expected {expected} {context} but found {found}")]
pub struct ParseError {
    pub location: Location,
    pub expected: &'static str,
    pub context: &'static str,
    pub found: Token,
}

/// Parses a Toy source file.
/// `file` is only used for locations.
pub fn parse(source: &str, file: &str) -> Result<Module> {
    Parser {
        lexer: Lexer::new(source, file),
    }
    .module()
}

type Result<T> = result::Result<T, ParseError>;

struct Parser<'a> {
    lexer: Lexer<'a>,
}

impl Parser<'_> {
    /// ```text
    /// module ::= (definition | struct)*
    /// ```
    fn module(&mut self) -> Result<Module> {
        let mut records = Vec::new();

        loop {
            let record = match self.lexer.current() {
                Token::Eof => break,
                Token::Def => Record::Function(self.definition()?),
                Token::Struct => Record::Struct(self.struct_def()?),
                _ => {
                    return self
                        .error("'def' or 'struct'", "when parsing top level module records");
                }
            };
            records.push(record);
        }

        Ok(Module { records })
    }

    /// ```text
    /// definition ::= prototype block
    /// ```
    fn definition(&mut self) -> Result<Function> {
        Ok(Function {
            prototype: self.prototype()?,
            body: self.block()?,
        })
    }

    /// ```text
    /// prototype ::= 'def' identifier '(' param_list? ')'
    /// param_list ::= param | param ',' param_list
    /// param ::= identifier | identifier identifier
    /// ```
    ///
    /// A parameter is either just a name, or a struct type name followed by the parameter name.
    fn prototype(&mut self) -> Result<Prototype> {
        let location = self.lexer.location();
        if *self.lexer.current() != Token::Def {
            return self.error("'def'", "in prototype");
        }
        self.lexer.advance();

        let Some(name) = self.identifier() else {
            return self.error("function name", "in prototype");
        };
        self.expect_char('(', "in prototype")?;

        let mut params = Vec::new();
        if !self.is_char(')') {
            loop {
                let location = self.lexer.location();
                let Some(name_or_type) = self.identifier() else {
                    return self.error("identifier", "in function parameter list");
                };

                // If the next token is an identifier too, we just parsed the type.
                let param = match self.identifier() {
                    Some(name) => VarDecl {
                        location,
                        name,
                        var_type: VarType::Struct(name_or_type),
                        init: None,
                    },
                    None => VarDecl {
                        location,
                        name: name_or_type,
                        var_type: VarType::Tensor(Vec::new()),
                        init: None,
                    },
                };
                params.push(param);

                if !self.is_char(',') {
                    break;
                }
                self.lexer.advance();
                if !matches!(self.lexer.current(), Token::Identifier(_)) {
                    return self.error("identifier", "after ',' in function parameter list");
                }
            }
        }
        self.expect_char(')', "to end function prototype")?;

        Ok(Prototype {
            location,
            name,
            params,
        })
    }

    /// ```text
    /// struct ::= 'struct' identifier '{' (declaration ';')+ '}'
    /// ```
    fn struct_def(&mut self) -> Result<StructDef> {
        let location = self.lexer.location();
        self.lexer.advance();

        let Some(name) = self.identifier() else {
            return self.error("name", "in struct definition");
        };
        self.expect_char('{', "in struct definition")?;

        let mut fields = Vec::new();
        loop {
            fields.push(self.declaration(false)?);
            self.expect_char(';', "after variable in struct definition")?;
            if self.is_char('}') {
                break;
            }
        }
        self.lexer.advance();

        Ok(StructDef {
            location,
            name,
            fields,
        })
    }

    /// ```text
    /// block ::= '{' expression_list '}'
    /// expression_list ::= block_expr ';' expression_list
    /// block_expr ::= declaration | 'return' | expression
    /// ```
    fn block(&mut self) -> Result<Vec<Expr>> {
        self.expect_char('{', "to begin block")?;
        // Ignore empty expressions: swallow sequences of semicolons.
        self.skip_semicolons();

        let mut body = Vec::new();
        while !self.is_char('}') && *self.lexer.current() != Token::Eof {
            let expr = match self.lexer.current() {
                // Variable declaration of a struct type, or a call.
                Token::Identifier(_) => self.declaration_or_call()?,
                Token::Var => Expr::VarDecl(self.declaration(true)?),
                Token::Return => Expr::Return(self.r#return()?),
                _ => self.expression()?,
            };
            body.push(expr);

            // Ensure that elements are separated by a semicolon.
            if !self.is_char(';') {
                return self.error("';'", "after expression");
            }
            self.skip_semicolons();
        }
        self.expect_char('}', "to close block")?;

        Ok(body)
    }

    /// Parses a variable declaration, either of a tensor or of a struct.
    ///
    /// ```text
    /// declaration ::= 'var' identifier type? ('=' expression)?
    ///               | identifier identifier ('=' expression)?
    /// ```
    fn declaration(&mut self, requires_initializer: bool) -> Result<VarDecl> {
        if *self.lexer.current() == Token::Var {
            return self.var_declaration(requires_initializer);
        }

        let location = self.lexer.location();
        let Some(type_name) = self.identifier() else {
            return self.error("type name", "in variable declaration");
        };
        self.typed_declaration(type_name, requires_initializer, location)
    }

    /// Parses either a struct variable declaration or a call expression, which both start with an identifier.
    fn declaration_or_call(&mut self) -> Result<Expr> {
        let location = self.lexer.location();
        let name = self
            .identifier()
            .expect("the current token is an identifier");

        if self.is_char('(') {
            self.call(name, location)
        } else {
            Ok(Expr::VarDecl(self.typed_declaration(name, true, location)?))
        }
    }

    /// Parses the rest of a struct variable declaration, after its type name.
    fn typed_declaration(
        &mut self,
        type_name: String,
        requires_initializer: bool,
        location: Location,
    ) -> Result<VarDecl> {
        let Some(name) = self.identifier() else {
            return self.error("name", "in variable declaration");
        };

        let init = if requires_initializer {
            if !self.is_char('=') {
                return self.error("initializer", "in variable declaration");
            }
            self.lexer.advance();
            Some(Box::new(self.expression()?))
        } else {
            None
        };

        Ok(VarDecl {
            location,
            name,
            var_type: VarType::Struct(type_name),
            init,
        })
    }

    /// Parses a declaration that starts with `var`, followed by a name and an optional shape.
    ///
    /// ```text
    /// var_declaration ::= 'var' identifier type? ('=' expression)?
    /// ```
    fn var_declaration(&mut self, requires_initializer: bool) -> Result<VarDecl> {
        let location = self.lexer.location();
        self.lexer.advance();

        let Some(name) = self.identifier() else {
            return self.error("identifier", "after 'var' declaration");
        };

        // The type is optional; if it is missing, it will be inferred.
        let shape = if self.is_char('<') {
            self.shape()?
        } else {
            Vec::new()
        };

        let init = if requires_initializer {
            self.expect_char('=', "in variable declaration")?;
            Some(Box::new(self.expression()?))
        } else {
            None
        };

        Ok(VarDecl {
            location,
            name,
            var_type: VarType::Tensor(shape),
            init,
        })
    }

    /// ```text
    /// type ::= '<' shape_list '>'
    /// shape_list ::= number | number ',' shape_list
    /// ```
    fn shape(&mut self) -> Result<Vec<i64>> {
        self.expect_char('<', "to begin type")?;

        let mut shape = Vec::new();
        while let Token::Number(dimension) = *self.lexer.current() {
            // Dimensions are integers; the lexer reads all numbers as floats.
            if dimension.fract() != 0.0 || dimension > i64::MAX as f64 {
                return self.error("integer dimension", "in type");
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the dimension is an integer within range"
            )]
            let dimension = dimension as i64;
            shape.push(dimension);
            self.lexer.advance();
            if self.is_char(',') {
                self.lexer.advance();
            }
        }
        self.expect_char('>', "to end type")?;

        Ok(shape)
    }

    /// ```text
    /// return ::= 'return' ';' | 'return' expression ';'
    /// ```
    fn r#return(&mut self) -> Result<Return> {
        let location = self.lexer.location();
        self.lexer.advance();

        // `return` takes an optional value.
        let value = if self.is_char(';') {
            None
        } else {
            Some(Box::new(self.expression()?))
        };

        Ok(Return { location, value })
    }

    /// ```text
    /// expression ::= primary binop_rhs
    /// ```
    fn expression(&mut self) -> Result<Expr> {
        let lhs = self.primary()?;
        self.binary_rhs(0, lhs)
    }

    /// Parses the right-hand side of a binary expression, given its left-hand side.
    /// `precedence` is the minimal operator precedence to consume: this is operator-precedence parsing.
    ///
    /// ```text
    /// binop_rhs ::= (operator primary)*
    /// ```
    fn binary_rhs(&mut self, precedence: i32, mut lhs: Expr) -> Result<Expr> {
        loop {
            // If this is an operator that binds at least as tightly as the
            // current one, consume it, otherwise we are done.
            let Some((operator, operator_precedence)) = self.operator() else {
                return Ok(lhs);
            };
            if operator_precedence < precedence {
                return Ok(lhs);
            }
            self.lexer.advance();

            // Like the C++ version, the location of the operation is taken
            // *after* consuming the operator, so it points to the right-hand
            // side.
            let location = self.lexer.location();
            let mut rhs = self.primary()?;

            // If the operator binds less tightly with the right-hand side than
            // the operator after it, let the pending operator take the
            // right-hand side as its left-hand side.
            if self
                .operator()
                .is_some_and(|(_, next_precedence)| operator_precedence < next_precedence)
            {
                rhs = self.binary_rhs(operator_precedence + 1, rhs)?;
            }

            lhs = Expr::Binary(Binary {
                location,
                operator,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            });
        }
    }

    /// Returns the binary operator at the current token, with its precedence.
    fn operator(&self) -> Option<(BinaryOperator, i32)> {
        match self.lexer.current() {
            Token::Char('+') => Some((BinaryOperator::Add, 20)),
            Token::Char('-') => Some((BinaryOperator::Subtract, 20)),
            Token::Char('*') => Some((BinaryOperator::Multiply, 40)),
            Token::Char('.') => Some((BinaryOperator::Access, 60)),
            _ => None,
        }
    }

    /// ```text
    /// primary ::= identifier_expr | number | '(' expression ')'
    ///           | tensor_literal | struct_literal
    /// ```
    fn primary(&mut self) -> Result<Expr> {
        match self.lexer.current() {
            Token::Identifier(_) => self.identifier_expr(),
            Token::Number(_) => Ok(Expr::Number(self.number())),
            Token::Char('(') => self.parenthesized(),
            Token::Char('[') => Ok(Expr::Literal(self.tensor_literal()?)),
            Token::Char('{') => Ok(Expr::StructLiteral(self.struct_literal()?)),
            _ => self.error("expression", ""),
        }
    }

    /// ```text
    /// identifier_expr ::= identifier | identifier '(' arguments ')'
    /// ```
    fn identifier_expr(&mut self) -> Result<Expr> {
        let location = self.lexer.location();
        let name = self
            .identifier()
            .expect("the current token is an identifier");

        if self.is_char('(') {
            self.call(name, location)
        } else {
            Ok(Expr::Variable(Variable { location, name }))
        }
    }

    /// Parses the arguments of a call to `callee`, which is either the builtin `print` or another function.
    ///
    /// ```text
    /// call ::= identifier '(' (expression (',' expression)*)? ')'
    /// ```
    fn call(&mut self, callee: String, location: Location) -> Result<Expr> {
        self.lexer.advance();

        let mut args = Vec::new();
        if !self.is_char(')') {
            loop {
                args.push(self.expression()?);
                if self.is_char(')') {
                    break;
                }
                if !self.is_char(',') {
                    return self.error("', or )'", "in argument list");
                }
                self.lexer.advance();
            }
        }
        self.lexer.advance();

        // It can be a call to the builtin `print`.
        if callee == "print" {
            let Ok([arg]) = <[Expr; 1]>::try_from(args) else {
                return self.error("<single arg>", "as argument to print()");
            };
            return Ok(Expr::Print(Print {
                location,
                arg: Box::new(arg),
            }));
        }

        Ok(Expr::Call(Call {
            location,
            callee,
            args,
        }))
    }

    /// ```text
    /// parenthesized ::= '(' expression ')'
    /// ```
    fn parenthesized(&mut self) -> Result<Expr> {
        self.lexer.advance();
        let expr = self.expression()?;
        self.expect_char(')', "to close expression with parentheses")?;
        Ok(expr)
    }

    /// ```text
    /// number ::= <number token>
    /// ```
    fn number(&mut self) -> Number {
        let location = self.lexer.location();
        let Token::Number(value) = *self.lexer.current() else {
            unreachable!("the current token is a number");
        };
        self.lexer.advance();

        Number { location, value }
    }

    /// ```text
    /// tensor_literal ::= '[' literal_list ']' | number
    /// literal_list ::= tensor_literal | tensor_literal ',' literal_list
    /// ```
    fn tensor_literal(&mut self) -> Result<Literal> {
        let location = self.lexer.location();
        self.lexer.advance();

        // The values at this nesting level.
        let mut values = Vec::new();
        loop {
            // Either another nested literal or a number.
            if self.is_char('[') {
                values.push(Expr::Literal(self.tensor_literal()?));
            } else if matches!(self.lexer.current(), Token::Number(_)) {
                values.push(Expr::Number(self.number()));
            } else {
                return self.error("<num> or [", "in literal expression");
            }

            // This list ends with ']'; elements are separated by commas.
            if self.is_char(']') {
                break;
            }
            if !self.is_char(',') {
                return self.error("] or ,", "in literal expression");
            }
            self.lexer.advance();
        }
        self.lexer.advance();

        // The shape starts with the current nesting level. If there are nested
        // literals, append their shape, which must be the same for all of them.
        let mut dims = vec![i64::try_from(values.len()).expect("a literal is not that large")];
        if let Some(Expr::Literal(first)) = values.first() {
            let nested_dims = first.dims.clone();
            let is_uniform = values.iter().all(
                |value| matches!(value, Expr::Literal(literal) if literal.dims == nested_dims),
            );
            if !is_uniform {
                return self.error(
                    "uniform well-nested dimensions",
                    "inside literal expression",
                );
            }
            dims.extend(nested_dims);
        } else if values.iter().any(|value| matches!(value, Expr::Literal(_))) {
            return self.error(
                "uniform well-nested dimensions",
                "inside literal expression",
            );
        }

        Ok(Literal {
            location,
            values,
            dims,
        })
    }

    /// ```text
    /// struct_literal ::= '{' (struct_literal | tensor_literal | number)+ '}'
    /// ```
    fn struct_literal(&mut self) -> Result<StructLiteral> {
        let location = self.lexer.location();
        self.lexer.advance();

        let mut values = Vec::new();
        loop {
            if self.is_char('[') {
                values.push(Expr::Literal(self.tensor_literal()?));
            } else if matches!(self.lexer.current(), Token::Number(_)) {
                values.push(Expr::Number(self.number()));
            } else if self.is_char('{') {
                values.push(Expr::StructLiteral(self.struct_literal()?));
            } else {
                return self.error("{, [, or number", "in struct literal expression");
            }

            if self.is_char('}') {
                break;
            }
            if !self.is_char(',') {
                return self.error("} or ,", "in struct literal expression");
            }
            self.lexer.advance();
        }
        self.lexer.advance();

        Ok(StructLiteral { location, values })
    }

    /// Consumes the current token if it is an identifier, returning its name.
    fn identifier(&mut self) -> Option<String> {
        let Token::Identifier(name) = self.lexer.current() else {
            return None;
        };
        let name = name.clone();
        self.lexer.advance();
        Some(name)
    }

    fn is_char(&self, char: char) -> bool {
        *self.lexer.current() == Token::Char(char)
    }

    /// Consumes the character `char`, or fails with `context`.
    fn expect_char(&mut self, char: char, context: &'static str) -> Result<()> {
        if !self.is_char(char) {
            let expected = match char {
                '(' => "'('",
                ')' => "')'",
                '{' => "'{'",
                '}' => "'}'",
                '<' => "'<'",
                '>' => "'>'",
                '=' => "'='",
                ';' => "';'",
                _ => "a different character",
            };
            return self.error(expected, context);
        }
        self.lexer.advance();
        Ok(())
    }

    fn skip_semicolons(&mut self) {
        while self.is_char(';') {
            self.lexer.advance();
        }
    }

    /// Fails at the current token, saying what was `expected` in `context`.
    fn error<T>(&self, expected: &'static str, context: &'static str) -> Result<T> {
        Err(ParseError {
            location: self.lexer.location(),
            expected,
            context,
            found: self.lexer.current().clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_operator_precedence() {
        let module = parse("def f(a, b) { return a + b * a; }", "test.toy").unwrap();
        let Record::Function(function) = &module.records[0] else {
            panic!("expected a function");
        };
        let Expr::Return(Return {
            value: Some(value), ..
        }) = &function.body[0]
        else {
            panic!("expected a return");
        };
        let Expr::Binary(add) = value.as_ref() else {
            panic!("expected a binary operation");
        };

        assert_eq!(add.operator, BinaryOperator::Add);
        assert!(matches!(
            add.rhs.as_ref(),
            Expr::Binary(Binary {
                operator: BinaryOperator::Multiply,
                ..
            })
        ));
    }

    #[test]
    fn test_computes_literal_shape() {
        let module = parse("def main() { var a = [[1, 2, 3], [4, 5, 6]]; }", "test.toy").unwrap();
        let Record::Function(function) = &module.records[0] else {
            panic!("expected a function");
        };
        let Expr::VarDecl(VarDecl {
            init: Some(init), ..
        }) = &function.body[0]
        else {
            panic!("expected a declaration");
        };

        assert!(matches!(init.as_ref(), Expr::Literal(Literal { dims, .. }) if *dims == [2, 3]));
    }

    #[test]
    fn test_rejects_non_uniform_literal() {
        let error = parse("def main() { var a = [[1, 2], [3]]; }", "test.toy").unwrap_err();

        assert_eq!(error.expected, "uniform well-nested dimensions");
    }

    #[test]
    fn test_rejects_missing_semicolon() {
        let error = parse("def main() { print(1) }", "test.toy").unwrap_err();

        assert_eq!(error.expected, "';'");
        assert_eq!(error.found, Token::Char('}'));
    }

    #[test]
    fn test_rejects_print_with_two_arguments() {
        let error = parse("def main() { print(1, 2); }", "test.toy").unwrap_err();

        assert_eq!(error.expected, "<single arg>");
    }
}
