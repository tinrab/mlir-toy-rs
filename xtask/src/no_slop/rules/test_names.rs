//! A narrow dictionary of placeholder names on explicitly attributed tests.

use syn::{
    File, ItemFn,
    ext::IdentExt as _,
    visit::{self, Visit},
};

use super::{
    super::{context::Context, diagnostic::Diagnostic},
    test_prefix::test_attribute,
};

fn vague(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("test_") else {
        return false;
    };
    matches!(
        suffix,
        "" | "basic" | "works" | "example" | "simple" | "test" | "case"
    ) || suffix.bytes().all(|byte| byte.is_ascii_digit())
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    struct Check<'a> {
        context: &'a Context<'a>,
        findings: Vec<Diagnostic>,
    }

    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_item_fn(&mut self, function: &'ast ItemFn) {
            let name = function.sig.ident.unraw().to_string();
            if vague(&name) && function.attrs.iter().any(|attr| test_attribute(&attr.meta)) {
                self.findings.push(self.context.report(
                    "no-vague-test-names", function.sig.ident.span(), format!("test name `{name}` does not describe the behavior being checked"),
                    "Name the behavior, scenario, or expected outcome instead of using a placeholder or number.",
                ));
            }
            visit::visit_item_fn(self, function);
        }
    }
    let mut check = Check {
        context,
        findings: Vec::new(),
    };
    check.visit_file(file);
    check.findings
}
