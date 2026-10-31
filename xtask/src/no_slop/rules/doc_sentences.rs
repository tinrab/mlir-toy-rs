//! Put each sentence of a `///` or `//!` comment on its own line.
//! Prose paragraphs and list items are rejoined and split after sentence-ending punctuation that is followed by a capitalized or mixed-case word, backtick, or link.
//! Code blocks, tables, headings, block quotes, HTML, and link definitions are left alone.
//! Inline code, brackets, parentheses, and common abbreviations never end a sentence.
//! Lines are aligned to the documented item and trailing empty lines are dropped, since rustfmt skips doc comments inside macro bodies.

use syn::File;

use super::super::{context::Context, diagnostic::Diagnostic, fix};

const ABBREVIATIONS: &[&str] = &["e.g", "i.e", "etc", "vs", "cf", "approx", "no", "No"];

struct Line<'a> {
    start: usize,
    end: usize,
    indent: &'a str,
    marker: &'a str,
    body: &'a str,
}

enum Kind {
    Prose { indent: String },
    Item { first: String, continuation: String },
}

struct Unit {
    first: usize,
    last: usize,
    kind: Kind,
    text: String,
}

pub fn check(_file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    let lines = doc_lines(context);
    let mut findings = Vec::new();
    let mut block_start = 0;
    for index in 1..=lines.len() {
        let continues = index < lines.len() && {
            let (previous, current) = (&lines[index - 1], &lines[index]);
            matches!(&context.source[previous.end..current.start], "\n" | "\r\n")
                && previous.marker == current.marker
        };
        if !continues {
            check_block(context, &lines[block_start..index], &mut findings);
            block_start = index;
        }
    }
    findings
}

fn doc_lines<'a>(context: &Context<'a>) -> Vec<Line<'a>> {
    let source = context.source;
    context
        .comments
        .iter()
        .filter_map(|comment| {
            let marker = if comment.text.starts_with("//!") {
                "//!"
            } else if comment.text.starts_with("///") && !comment.text.starts_with("////") {
                "///"
            } else {
                return None;
            };
            let start = source[..comment.range.start]
                .rfind('\n')
                .map_or(0, |p| p + 1);
            let indent = &source[start..comment.range.start];
            if !indent.chars().all(|c| c == ' ' || c == '\t') {
                return None;
            }
            let text = comment.text.trim_end_matches('\r');
            let body = &text[marker.len()..];
            Some(Line {
                start,
                end: comment.range.start + text.len(),
                indent,
                marker,
                body: body.strip_prefix(' ').unwrap_or(body),
            })
        })
        .collect()
}

fn check_block(context: &Context<'_>, lines: &[Line<'_>], findings: &mut Vec<Diagnostic>) {
    let indent = block_indent(context, lines);
    check_layout(context, lines, indent, findings);
    let mut fenced = false;
    let mut unit: Option<Unit> = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.body.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            flush(context, lines, indent, unit.take(), findings);
            fenced = !fenced;
            continue;
        }
        if fenced || trimmed.is_empty() || literal(line.body, unit.is_some()) {
            flush(context, lines, indent, unit.take(), findings);
            continue;
        }
        if let Some(content) = list_item_content(line.body) {
            flush(context, lines, indent, unit.take(), findings);
            unit = Some(Unit {
                first: index,
                last: index,
                kind: Kind::Item {
                    first: line.body[..content].to_owned(),
                    continuation: " ".repeat(content),
                },
                text: line.body[content..].trim().to_owned(),
            });
        } else if let Some(current) = &mut unit {
            current.last = index;
            current.text.push(' ');
            current.text.push_str(trimmed);
        } else {
            let indent = &line.body[..line.body.len() - line.body.trim_start().len()];
            unit = Some(Unit {
                first: index,
                last: index,
                kind: Kind::Prose {
                    indent: indent.to_owned(),
                },
                text: trimmed.to_owned(),
            });
        }
        // A trailing backslash or two spaces is a hard line break.
        if line.body.ends_with("  ") || line.body.ends_with('\\') {
            flush(context, lines, indent, unit.take(), findings);
        }
    }
    flush(context, lines, indent, unit, findings);
}

