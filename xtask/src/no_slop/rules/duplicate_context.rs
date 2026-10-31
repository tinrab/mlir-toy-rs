//! Compare adjacent literal error contexts, without resolving receiver types.

use syn::{
    Expr, ExprMethodCall, File,
    visit::{self, Visit},
};

use super::{
    super::{context::Context, diagnostic::Diagnostic},
    error_support::{context_message, unwrapped},
};

fn duplicate(outer: &ExprMethodCall) -> bool {
    let Expr::MethodCall(inner) = unwrapped(&outer.receiver) else {
        return false;
    };
    let (Some(outer_message), Some(inner_message)) =
        (context_message(outer), context_message(inner))
    else {
        return false;
    };
    !outer_message.formatted
        && !inner_message.formatted
        && outer_message.literal.value() == inner_message.literal.value()
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    struct Check<'a> {
        context: &'a Context<'a>,
        findings: Vec<Diagnostic>,
    }

    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_expr_method_call(&mut self, outer: &'ast ExprMethodCall) {
            if duplicate(outer) {
                self.findings.push(self.context.report(
                            "no-duplicate-error-context", outer.method.span(), "adjacent error contexts repeat the same message",
                            "Remove the duplicate context or describe the additional operation or boundary.",
                        ));
            }
            visit::visit_expr_method_call(self, outer);
        }
    }
    let mut check = Check {
        context,
        findings: Vec::new(),
    };
    check.visit_file(file);
    check.findings
}
