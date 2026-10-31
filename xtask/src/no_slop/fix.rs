//! Plan trivia-only edits in memory, then commit a validated file replacement.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{ErrorKind, Write as _},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context as _, Result, bail, ensure};
use ra_ap_rustc_lexer::{self, FrontmatterAllowed, TokenKind};

use super::{
    check_file,
    config::Config,
    context::Context,
    diagnostic::{Report, TextEdit},
    source,
};

pub fn newline(source: &str, at: usize) -> &'static str {
    let line_end = source[..at].rfind('\n').or_else(|| source.find('\n'));
    if line_end.is_some_and(|p| p > 0 && source.as_bytes()[p - 1] == b'\r') {
        "\r\n"
    } else {
        "\n"
    }
}

/// Original source coordinates are discarded after each batch and recalculated.
pub fn plan(path: &Path, original: &str, test_file: bool, config: &Config) -> Result<String> {
    let mut current = original.to_owned();
    let mut seen = BTreeSet::new();
    for pass in 0..=16 {
        ensure!(
            seen.insert(current.clone()),
            "fixes cycle instead of stabilizing"
        );
        let syntax =
            syn::parse_file(&source::prepare(&current)).context("fixed source does not parse")?;
        let context = Context::new(path, &current, &syntax, test_file);
        let mut report = Report::default();
        check_file(&context, &syntax, config, &mut report);
        ensure!(
            report.failures.is_empty(),
            "fixing would leave invalid local exceptions"
        );
        let edits: Vec<_> = report
            .diagnostics
            .into_iter()
            .filter_map(|d| d.fix)
            .collect();
        if edits.is_empty() {
            return Ok(current);
        }
        ensure!(pass < 16, "fixes did not stabilize within 16 passes");
        let candidate = apply_edits(&current, edits)?;
        ensure!(candidate != current, "fixes made no progress");
        validate(original, &candidate)?;
        current = candidate;
    }
    unreachable!()
}

/// Prefer replacements over insertions at a shared boundary; retry deferred edits.
pub fn apply_edits(source: &str, mut edits: Vec<TextEdit>) -> Result<String> {
    for edit in &edits {
        ensure!(
            edit.start <= edit.end
                && edit.end <= source.len()
                && source.is_char_boundary(edit.start)
                && source.is_char_boundary(edit.end),
            "fix has an invalid source range"
        );
    }
    edits.sort_by(|a, b| {
        (a.start, a.start == a.end, a.end, &a.replacement).cmp(&(
            b.start,
            b.start == b.end,
            b.end,
            &b.replacement,
        ))
    });
    edits.dedup();
    let mut selected: Vec<TextEdit> = Vec::new();
    for edit in edits {
        if selected.last().is_some_and(|last| {
            edit.end < last.end
                || (edit.start == last.start)
                || (last.start == last.end && edit.start == edit.end)
        }) {
            continue;
        }
        selected.push(edit);
    }
    let mut result = source.to_owned();
    for edit in selected.into_iter().rev() {
        result.replace_range(edit.start..edit.end, &edit.replacement);
    }
    Ok(result)
}

/// Retain doc comments and every token other than ordinary comments/whitespace.
pub fn tokens(source: &str) -> Vec<String> {
    let prepared = source::prepare(source);
    let mut offset = 0;
    ra_ap_rustc_lexer::tokenize(&prepared, FrontmatterAllowed::No)
        .filter_map(|token| {
            let start = offset;
            offset += token.len as usize;
            match token.kind {
                TokenKind::Whitespace
                | TokenKind::LineComment { doc_style: None }
                | TokenKind::BlockComment {
                    doc_style: None, ..
                } => None,
                _ => Some(prepared[start..offset].to_owned()),
            }
        })
        .collect()
}

/// Like [`tokens`], but split doc line comments into words, so reflowing a doc paragraph keeps the same documentation.
pub fn doc_words(source: &str) -> Vec<String> {
    tokens(source)
        .into_iter()
        .flat_map(|token| {
            let marker = ["///", "//!"]
                .into_iter()
                .find(|marker| token.starts_with(marker) && !token.starts_with("////"));
            match marker {
                Some(marker) => token[marker.len()..]
                    .split_whitespace()
                    .map(|word| format!("{marker}{word}"))
                    .collect(),
                None => vec![token],
            }
        })
        .collect()
}

pub fn validate(original: &str, candidate: &str) -> Result<()> {
    let prefix = source::prefix_len(original);
    ensure!(
        source::prefix_len(candidate) == prefix && candidate.starts_with(&original[..prefix]),
        "fix changes the BOM or shebang"
    );
    ensure!(
        doc_words(original) == doc_words(candidate)
            || super::imports::normalized(original)? == super::imports::normalized(candidate)?,
        "fix changes executable tokens or documentation"
    );
    let prepared = source::prepare(candidate);
    syn::parse_file(&prepared).context("fixed source does not parse")?;
    ensure!(
        !source::is_generated(&prepared, &source::comments(&prepared)),
        "fix would mark the file as generated"
    );
    Ok(())
}

struct Temporary(PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Recheck the snapshot before replacement; never truncate the original file.
pub fn commit(path: &Path, original: &str, candidate: &str) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let unchanged = || -> Result<()> {
        ensure!(
            fs::symlink_metadata(path)?.file_type().is_file(),
            "source is no longer a regular file"
        );
        ensure!(
            fs::read(path)? == original.as_bytes(),
            "source changed since it was scanned"
        );
        Ok(())
    };
    unchanged()?;
    let permissions = fs::metadata(path)?.permissions();
    ensure!(!permissions.readonly(), "source file is read-only");
    let parent = path.parent().context("source file has no parent")?;
    for _ in 0..32 {
        let mut name = OsString::from(".no-slop-fix-");
        name.push(path.file_name().context("source file has no name")?);
        name.push(format!(
            ".{}.{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = parent.join(name);
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let guard = Temporary(temporary);
        file.write_all(candidate.as_bytes())?;
        file.set_permissions(permissions)?;
        file.sync_all()?;
        drop(file);
        unchanged()?;
        fs::rename(&guard.0, path).context("replacing source with fixed contents")?;
        return Ok(());
    }
    bail!("could not create a temporary file for fixing")
}
