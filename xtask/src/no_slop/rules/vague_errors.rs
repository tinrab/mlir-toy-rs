//! Exact complete-message placeholders at conventional error APIs.

use syn::File;

use super::{
    super::{context::Context, diagnostic::Diagnostic},
    error_support,
};

fn vague(message: &str) -> bool {
    let normalized = message
        .trim_end_matches(|ch: char| ch.is_whitespace() || ".!?:;…".contains(ch))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "error"
            | "failed"
            | "failure"
            | "error occurred"
            | "an error occurred"
            | "something went wrong"
            | "operation failed"
            | "unknown error"
            | "unexpected error"
            | "an unexpected error occurred"
    )
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    error_support::messages(file)
        .into_iter()
        .filter(|message| vague(&message.literal.value()))
        .map(|message| {
            context.report(
                "no-vague-error-messages",
                message.literal.span(),
                "error message does not identify the operation or failure",
                "Describe the operation or failure instead of using a generic placeholder message.",
            )
        })
        .collect()
}
