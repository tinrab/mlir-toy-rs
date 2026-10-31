//! Chapter 1: the Toy language and its AST.
//!
//! The expected dumps are those of the C++ tutorial's `toyc -emit=ast`.

use toy::parser::parse;

#[test]
fn test_dumps_ast() {
    let source = include_str!("../../examples/ast.toy");

    assert_eq!(parse(source, "ast.toy").unwrap().dump(), EXPECTED_AST);
}

#[test]
fn test_dumps_ast_with_structs() {
    let source = include_str!("../../examples/struct_ast.toy");

    assert_eq!(
        parse(source, "struct_ast.toy").unwrap().dump(),
        EXPECTED_STRUCT_AST
    );
}

#[test]
fn test_reports_syntax_errors_with_location() {
    let error = parse("def main() {\n  var a = [1, 2]\n}", "error.toy").unwrap_err();

    assert_eq!(
        error.to_string(),
        "error.toy:3:1: parse error: expected ';' after expression but found '}'"
    );
}

const EXPECTED_AST: &str = r#"  Module:
    Function 
      Proto 'multiply_transpose' @ast.toy:4:1
      Params: [a, b]
      Block {
        Return
          BinOp: * @ast.toy:5:25
            Call 'transpose' [ @ast.toy:5:10
              var: a @ast.toy:5:20
            ]
            Call 'transpose' [ @ast.toy:5:25
              var: b @ast.toy:5:35
            ]
      } // Block
    Function 
      Proto 'main' @ast.toy:8:1
      Params: []
      Block {
        VarDecl a<> @ast.toy:11:3
          Literal: <2, 3>[ <3>[ 1.000000e+00, 2.000000e+00, 3.000000e+00], <3>[ 4.000000e+00, 5.000000e+00, 6.000000e+00]] @ast.toy:11:11
        VarDecl b<2, 3> @ast.toy:15:3
          Literal: <6>[ 1.000000e+00, 2.000000e+00, 3.000000e+00, 4.000000e+00, 5.000000e+00, 6.000000e+00] @ast.toy:15:17
        VarDecl c<> @ast.toy:19:3
          Call 'multiply_transpose' [ @ast.toy:19:11
            var: a @ast.toy:19:30
            var: b @ast.toy:19:33
          ]
        VarDecl d<> @ast.toy:22:3
          Call 'multiply_transpose' [ @ast.toy:22:11
            var: b @ast.toy:22:30
            var: a @ast.toy:22:33
          ]
        VarDecl e<> @ast.toy:25:3
          Call 'multiply_transpose' [ @ast.toy:25:11
            var: b @ast.toy:25:30
            var: c @ast.toy:25:33
          ]
        VarDecl f<> @ast.toy:28:3
          Call 'multiply_transpose' [ @ast.toy:28:11
            Call 'transpose' [ @ast.toy:28:30
              var: a @ast.toy:28:40
            ]
            var: c @ast.toy:28:44
          ]
      } // Block
"#;

const EXPECTED_STRUCT_AST: &str = r#"  Module:
    Struct: Struct @struct_ast.toy:3:1
      Variables: [
        VarDecl a<> @struct_ast.toy:4:3
        VarDecl b<> @struct_ast.toy:5:3
      ]
    Function 
      Proto 'multiply_transpose' @struct_ast.toy:9:1
      Params: [value]
      Block {
        Return
          BinOp: * @struct_ast.toy:11:31
            Call 'transpose' [ @struct_ast.toy:11:10
              BinOp: . @struct_ast.toy:11:26
                var: value @struct_ast.toy:11:20
                var: a @struct_ast.toy:11:26
            ]
            Call 'transpose' [ @struct_ast.toy:11:31
              BinOp: . @struct_ast.toy:11:47
                var: value @struct_ast.toy:11:41
                var: b @struct_ast.toy:11:47
            ]
      } // Block
    Function 
      Proto 'main' @struct_ast.toy:14:1
      Params: []
      Block {
        VarDecl value<Struct> @struct_ast.toy:16:3
          Struct Literal:             Literal: <2, 3>[ <3>[ 1.000000e+00, 2.000000e+00, 3.000000e+00], <3>[ 4.000000e+00, 5.000000e+00, 6.000000e+00]] @struct_ast.toy:16:19
            Literal: <2, 3>[ <3>[ 1.000000e+00, 2.000000e+00, 3.000000e+00], <3>[ 4.000000e+00, 5.000000e+00, 6.000000e+00]] @struct_ast.toy:16:43
           @struct_ast.toy:16:18
        VarDecl c<> @struct_ast.toy:19:3
          Call 'multiply_transpose' [ @struct_ast.toy:19:11
            var: value @struct_ast.toy:19:30
          ]
        Print [ @struct_ast.toy:20:3
          var: c @struct_ast.toy:20:9
        ]
      } // Block
"#;
