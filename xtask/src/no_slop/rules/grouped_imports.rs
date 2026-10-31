//! Group written imports without resolving names or entering macro bodies.

use syn::{File, ItemUse, UseTree, spanned::Spanned};

use super::super::{
    context::Context,
    diagnostic::Diagnostic,
    fix,
    imports::{self, Import},
};

#[derive(Default)]
struct Node {
    name: String,
    aliases: Vec<(Option<String>, bool)>,
    children: Vec<Self>,
}

impl Node {
    fn insert(&mut self, path: &[String], alias: Option<String>, type_only: bool) {
        let Some((name, rest)) = path.split_first() else {
            self.aliases.push((alias, type_only));
            return;
        };
        let index = self
            .children
            .iter()
            .position(|node| &node.name == name)
            .unwrap_or_else(|| {
                self.children.push(Self {
                    name: name.clone(),
                    ..Self::default()
                });
                self.children.len() - 1
            });
        self.children[index].insert(rest, alias, type_only);
    }

    fn render(&self) -> Vec<String> {
        let named = |name: &str, alias: &Option<String>| {
            alias
                .as_ref()
                .map_or_else(|| name.to_owned(), |alias| format!("{name} as {alias}"))
        };
        if self.children.is_empty() {
            return self
                .aliases
                .iter()
                .map(|(alias, type_only)| {
                    named(
                        &format!("{}{}", self.name, if *type_only { "::self" } else { "" }),
                        alias,
                    )
                })
                .collect();
        }
        let mut children: Vec<_> = self.children.iter().flat_map(Self::render).collect();
        // Preserve explicit self imports: they bind only the type namespace.
        // Plain terminal imports must remain separate from their child paths.
        let mut separate = Vec::new();
        let self_count = self.aliases.iter().filter(|(_, is_self)| *is_self).count();
        for (alias, is_self) in &self.aliases {
            if *is_self && self_count == 1 {
                children.insert(0, named("self", alias));
            } else {
                let name = format!("{}{}", self.name, if *is_self { "::self" } else { "" });
                separate.push(named(&name, alias));
            }
        }
        let tail = if children.len() == 1 {
            children.pop().unwrap()
        } else {
            format!("{{{}}}", children.join(", "))
        };
        separate.push(format!("{}::{tail}", self.name));
        separate
    }
}

struct Group {
    root: String,
    absolute: bool,
    visibility: Vec<String>,
    tree: Node,
}

fn category(root: &str) -> u8 {
    match root {
        "std" | "core" | "alloc" => 0,
        "crate" | "self" | "super" => 2,
        _ => 1,
    }
}

fn grouped(leaves: impl Iterator<Item = Import>) -> Vec<(u8, String)> {
    let mut groups: Vec<Group> = Vec::new();
    for leaf in leaves {
        let mut path = leaf.path;
        if leaf.glob {
            path.push("*".to_owned());
        }
        let root = path[0].clone();
        let index = groups
            .iter()
            .position(|group| {
                group.root == root
                    && group.absolute == leaf.absolute
                    && group.visibility == leaf.visibility
            })
            .unwrap_or_else(|| {
                groups.push(Group {
                    root: root.clone(),
                    absolute: leaf.absolute,
                    visibility: leaf.visibility,
                    tree: Node {
                        name: root,
                        ..Node::default()
                    },
                });
                groups.len() - 1
            });
        groups[index]
            .tree
            .insert(&path[1..], leaf.alias, leaf.type_only);
    }
    groups.sort_by_key(|group| category(&group.root));
    groups
        .into_iter()
        .map(|group| {
            let visibility = if group.visibility.is_empty() {
                String::new()
            } else {
                format!("{} ", group.visibility.join(" "))
            };
            let tree = group.tree.render();
            let tree = if tree.len() == 1 {
                tree[0].clone()
            } else {
                format!("{{{}}}", tree.join(", "))
            };
            (
                category(&group.root),
                format!(
                    "{visibility}use {}{tree};",
                    if group.absolute { "::" } else { "" }
                ),
            )
        })
        .collect()
}

