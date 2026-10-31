//! Require `name/mod.rs` for a module with out-of-line children instead of a sibling `name.rs` file.
//! Crate roots, such as `lib.rs`, `main.rs`, tests, and examples, keep their children beside them.
//! Children declared with `#[path]` are not counted.

use syn::{File, Item};

use super::super::{context::Context, diagnostic::Diagnostic};

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    let is_mod_file = context
        .path
        .file_name()
        .is_some_and(|name| name == "mod.rs");
    if context.crate_root || is_mod_file {
        return Vec::new();
    }
    let Some(child) = file.items.iter().find_map(|item| match item {
        Item::Mod(module)
            if module.content.is_none()
                && !module.attrs.iter().any(|attr| attr.path().is_ident("path")) =>
        {
            Some(module)
        }
        _ => None,
    }) else {
        return Vec::new();
    };
    let stem = context
        .path
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    vec![context.report(
        "require-mod-rs-layout",
        child.ident.span(),
        format!("module `{stem}` declares child modules but is not a `mod.rs` file"),
        "Move the file to <name>/mod.rs next to its children.",
    )]
}
