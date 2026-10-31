//! Check direct standard-error fields in explicitly derived thiserror wrappers.
//! Derive/type aliases, containers, and conditional attributes are not resolved.

use syn::{
    Attribute, Fields, File, Ident, ItemEnum, ItemStruct, Meta, Path, Token, Type,
    ext::IdentExt as _,
    punctuated::Punctuated,
    visit::{self, Visit},
};

use super::{
    super::{context::Context, diagnostic::Diagnostic},
    path_name,
};

fn conditional(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .any(|attr| attr.path().is_ident("cfg_attr"))
}

fn derived(attributes: &[Attribute]) -> bool {
    !conditional(attributes)
        && attributes.iter().any(|attr| {
            attr.path().is_ident("derive")
                && attr
                    .parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
                    .is_ok_and(|paths| {
                        paths.iter().any(|path| {
                            matches!(path_name(path).as_str(), "Error" | "thiserror::Error")
                        })
                    })
        })
}

fn standard_error(ty: &Type) -> bool {
    let ty = match ty {
        Type::Paren(ty) => return standard_error(&ty.elem),
        Type::Group(ty) => return standard_error(&ty.elem),
        Type::Path(ty) if ty.qself.is_none() => ty,
        _ => return false,
    };
    matches!(
        path_name(&ty.path).as_str(),
        "std::io::Error"
            | "io::Error"
            | "std::num::ParseIntError"
            | "core::num::ParseIntError"
            | "num::ParseIntError"
            | "ParseIntError"
            | "std::num::ParseFloatError"
            | "core::num::ParseFloatError"
            | "num::ParseFloatError"
            | "ParseFloatError"
            | "std::str::Utf8Error"
            | "core::str::Utf8Error"
            | "str::Utf8Error"
            | "Utf8Error"
            | "std::string::FromUtf8Error"
            | "alloc::string::FromUtf8Error"
            | "string::FromUtf8Error"
            | "FromUtf8Error"
    )
}

fn transparent(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attr| {
        attr.path().is_ident("error")
            && attr
                .parse_args::<Meta>()
                .is_ok_and(|meta| meta.path().is_ident("transparent"))
    })
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    struct Check<'a> {
        context: &'a Context<'a>,
        findings: Vec<Diagnostic>,
    }

    impl Check<'_> {
        fn wrapper(&mut self, name: &Ident, attrs: &[Attribute], fields: &Fields) {
            if conditional(attrs)
                || transparent(attrs)
                || fields.iter().any(|field| conditional(&field.attrs))
            {
                return;
            }
            let has_source = fields.iter().any(|field| {
                field
                    .ident
                    .as_ref()
                    .is_some_and(|name| name.unraw() == "source")
                    || field
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident("source") || attr.path().is_ident("from"))
            });
            if !has_source && fields.iter().any(|field| standard_error(&field.ty)) {
                self.findings.push(self.context.report(
                    "require-error-sources", name.span(), "error wrapper retains a standard error without declaring its source",
                    "Mark the underlying error with #[source] or #[from], name its field source, or use #[error(transparent)] when appropriate.",
                ));
            }
        }
    }

    impl<'ast> Visit<'ast> for Check<'_> {
        fn visit_item_struct(&mut self, item: &'ast ItemStruct) {
            if derived(&item.attrs) {
                self.wrapper(&item.ident, &item.attrs, &item.fields);
            }
            visit::visit_item_struct(self, item);
        }

        fn visit_item_enum(&mut self, item: &'ast ItemEnum) {
            if derived(&item.attrs) {
                for variant in &item.variants {
                    self.wrapper(&variant.ident, &variant.attrs, &variant.fields);
                }
            }
            visit::visit_item_enum(self, item);
        }
    }
    let mut check = Check {
        context,
        findings: Vec::new(),
    };
    check.visit_file(file);
    check.findings
}
