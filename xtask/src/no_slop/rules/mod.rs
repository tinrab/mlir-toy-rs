//! Each rule owns its traversal.
//! Add a function and register it here to add a rule.

mod comments;
mod doc_sentences;
mod duplicate_context;
mod error_messages;
mod error_sources;
mod error_support;
mod fallible_constructors;
mod grouped_imports;
mod lint_suppressions;
mod module_layout;
mod readable_spacing;
mod string_errors;
mod stringified_errors;
mod test_names;
mod test_prefix;
mod vague_errors;
mod vague_names;
mod visibility;

use syn::{File, Path};

use super::{context::Context, diagnostic::Diagnostic};

pub struct Rule {
    pub id: &'static str,
    pub description: &'static str,
    pub fixable: bool,
    pub check: fn(&File, &Context<'_>) -> Vec<Diagnostic>,
}

pub const RULES: &[Rule] = &[
    Rule {
        id: "no-vague-error-messages",
        fixable: false,
        description: "Replace exact generic error messages with operation-specific explanations.",
        check: vague_errors::check,
    },
    Rule {
        id: "no-duplicate-error-context",
        fixable: false,
        description: "Avoid adjacent literal error contexts that repeat the same message.",
        check: duplicate_context::check,
    },
    Rule {
        id: "require-error-sources",
        fixable: false,
        description: "Declare a source when a thiserror wrapper retains a recognized standard error.",
        check: error_sources::check,
    },
    Rule {
        id: "no-vague-test-names",
        fixable: false,
        description: "Replace placeholder and numeric-only test names with descriptions of behavior.",
        check: test_names::check,
    },
    Rule {
        id: "require-lowercase-error-messages",
        fixable: false,
        description: "Begin literal messages at conventional error APIs with lowercase prose.",
        check: error_messages::check,
    },
    Rule {
        id: "require-create-for-fallible-constructors",
        fixable: false,
        description: "Name fallible associated constructors create or create_* instead of new.",
        check: fallible_constructors::check,
    },
    Rule {
        id: "require-grouped-imports",
        fixable: true,
        description: "Group imports by written root and separate standard, external, and local roots.",
        check: grouped_imports::check,
    },
    Rule {
        id: "require-test-prefix",
        fixable: false,
        description: "Name test-attributed functions with the test_ prefix; ordinary helpers are exempt.",
        check: test_prefix::check,
    },
    Rule {
        id: "require-readable-spacing",
        fixable: true,
        description: "Separate sibling declarations with blank lines while preserving compact imports, modules, and constants.",
        check: readable_spacing::check,
    },
    Rule {
        id: "no-string-errors",
        fixable: false,
        description: "Use meaningful error types instead of Result errors of String or &str.",
        check: string_errors::check,
    },
    Rule {
        id: "no-blanket-lint-suppression",
        fixable: false,
        description: "Avoid suppressing warnings or clippy::all as a group.",
        check: lint_suppressions::check_blanket,
    },
    Rule {
        id: "no-vague-type-names",
        fixable: false,
        description: "Name responsibilities rather than Manager, Helper, Processor, or Data types.",
        check: vague_names::check,
    },
    Rule {
        id: "no-decorative-comments",
        fixable: true,
        description: "Replace decorative comment separators and framed headings with ordinary explanations.",
        check: comments::check_decorative,
    },
    Rule {
        id: "no-boilerplate-comments",
        fixable: true,
        description: "Remove exact boilerplate labels that add no explanation.",
        check: comments::check_boilerplate,
    },
    Rule {
        id: "no-vague-todos",
        fixable: false,
        description: "Replace empty TODO/FIXME comments and generic placeholders with concrete tasks.",
        check: comments::check_todos,
    },
    Rule {
        id: "no-stringified-error-mapping",
        fixable: false,
        description: "Preserve the original error instead of directly mapping it to its display string.",
        check: stringified_errors::check,
    },
    Rule {
        id: "no-restricted-visibility",
        fixable: false,
        description: "Use private, pub(crate), or pub visibility instead of pub(super), pub(self), or pub(in path).",
        check: visibility::check,
    },
    Rule {
        id: "require-mod-rs-layout",
        fixable: false,
        description: "Put a module with child modules in name/mod.rs instead of a sibling name.rs file.",
        check: module_layout::check,
    },
    Rule {
        id: "require-sentence-per-line",
        fixable: true,
        description: "Write each sentence of a doc comment on its own line.",
        check: doc_sentences::check,
    },
];

pub fn is_known(id: &str) -> bool {
    RULES.iter().any(|rule| rule.id == id)
}

pub fn path_name(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
