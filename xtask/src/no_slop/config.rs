//! Workspace policy lives in Cargo's existing metadata table.

use std::{collections::BTreeMap, path::Path};

use anyhow::{Context as _, Result, ensure};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use serde_json::{self, Value};

use super::{diagnostic::Level, rules};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub rules: BTreeMap<String, Level>,
}

impl Config {
    pub fn parse(metadata: &Value) -> Result<Self> {
        let value = metadata.get("no-slop").cloned();
        let config: Self = match value {
            Some(value) => serde_json::from_value(value)
                .context("invalid [workspace.metadata.no-slop] configuration")?,
            None => Self::default(),
        };
        for id in config.rules.keys() {
            ensure!(rules::is_known(id), "unknown no-slop rule: {id}");
        }
        Ok(config)
    }

    pub fn exclusions(&self) -> Result<GlobSet> {
        let mut builder = GlobSetBuilder::new();
        for pattern in &self.exclude {
            builder.add(
                GlobBuilder::new(pattern)
                    .literal_separator(true)
                    .build()
                    .with_context(|| format!("invalid exclusion: {pattern}"))?,
            );
        }
        builder.build().context("building exclusion patterns")
    }

    pub fn level(&self, id: &str) -> Level {
        self.rules.get(id).copied().unwrap_or_default()
    }
}

/// Always omit artifacts, dependency copies, and installed skill source bundles.
pub fn default_excluded(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(
                "target"
                    | "thirdparty"
                    | "vendor"
                    | ".git"
                    | ".agent"
                    | ".agents"
                    | ".claude"
                    | ".codex"
                    | ".continue"
                    | ".cursor"
                    | ".gemini"
                    | ".opencode"
                    | ".pi"
                    | ".roo"
                    | ".windsurf"
            )
        )
    }) || path
        .components()
        .collect::<Vec<_>>()
        .windows(3)
        .any(|parts| {
            parts[0].as_os_str() == "skills"
                && parts[1].as_os_str() == "install-no-slop-rs"
                && parts[2].as_os_str() == "assets"
        })
}
