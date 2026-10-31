//! Written import trees shared by the grouping rule and fix validation.

use std::{mem, ops::Range};

use syn::{
    Block, File, Item, ItemMod, ItemUse, Stmt, UseTree,
    spanned::Spanned,
    visit::{self, Visit},
};

use super::{fix, source};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Import {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub glob: bool,
    pub type_only: bool,
    pub absolute: bool,
    pub visibility: Vec<String>,
}

pub fn flatten(item: &ItemUse, source: &str) -> Vec<Import> {
    fn tree(
        node: &UseTree,
        prefix: &mut Vec<String>,
        leaves: &mut Vec<(Vec<String>, Option<String>, bool, bool)>,
    ) {
        match node {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                tree(&path.tree, prefix, leaves);
                prefix.pop();
            }
            UseTree::Group(group) => {
                for child in &group.items {
                    tree(child, prefix, leaves);
                }
            }
            UseTree::Name(name) => {
                let mut path = prefix.clone();
                if name.ident != "self" || path.is_empty() {
                    path.push(name.ident.to_string());
                }
                leaves.push((
                    path,
                    None,
                    false,
                    name.ident == "self" && !prefix.is_empty(),
                ));
            }
            UseTree::Rename(rename) => {
                let mut path = prefix.clone();
                if rename.ident != "self" || path.is_empty() {
                    path.push(rename.ident.to_string());
                }
                leaves.push((
                    path,
                    Some(rename.rename.to_string()),
                    false,
                    rename.ident == "self" && !prefix.is_empty(),
                ));
            }
            UseTree::Glob(_) => leaves.push((prefix.clone(), None, true, false)),
        }
    }
    let mut leaves = Vec::new();
    tree(&item.tree, &mut Vec::new(), &mut leaves);
    let visibility = fix::tokens(&source[item.vis.span().byte_range()]);
    leaves
        .into_iter()
        .map(|(path, alias, glob, type_only)| Import {
            path,
            alias,
            glob,
            type_only,
            absolute: item.leading_colon.is_some(),
            visibility: visibility.clone(),
        })
        .collect()
}

/// Attributed imports and all non-use items/statements delimit lexical runs.
pub fn runs(file: &File) -> Vec<Vec<&ItemUse>> {
    #[derive(Default)]
    struct Runs<'a>(Vec<Vec<&'a ItemUse>>);

    impl<'a> Runs<'a> {
        fn collect(&mut self, items: impl IntoIterator<Item = Option<&'a ItemUse>>) {
            let mut run = Vec::new();
            for item in items {
                if let Some(item) = item.filter(|item| item.attrs.is_empty()) {
                    run.push(item);
                } else if !run.is_empty() {
                    self.0.push(mem::take(&mut run));
                }
            }
            if !run.is_empty() {
                self.0.push(run);
            }
        }
    }

    fn empty(tree: &UseTree) -> bool {
        match tree {
            UseTree::Group(group) => group.items.is_empty() || group.items.iter().any(empty),
            UseTree::Path(path) => empty(&path.tree),
            _ => false,
        }
    }

    fn as_use(item: &Item) -> Option<&ItemUse> {
        if let Item::Use(item) = item {
            if empty(&item.tree) {
                return None;
            }
            Some(item)
        } else {
            None
        }
    }

    impl<'a> Visit<'a> for Runs<'a> {
        fn visit_file(&mut self, file: &'a File) {
            self.collect(file.items.iter().map(as_use));
            visit::visit_file(self, file);
        }

        fn visit_item_mod(&mut self, module: &'a ItemMod) {
            if let Some((_, items)) = &module.content {
                self.collect(items.iter().map(as_use));
            }
            visit::visit_item_mod(self, module);
        }

        fn visit_block(&mut self, block: &'a Block) {
            self.collect(block.stmts.iter().map(|stmt| match stmt {
                Stmt::Item(item) => as_use(item),
                _ => None,
            }));
            visit::visit_block(self, block);
        }
    }
    let mut runs = Runs::default();
    runs.visit_file(file);
    runs.0.sort_by_key(|run| run[0].span().byte_range().start);
    runs.0
}

pub fn range(run: &[&ItemUse]) -> Range<usize> {
    run[0].span().byte_range().start..run.last().unwrap().span().byte_range().end
}

#[derive(Debug, Eq, PartialEq)]
pub enum Token {
    Written(String),
    Imports(Vec<Import>),
}

/// Replace each run with a multiset, retaining every surrounding token and scope.
pub fn normalized(source: &str) -> anyhow::Result<Vec<Token>> {
    let prepared = source::prepare(source);
    let file = syn::parse_file(&prepared)?;
    let mut result = Vec::new();
    let mut offset = 0;
    for run in runs(&file) {
        let range = range(&run);
        result.extend(
            fix::doc_words(&prepared[offset..range.start])
                .into_iter()
                .map(Token::Written),
        );
        let mut leaves: Vec<_> = run
            .iter()
            .flat_map(|item| flatten(item, &prepared))
            .collect();
        leaves.sort();
        result.push(Token::Imports(leaves));
        offset = range.end;
    }
    result.extend(
        fix::doc_words(&prepared[offset..])
            .into_iter()
            .map(Token::Written),
    );
    Ok(result)
}
