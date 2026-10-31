//! Require `test_` on functions with a test attribute, including qualified async test attributes and tests inside `cfg_attr`.
//! Merely being in a test-only module does not make a helper a test.
//! Attribute aliases and generated tests inside opaque macro bodies are not resolved or expanded.

use syn::{
    File, ItemFn, Meta, Token,
    ext::IdentExt as _,
    punctuated::Punctuated,
    visit::{self, Visit},
};

use super::super::{context::Context, diagnostic::Diagnostic};

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    struct Check<'a> {
        context: &'a Context<'a>,
        findings: Vec<Diagnostic>,
    }

    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_item_fn(&mut self, function: &'ast ItemFn) {
            let name = function.sig.ident.unraw().to_string();
            if !name.starts_with("test_")
                && function.attrs.iter().any(|attr| test_attribute(&attr.meta))
            {
                self.findings.push(self.context.report(
                    "require-test-prefix", function.sig.ident.span(),
                    format!("test function `{name}` must begin with `test_`"),
                    "Prefix the function name with test_, then describe the case and expected outcome.",
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

pub fn test_attribute(meta: &Meta) -> bool {
    if meta
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "test")
    {
        return true;
    }
    let Meta::List(list) = meta else { return false };
    if !list.path.is_ident("cfg_attr") {
        return false;
    }
    let Ok(parts) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return false;
    };
    // The first argument is a condition, not an attribute. Inspect both active
    // and inactive branches, consistently with the rest of the source scanner.
    parts.iter().skip(1).any(test_attribute)
}