/// An outer doc comment takes the indent of the item it documents; an inner one keeps its first line's indent.
fn block_indent<'a>(context: &Context<'a>, lines: &[Line<'a>]) -> &'a str {
    let first = &lines[0];
    let last = &lines[lines.len() - 1];
    if first.marker != "///" {
        return first.indent;
    }
    let rest = &context.source[last.end..];
    let next = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or_default();
    let line = &next[..next.find('\n').unwrap_or(next.len())];
    let indent = &next[..line.len() - line.trim_start().len()];
    if line.trim().is_empty() {
        first.indent
    } else {
        indent
    }
}

/// Aligns every line of a block and drops trailing empty lines, which macro bodies keep because rustfmt skips them.
fn check_layout(
    context: &Context<'_>,
    lines: &[Line<'_>],
    indent: &str,
    findings: &mut Vec<Diagnostic>,
) {
    let kept = lines
        .iter()
        .rposition(|line| !line.body.trim().is_empty())
        .map_or(lines.len(), |last| last + 1);
    let misaligned = lines[..kept].iter().find(|line| line.indent != indent);
    let (line, message) = match misaligned {
        Some(line) => (line, "doc comment line is not aligned with its comment"),
        None if kept < lines.len() => (&lines[kept], "doc comment ends with an empty line"),
        None => return,
    };
    let first = &lines[0];
    let last = &lines[lines.len() - 1];
    let newline = fix::newline(context.source, first.start);
    let replacement = lines[..kept]
        .iter()
        .map(|line| {
            format!(
                "{indent}{}",
                &context.source[line.start + line.indent.len()..line.end]
            )
        })
        .collect::<Vec<_>>()
        .join(newline);
    findings.push(
        context
            .report_range(
                "require-sentence-per-line",
                line.start + line.indent.len()..line.end,
                message,
                "Indent each doc comment line like the item it documents, and drop trailing empty lines.",
            )
            .with_fix(first.start, last.end, replacement),
    );
}

/// Lines whose layout carries meaning, or indented code outside a paragraph.
fn literal(body: &str, in_paragraph: bool) -> bool {
    let trimmed = body.trim_start();
    trimmed.starts_with(['|', '#', '>', '<'])
        || (trimmed.starts_with('[') && trimmed.contains("]:") && !trimmed.contains("]("))
        || (!in_paragraph && body.starts_with("    "))
}

/// Returns the byte offset of a list item's content, after its marker and space.
fn list_item_content(body: &str) -> Option<usize> {
    let indent = body.len() - body.trim_start().len();
    let rest = &body[indent..];
    let marker = if rest.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || !rest[digits..].starts_with(['.', ')']) {
            return None;
        }
        digits + 1
    };
    rest[marker..].starts_with(' ').then(|| indent + marker + 1)
}

fn flush(
    context: &Context<'_>,
    lines: &[Line<'_>],
    indent: &str,
    unit: Option<Unit>,
    findings: &mut Vec<Diagnostic>,
) {
    let Some(unit) = unit else { return };
    let sentences = sentences(&unit.text);
    let bodies: Vec<String> = sentences
        .iter()
        .enumerate()
        .map(|(index, sentence)| match &unit.kind {
            Kind::Prose { indent } => format!("{indent}{sentence}"),
            Kind::Item { first, .. } if index == 0 => format!("{first}{sentence}"),
            Kind::Item { continuation, .. } => format!("{continuation}{sentence}"),
        })
        .collect();
    let original = &lines[unit.first..=unit.last];
    if bodies.len() == original.len()
        && bodies
            .iter()
            .zip(original)
            .all(|(body, line)| body == line.body.trim_end())
    {
        return;
    }
    let several = original
        .iter()
        .any(|line| !sentences_of(line.body.trim()).is_empty());
    let message = if several {
        "doc comment line holds more than one sentence"
    } else {
        "sentence continues onto the next doc comment line"
    };
    let first = &original[0];
    let last = &original[original.len() - 1];
    let newline = fix::newline(context.source, first.start);
    let replacement = bodies
        .iter()
        .map(|body| format!("{indent}{} {body}", first.marker))
        .collect::<Vec<_>>()
        .join(newline);
    findings.push(
        context
            .report_range(
                "require-sentence-per-line",
                first.start + first.indent.len()..first.end,
                message,
                "Put each sentence on its own line, with a blank comment line between paragraphs.",
            )
            .with_fix(first.start, last.end, replacement),
    );
}

fn sentences(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    for split in sentences_of(text) {
        parts.push(text[start..split].trim());
        start = split;
    }
    parts.push(text[start..].trim());
    parts
}

/// Returns the byte offsets where a sentence after the first one begins.
fn sentences_of(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = Vec::new();
    let mut code = false;
    let mut depth = 0_usize;
    for (index, &byte) in bytes.iter().enumerate() {
        match byte {
            b'`' => code = !code,
            b'(' | b'[' if !code => depth += 1,
            b')' | b']' if !code => depth = depth.saturating_sub(1),
            b'.' | b'?' | b'!' if !code && depth == 0 && bytes.get(index + 1) == Some(&b' ') => {
                let next =
                    index + 1 + text[index + 1..].len() - text[index + 1..].trim_start().len();
                // Mixed-case names such as nvCOMP or iOS also start a sentence.
                let next_word = text[next..]
                    .split(|c: char| !c.is_alphanumeric())
                    .next()
                    .unwrap_or_default();
                let starts_sentence = next_word.chars().any(char::is_uppercase)
                    || text[next..].starts_with(['`', '[']);
                let word_start = text[..index]
                    .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
                    .map_or(0, |p| p + 1);
                let word = &text[word_start..index];
                // A single letter or number before a period is an initial or list number.
                let abbreviation = ABBREVIATIONS.contains(&word)
                    || word.ends_with('.')
                    || (byte == b'.'
                        && (word.chars().count() == 1
                            || (!word.is_empty() && word.bytes().all(|b| b.is_ascii_digit()))));
                if starts_sentence && !abbreviation {
                    starts.push(next);
                }
            }
            _ => {}
        }
    }
    starts
}
