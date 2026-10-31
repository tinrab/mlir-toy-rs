//! Recognize literal messages at conventional error APIs without name resolution.
//! Only selected macro arguments are parsed; arbitrary macro bodies stay opaque.

use proc_macro2::TokenStream;
use syn::{
    Attribute, Expr, ExprCall, ExprMethodCall, File, ItemStruct, Lit, LitStr, Macro, Stmt, Token,
    Variant,
    parse::{ParseStream, Parser as _},
    visit::{self, Visit},
};

use super::path_name;

pub fn unwrapped(mut expr: &Expr) -> &Expr {
    loop {
        expr = match expr {
            Expr::Paren(paren) => &paren.expr,
            Expr::Group(group) => &group.expr,
            Expr::Block(block) if block.block.stmts.len() == 1 => {
                let Stmt::Expr(tail, None) = &block.block.stmts[0] else {
                    return expr;
                };
                tail
            }
            _ => return expr,
        };
    }
}

/// Parse just the message expression, leaving formatting arguments opaque.
fn argument(tokens: TokenStream, index: usize) -> Option<Expr> {
    // Unsupported macro arguments are outside this syntax rule.
    (|input: ParseStream<'_>| {
        for _ in 0..index {
            input.parse::<Expr>()?;
            input.parse::<Token![,]>()?;
        }
        let message = input.parse::<Expr>()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
            input.parse::<TokenStream>()?;
        }
        Ok(message)
    })
    .parse2(tokens)
    .ok()
}

fn format_macro(mac: &Macro) -> bool {
    matches!(
        path_name(&mac.path).as_str(),
        "format" | "std::format" | "alloc::format"
    )
}

fn error_macro(mac: &Macro) -> Option<usize> {
    let name = path_name(&mac.path);
    let name = name
        .strip_prefix("anyhow::")
        .or_else(|| name.strip_prefix("eyre::"))
        .or_else(|| name.strip_prefix("color_eyre::eyre::"))
        .unwrap_or(&name);
    match name {
        "anyhow" | "eyre" | "bail" | "format_err" => Some(0),
        "ensure" => Some(1),
        _ => None,
    }
}

#[derive(Clone)]
pub struct Message {
    pub literal: LitStr,
    pub formatted: bool,
}

fn message(expr: &Expr, formatted: bool) -> Option<Message> {
    match unwrapped(expr) {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Str(literal) => Some(Message {
                literal: literal.clone(),
                formatted,
            }),
            _ => None,
        },
        Expr::Macro(expr) if format_macro(&expr.mac) => {
            message(&argument(expr.mac.tokens.clone(), 0)?, true)
        }
        Expr::MethodCall(call)
            if call.args.is_empty()
                && matches!(
                    call.method.to_string().as_str(),
                    "to_owned" | "to_string" | "into"
                ) =>
        {
            message(&call.receiver, formatted)
        }
        _ => None,
    }
}

fn lazy_message(expr: &Expr) -> Option<Message> {
    let Expr::Closure(closure) = unwrapped(expr) else {
        return None;
    };
    if !closure.inputs.is_empty() || closure.asyncness.is_some() {
        return None;
    }
    message(&closure.body, false)
}

pub fn context_message(call: &ExprMethodCall) -> Option<Message> {
    if call.args.len() != 1 {
        return None;
    }
    match call.method.to_string().as_str() {
        "context" | "wrap_err" => message(&call.args[0], false),
        "with_context" | "wrap_err_with" => lazy_message(&call.args[0]),
        _ => None,
    }
}

fn call_message(call: &ExprCall) -> Option<Message> {
    let Expr::Path(path) = unwrapped(&call.func) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    let name = path_name(&path.path);
    let expr = match name.as_str() {
        "Err"
        | "Result::Err"
        | "std::result::Result::Err"
        | "core::result::Result::Err"
        | "Error::other"
        | "io::Error::other"
        | "std::io::Error::other"
            if call.args.len() == 1 =>
        {
            call.args.first()
        }
        "Error::new" | "io::Error::new" | "std::io::Error::new" if call.args.len() == 2 => {
            call.args.get(1)
        }
        _ => None,
    }?;
    message(expr, false)
}

pub fn messages(file: &File) -> Vec<Message> {
    #[derive(Default)]
    struct Collect(Vec<Message>);

    impl Collect {
        fn attributes(&mut self, attributes: &[Attribute]) {
            for attr in attributes
                .iter()
                .filter(|attr| attr.path().is_ident("error"))
            {
                let syn::Meta::List(list) = &attr.meta else {
                    continue;
                };
                let Some(expr) = argument(list.tokens.clone(), 0) else {
                    continue;
                };
                let Expr::Lit(literal) = unwrapped(&expr) else {
                    continue;
                };
                let Lit::Str(literal) = &literal.lit else {
                    continue;
                };
                self.0.push(Message {
                    literal: literal.clone(),
                    formatted: true,
                });
            }
        }
    }

    impl<'ast> Visit<'ast> for Collect {
        fn visit_item_struct(&mut self, item: &'ast ItemStruct) {
            self.attributes(&item.attrs);
            visit::visit_item_struct(self, item);
        }

        fn visit_variant(&mut self, variant: &'ast Variant) {
            self.attributes(&variant.attrs);
            visit::visit_variant(self, variant);
        }

        fn visit_macro(&mut self, mac: &'ast Macro) {
            let Some(index) = error_macro(mac) else {
                return;
            };
            let Some(expr) = argument(mac.tokens.clone(), index) else {
                return;
            };
            self.0.extend(message(&expr, true));
        }

        fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
            self.0.extend(context_message(call));
            if call.args.len() == 1 {
                match call.method.to_string().as_str() {
                    "ok_or" => self.0.extend(message(&call.args[0], false)),
                    "ok_or_else" => self.0.extend(lazy_message(&call.args[0])),
                    _ => {}
                }
            }
            visit::visit_expr_method_call(self, call);
        }

        fn visit_expr_call(&mut self, call: &'ast ExprCall) {
            self.0.extend(call_message(call));
            visit::visit_expr_call(self, call);
        }
    }
    let mut collector = Collect::default();
    collector.visit_file(file);
    collector.0
}
