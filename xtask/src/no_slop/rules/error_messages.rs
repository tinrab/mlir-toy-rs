//! Lowercase openings at the error sites recognized by the shared collector.

use std::iter;

use syn::File;

use super::{
    super::{context::Context, diagnostic::Diagnostic},
    error_support,
};

fn capitalized(message: &str, formatted: bool) -> bool {
    let mut chars = message.chars().peekable();
    while let Some(ch) = chars.next() {
        if formatted && ch == '{' {
            if chars.next_if_eq(&'{').is_some() {
                continue;
            }
            // The opening is determined by runtime output, not this field's name.
            return false;
        }
        if ch.is_lowercase() {
            return false;
        }
        if ch.to_lowercase().ne(iter::once(ch)) {
            return true;
        }
    }
    false
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    error_support::messages(file).into_iter().filter(|message| capitalized(&message.literal.value(), message.formatted)).map(|message| context.report(
        "require-lowercase-error-messages", message.literal.span(),
        "error message should begin with lowercase prose",
        "Use a lowercase opening; preserve acronyms and identifiers later in the message, or explain a required opening with a local exception.",
    )).collect()
}