/// Ignore sibling ordering, unary braces, whitespace, and wrapping.
fn shape(tree: &UseTree) -> String {
    match tree {
        UseTree::Path(path) => format!("{}::{}", path.ident, shape(&path.tree)),
        UseTree::Name(name) => name.ident.to_string(),
        UseTree::Rename(rename) => format!("{} as {}", rename.ident, rename.rename),
        UseTree::Glob(_) => "*".to_owned(),
        UseTree::Group(group) => {
            let mut items: Vec<_> = group.items.iter().map(shape).collect();
            items.sort();
            if items.len() == 1 {
                items.pop().unwrap()
            } else {
                format!("{{{}}}", items.join(","))
            }
        }
    }
}

fn signature(item: &ItemUse, source: &str) -> (Vec<String>, bool, String) {
    (
        fix::tokens(&source[item.vis.span().byte_range()]),
        item.leading_colon.is_some(),
        shape(&item.tree),
    )
}

fn protected(item: &ItemUse, context: &Context<'_>) -> bool {
    let range = item.span().byte_range();
    context.comments.iter().any(|comment| {
        let before = comment.range.end <= range.start
            && context.source[comment.range.end..range.start]
                .trim()
                .is_empty()
            && !comment.text.starts_with("//!")
            && !comment.text.starts_with("/*!");
        let inside = comment.range.start < range.end && comment.range.end > range.start;
        let trailing = comment.range.start >= range.end
            && !context.source[range.end..comment.range.start].contains('\n')
            && context.source[range.end..comment.range.start]
                .trim()
                .is_empty();
        before || inside || trailing
    })
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    let mut findings = Vec::new();
    for run in imports::runs(file) {
        for run in run
            .split(|item| protected(item, context))
            .filter(|run| !run.is_empty())
        {
            let groups = grouped(
                run.iter()
                    .flat_map(|item| imports::flatten(item, context.source)),
            );
            let range = imports::range(run);
            let newline = fix::newline(context.source, range.start);
            let line_start = context.source[..range.start]
                .rfind('\n')
                .map_or(0, |i| i + 1);
            let indent: String = context.source[line_start..range.start]
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            let mut replacement = String::new();
            let mut previous = None;
            for (category, text) in groups {
                if let Some(previous) = previous {
                    replacement.push_str(newline);
                    if previous != category {
                        replacement.push_str(newline);
                    }
                    replacement.push_str(&indent);
                }
                replacement.push_str(&text);
                previous = Some(category);
            }
            let expected = syn::parse_file(&replacement).expect("rendered use trees parse");
            let expected: Vec<_> = expected
                .items
                .iter()
                .filter_map(|item| {
                    if let syn::Item::Use(item) = item {
                        Some(signature(item, &replacement))
                    } else {
                        None
                    }
                })
                .collect();
            let actual: Vec<_> = run
                .iter()
                .map(|item| signature(item, context.source))
                .collect();
            let separated = run.windows(2).all(|pair| {
                let left = imports::flatten(pair[0], context.source);
                let right = imports::flatten(pair[1], context.source);
                let categories = |leaves: &[Import]| {
                    leaves
                        .first()
                        .map(|leaf| category(leaf.path.first().map_or("*", String::as_str)))
                };
                categories(&left) == categories(&right)
                    || context.source
                        [pair[0].span().byte_range().end..pair[1].span().byte_range().start]
                        .split_inclusive('\n')
                        .enumerate()
                        .any(|(index, line)| {
                            index > 0 && line.ends_with('\n') && line.trim().is_empty()
                        })
            });
            if actual != expected || !separated {
                findings.push(context.report_range("require-grouped-imports", range.clone(), "group imports by written root and separate standard, external, and local roots", "Merge shared import prefixes using braces and separate root categories with a blank line.").with_fix(range.start, range.end, replacement));
            }
        }
    }
    findings
}
