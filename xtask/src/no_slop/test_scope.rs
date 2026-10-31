//! Propagate test-only scope through ordinary file-backed module declarations.
//! A file also referenced from production remains production.
//! Macro-generated modules and conditional path attributes are not resolved.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use syn::{Expr, Item, Lit, Meta};

use super::{ParsedFile, context};

struct Edge {
    source: usize,
    target: usize,
    test_only: bool,
}

pub fn propagate(files: &mut [ParsedFile]) {
    let paths: BTreeMap<_, _> = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.path.clone(), index))
        .collect();
    let mut edges = Vec::new();
    for (index, file) in files.iter_mut().enumerate() {
        file.is_test |= context::test_attributes(&file.syntax.attrs);
        let parent = file
            .path
            .parent()
            .expect("discovered source paths have a parent");
        let stem = file
            .path
            .file_stem()
            .expect("Rust source files have a stem");
        let directory =
            if file.is_crate_root || matches!(stem.to_str(), Some("lib" | "main" | "mod")) {
                parent.to_owned()
            } else {
                parent.join(stem)
            };
        collect(
            &file.syntax.items,
            &directory,
            parent,
            false,
            index,
            &paths,
            &mut edges,
        );
    }
    loop {
        let mut changed = false;
        for index in 0..files.len() {
            if files[index].is_test || files[index].is_crate_root {
                continue;
            }
            let incoming: Vec<_> = edges.iter().filter(|edge| edge.target == index).collect();
            if !incoming.is_empty()
                && incoming
                    .iter()
                    .all(|edge| edge.test_only || files[edge.source].is_test)
            {
                files[index].is_test = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // A conventional tests/ location alone cannot exempt code that an ordinary
    // production module also imports. Explicit file-level cfg(test) still wins.
    loop {
        let mut changed = false;
        for edge in &edges {
            if !edge.test_only
                && !files[edge.source].is_test
                && files[edge.target].is_test
                && !context::test_attributes(&files[edge.target].syntax.attrs)
            {
                files[edge.target].is_test = false;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn collect(
    items: &[Item],
    directory: &Path,
    attribute_directory: &Path,
    test_only: bool,
    source: usize,
    paths: &BTreeMap<PathBuf, usize>,
    edges: &mut Vec<Edge>,
) {
    for item in items {
        let Item::Mod(module) = item else {
            continue;
        };
        let test_only = test_only || context::test_attributes(&module.attrs);
        let override_path = module.attrs.iter().find_map(|attr| {
            let Meta::NameValue(value) = &attr.meta else {
                return None;
            };
            if !value.path.is_ident("path") {
                return None;
            }
            let Expr::Lit(lit) = &value.value else {
                return None;
            };
            let Lit::Str(path) = &lit.lit else {
                return None;
            };
            Some(path.value())
        });
        if let Some((_, items)) = &module.content {
            let directory = override_path.map_or_else(
                || directory.join(module.ident.to_string()),
                |path| attribute_directory.join(path),
            );
            collect(
                items, &directory, &directory, test_only, source, paths, edges,
            );
        } else {
            let candidates = override_path.map_or_else(
                || {
                    vec![
                        directory.join(format!("{}.rs", module.ident)),
                        directory.join(module.ident.to_string()).join("mod.rs"),
                    ]
                },
                |path| vec![attribute_directory.join(path)],
            );
            for candidate in candidates {
                let Ok(path) = candidate.canonicalize() else {
                    continue;
                };
                let Some(&target) = paths.get(&path) else {
                    continue;
                };
                edges.push(Edge {
                    source,
                    target,
                    test_only,
                });
            }
        }
    }
}
