//! Allow only private, `pub(crate)`, and `pub` visibility.
//! A restricted path such as `pub(super)`, `pub(self)`, or `pub(in path)` ties an item to its module position.

use syn::{
    File, VisRestricted,
    spanned::Spanned as _,
    visit::{self, Visit},
};

use super::super::{context::Context, diagnostic::Diagnostic};

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    struct Check<'a> {
        context: &'a Context<'a>,
        findings: Vec<Diagnostic>,
    }

    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_vis_restricted(&mut self, restricted: &'ast VisRestricted) {
            if restricted.in_token.is_some() || !restricted.path.is_ident("crate") {
                let written = self.context.source[restricted.span().byte_range()].to_owned();
                self.findings.push(self.context.report(
                    "no-restricted-visibility",
                    restricted.span(),
                    format!("`{written}` ties the item to its module position"),
                    "Use a private item, pub(crate), or pub instead.",
                ));
            }
            visit::visit_vis_restricted(self, restricted);
        }
    }

    let mut check = Check {
        context,
        findings: Vec::new(),
    };
    check.visit_file(file);
    check.findings
}
