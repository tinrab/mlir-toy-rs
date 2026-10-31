//! Check exact, case-sensitive suffixes on locally declared types and traits.
//! A domain where one of these words has a precise meaning can exempt the item.

use proc_macro2::Ident;
use syn::{
    File, ItemEnum, ItemStruct, ItemTrait, ItemType,
    visit::{self, Visit},
};

use super::super::{context::Context, diagnostic::Diagnostic};

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    struct Check<'a> {
        context: &'a Context<'a>,
        findings: Vec<Diagnostic>,
    }

    impl Check<'_> {
        fn name(&mut self, ident: &Ident) {
            let name = ident.to_string();
            if ["Manager", "Helper", "Processor", "Data"]
                .iter()
                .any(|suffix| name.ends_with(suffix))
            {
                self.findings.push(self.context.report(
                    "no-vague-type-names", ident.span(), format!("type name `{name}` does not identify a responsibility"),
                    "Name the domain concept or operation; explain a precise domain meaning with a local exception.",
                ));
            }
        }
    }

    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_item_struct(&mut self, item: &'ast ItemStruct) {
            self.name(&item.ident);
            visit::visit_item_struct(self, item);
        }

        fn visit_item_enum(&mut self, item: &'ast ItemEnum) {
            self.name(&item.ident);
            visit::visit_item_enum(self, item);
        }

        fn visit_item_trait(&mut self, item: &'ast ItemTrait) {
            self.name(&item.ident);
            visit::visit_item_trait(self, item);
        }

        fn visit_item_type(&mut self, item: &'ast ItemType) {
            self.name(&item.ident);
            visit::visit_item_type(self, item);
        }
    }
    let mut check = Check {
        context,
        findings: Vec::new(),
    };
    check.visit_file(file);
    check.findings
}
