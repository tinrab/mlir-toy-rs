//! Only direct inline `map_err` closures returning the bound error's `to_string()`.
//! Method spellings follow Result conventions; types and aliases are not resolved.

use syn::{
    Expr, ExprMethodCall, File, Pat, Stmt,
    ext::IdentExt as _,
    visit::{self, Visit},
};

use super::super::{context::Context, diagnostic::Diagnostic};

fn unwrapped(mut expr: &Expr) -> &Expr {
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

fn stringifies_error(call: &ExprMethodCall) -> bool {
    if call.method != "map_err" || call.args.len() != 1 {
        return false;
    }
    let Expr::Closure(closure) = unwrapped(&call.args[0]) else {
        return false;
    };
    if closure.asyncness.is_some() || closure.inputs.len() != 1 {
        return false;
    }
    let Pat::Ident(binding) = &closure.inputs[0] else {
        return false;
    };
    if binding.by_ref.is_some() || binding.subpat.is_some() {
        return false;
    }
    let Expr::MethodCall(conversion) = unwrapped(&closure.body) else {
        return false;
    };
    if conversion.method != "to_string" || !conversion.args.is_empty() {
        return false;
    }
    let Expr::Path(receiver) = unwrapped(&conversion.receiver) else {
        return false;
    };
    receiver.qself.is_none()
        && receiver
            .path
            .get_ident()
            .is_some_and(|ident| ident.unraw() == binding.ident.unraw())
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    struct Check<'a> {
        context: &'a Context<'a>,
        findings: Vec<Diagnostic>,
    }

    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
            if stringifies_error(call) {
                self.findings.push(self.context.report(
                    "no-stringified-error-mapping", call.method.span(),
                    "error mapping replaces the original error with its display string",
                    "Wrap the original error or add context while retaining its source; explain intentional serialization with a local exception.",
                ));
            }
            visit::visit_expr_method_call(self, call);
        }
    }
    let mut check = Check {
        context,
        findings: Vec::new(),
    };
    check.visit_file(file);
    check.findings
}
