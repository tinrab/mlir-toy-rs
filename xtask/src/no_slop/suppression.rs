//! A directive covers leading comments and one following statement or item.

use std::ops::Range;

use super::{
    context::Context,
    diagnostic::{Diagnostic, Failure},
    rules,
    source::{self, Comment},
};

struct Suppression<'a> {
    rule: &'a str,
    range: Range<usize>,
}

pub fn apply(
    context: &Context<'_>,
    comments: &[Comment<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) -> (usize, Vec<Failure>) {
    let mut suppressions = Vec::new();
    let mut failures = Vec::new();
    for comment in comments.iter().filter(|comment| comment.ordinary_line) {
        if context
            .opaque_macros
            .iter()
            .any(|range| range.contains(&comment.range.start))
        {
            continue;
        }
        let text = comment.text.trim_start_matches('/').trim();
        let Some(body) = text.strip_prefix("no-slop-allow") else {
            continue;
        };
        let parsed = body
            .strip_prefix(':')
            .and_then(|body| body.split_once("--"))
            .and_then(|(id, reason)| {
                let id = id.trim();
                (rules::is_known(id) && !reason.trim().is_empty()).then_some(id)
            });
        let target = context.targets.iter().find(|range| {
            range.start >= comment.range.end
                && source::trivia_only(&context.source[comment.range.end..range.start])
        });
        match (parsed, target) {
            (Some(rule), Some(range)) => suppressions.push(Suppression { rule, range: comment.range.end..range.end }),
            _ => failures.push(Failure {
                path: context.path.to_owned(),
                position: Some(context.position(comment.range.start)),
                message: "invalid no-slop-allow directive: require a known rule, `-- reason`, and a following statement or item".into(),
            }),
        }
    }
    let before = diagnostics.len();
    diagnostics.retain(|diagnostic| {
        !suppressions.iter().any(|suppression| {
            suppression.rule == diagnostic.rule
                && suppression.range.contains(&diagnostic.start.byte)
        })
    });
    (before - diagnostics.len(), failures)
}
