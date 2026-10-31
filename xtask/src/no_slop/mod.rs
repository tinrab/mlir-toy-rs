//! Editable, syntax-based rules for common agent-generated Rust patterns.
//!
//! Integrate by dispatching `no-slop` to [`run`].
//! Only --fix writes application code.
//! Rules inspect written syntax, including inactive cfg branches, without macro expansion, inferred types, or cross-file alias resolution.

pub(crate) mod cli;
pub(crate) mod config;
pub(crate) mod context;
pub(crate) mod diagnostic;
mod discovery;
pub(crate) mod fix;
mod imports;
pub(crate) mod rules;
mod source;
mod suppression;
mod test_scope;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
pub use cli::run;
use context::Context;
use diagnostic::{Failure, FileChange, Level, Report};
use syn::{self, File};

use self::{cli::Options, config::Config, rules::RULES};

struct ParsedFile {
    path: PathBuf,
    source: String,
    syntax: File,
    is_test: bool,
    is_crate_root: bool,
}

pub(crate) fn scan(options: &Options) -> Result<Report> {
    let workspace = discovery::discover(options)?;
    let mut report = workspace.report;
    let mut parsed = Vec::new();
    for file in workspace.files {
        let path = file
            .path
            .strip_prefix(&workspace.root)
            .unwrap_or(&file.path);
        let source = match fs::read_to_string(&file.path) {
            Ok(source) => source,
            Err(error) => {
                report.failures.push(Failure {
                    path: path.to_owned(),
                    message: error.to_string(),
                    position: None,
                });
                continue;
            }
        };
        if let Some(syntax) = parse_source(path, &source, &mut report) {
            parsed.push(ParsedFile {
                path: file.path,
                source,
                syntax,
                is_test: file.is_test,
                is_crate_root: file.is_crate_root,
            });
        }
    }
    test_scope::propagate(&mut parsed);
    for file in parsed {
        let path = file
            .path
            .strip_prefix(&workspace.root)
            .unwrap_or(&file.path);
        let mut context = Context::new(path, &file.source, &file.syntax, file.is_test);
        context.crate_root = file.is_crate_root;
        let mut local = Report::default();
        check_file(&context, &file.syntax, &workspace.config, &mut local);
        if options.fix_mode.enabled() && local.failures.is_empty() {
            let change = (|| -> Result<_> {
                let candidate = fix::plan(path, &file.source, file.is_test, &workspace.config)?;
                if candidate == file.source {
                    return Ok(None);
                }
                let syntax = syn::parse_file(&source::prepare(&candidate))?;
                if options.fix_mode.applies() {
                    fix::commit(&file.path, &file.source, &candidate)?;
                }
                Ok(Some((candidate, syntax)))
            })();
            match change {
                Ok(Some((candidate, syntax))) => {
                    let diff =
                        similar::TextDiff::from_lines(file.source.as_str(), candidate.as_str())
                            .unified_diff()
                            .header(
                                &format!("a/{}", path.display()),
                                &format!("b/{}", path.display()),
                            )
                            .to_string();
                    report.changes.push(FileChange {
                        path: path.to_owned(),
                        diff,
                        applied: options.fix_mode.applies(),
                    });
                    if options.fix_mode.applies() {
                        report.summary.files_changed += 1;
                        local = Report::default();
                        let mut context = Context::new(path, &candidate, &syntax, file.is_test);
                        context.crate_root = file.is_crate_root;
                        check_file(&context, &syntax, &workspace.config, &mut local);
                    } else {
                        report.summary.files_would_change += 1;
                    }
                }
                Ok(None) => {}
                Err(error) => local.failures.push(Failure {
                    path: path.to_owned(),
                    position: None,
                    message: format!("could not fix file: {error:#}"),
                }),
            }
        }
        report.summary.suppressed += local.summary.suppressed;
        report.diagnostics.extend(local.diagnostics);
        report.failures.extend(local.failures);
    }
    report.sort();
    Ok(report)
}

pub(crate) fn parse_source(path: &Path, source: &str, report: &mut Report) -> Option<File> {
    let prepared = source::prepare(source);
    let comments = source::comments(&prepared);
    if source::is_generated(&prepared, &comments) {
        report.summary.skipped += 1;
        return None;
    }
    let file = match syn::parse_file(&prepared) {
        Ok(file) => file,
        Err(error) => {
            // Even a failed parse provides an address into the original source.
            let empty = File {
                shebang: None,
                frontmatter: None,
                attrs: Vec::new(),
                items: Vec::new(),
            };
            let context = Context::new(path, source, &empty, false);
            report.failures.push(Failure {
                path: path.to_owned(),
                message: error.to_string(),
                position: Some(context.position(error.span().byte_range().start)),
            });
            return None;
        }
    };
    report.summary.scanned += 1;
    Some(file)
}

pub(crate) fn check_file(context: &Context<'_>, file: &File, config: &Config, report: &mut Report) {
    let comments = &context.comments;
    let mut diagnostics = Vec::new();
    for rule in RULES {
        let level = config.level(rule.id);
        if level == Level::Off {
            continue;
        }
        diagnostics.extend(
            (rule.check)(file, context)
                .into_iter()
                .map(|mut diagnostic| {
                    diagnostic.level = level;
                    diagnostic
                }),
        );
    }
    let (suppressed, failures) = suppression::apply(context, comments, &mut diagnostics);
    report.summary.suppressed += suppressed;
    report.diagnostics.extend(diagnostics);
    report.failures.extend(failures);
}
