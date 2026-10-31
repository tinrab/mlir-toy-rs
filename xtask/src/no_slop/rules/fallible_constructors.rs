//! Inspect explicit outer Result/Option contracts on user-controlled constructors.
//! Written type names are conventions; aliases and nested wrappers are not resolved.

use syn::{
    File, ImplItem, ItemImpl, ItemTrait, ReturnType, Signature, TraitItem, Type,
    ext::IdentExt as _,
    visit::{self, Visit},
};

use super::super::{context::Context, diagnostic::Diagnostic};

fn fallible_new(signature: &Signature) -> bool {
    if signature.ident.unraw() != "new" || signature.receiver().is_some() {
        return false;
    }
    let ReturnType::Type(_, ty) = &signature.output else {
        return false;
    };
    let mut ty = &**ty;
    loop {
        ty = match ty {
            Type::Paren(paren) => &paren.elem,
            Type::Group(group) => &group.elem,
            _ => break,
        };
    }
    matches!(ty, Type::Path(path) if path.qself.is_none()
        && path.path.segments.last().is_some_and(|segment|
            matches!(segment.ident.to_string().as_str(), "Result" | "Option")))
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    struct Check<'a> {
        context: &'a Context<'a>,
        findings: Vec<Diagnostic>,
    }

    impl Check<'_> {
        fn constructor(&mut self, signature: &Signature) {
            if fallible_new(signature) {
                self.findings.push(self.context.report(
                    "require-create-for-fallible-constructors",
                    signature.ident.span(),
                    "fallible constructor `new` should be named `create` or `create_*`",
                    "Rename the constructor and its references to create or a descriptive create_* name.",
                ));
            }
        }
    }

    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
            if item.trait_.is_none() {
                for member in &item.items {
                    if let ImplItem::Fn(function) = member {
                        self.constructor(&function.sig);
                    }
                }
            }
            visit::visit_item_impl(self, item);
        }

        fn visit_item_trait(&mut self, item: &'ast ItemTrait) {
            for member in &item.items {
                if let TraitItem::Fn(function) = member {
                    self.constructor(&function.sig);
                }
            }
            visit::visit_item_trait(self, item);
        }
    }
    let mut check = Check {
        context,
        findings: Vec::new(),
    };
    check.visit_file(file);
    check.findings
}
