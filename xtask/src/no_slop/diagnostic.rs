//! Diagnostics use one-based lines and columns and UTF-8 byte offsets.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Off,
    Warn,
    #[default]
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub byte: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub rule: &'static str,
    pub level: Level,
    #[serde(serialize_with = "serialize_path")]
    pub path: PathBuf,
    pub start: Position,
    pub end: Position,
    pub message: String,
    pub help: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<TextEdit>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TextEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

impl Diagnostic {
    pub(crate) fn with_fix(
        mut self,
        start: usize,
        end: usize,
        replacement: impl Into<String>,
    ) -> Self {
        self.fix = Some(TextEdit {
            start,
            end,
            replacement: replacement.into(),
        });
        self
    }
}

#[derive(Debug, Serialize)]
pub struct FileChange {
    #[serde(serialize_with = "serialize_path")]
    pub path: PathBuf,
    pub diff: String,
    pub applied: bool,
}

#[derive(Debug, Serialize)]
pub struct Failure {
    #[serde(serialize_with = "serialize_path")]
    pub path: PathBuf,
    pub message: String,
    pub position: Option<Position>,
}

fn serialize_path<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&path.display())
}

#[derive(Default, Debug, Serialize)]
pub struct Summary {
    pub scanned: usize,
    /// Excluded files, generated files, symlinks, and pruned directories.
    pub skipped: usize,
    pub suppressed: usize,
    pub files_changed: usize,
    pub files_would_change: usize,
}

#[derive(Default, Debug, Serialize)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub failures: Vec<Failure>,
    pub summary: Summary,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<FileChange>,
}

impl Report {
    pub fn exit_status(&self) -> u8 {
        if self.failures.is_empty() {
            u8::from(self.diagnostics.iter().any(|d| d.level == Level::Error))
        } else {
            2
        }
    }

    pub(crate) fn sort(&mut self) {
        self.diagnostics
            .sort_by(|a, b| (&a.path, a.start.byte, a.rule).cmp(&(&b.path, b.start.byte, b.rule)));
        self.failures
            .sort_by(|a, b| (&a.path, &a.message).cmp(&(&b.path, &b.message)));
        self.changes.sort_by(|a, b| a.path.cmp(&b.path));
    }
}
