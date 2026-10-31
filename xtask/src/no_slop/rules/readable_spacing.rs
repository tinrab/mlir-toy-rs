//! Require a blank physical line between sibling item declarations.
//! Imports, external modules, and single-line constants may form compact groups.
//! A leading comment starts a new group; same-line trailing comments belong to the prior item.
//! Fields, enum variants, ordinary statements, and macro bodies are outside this rule.
//! Spacing inside an item's attributes and documentation is untouched.

use proc_macro2::Span;
use syn::{
    Attribute, Block, File, ForeignItem, ImplItem, Item, ItemForeignMod, ItemImpl, ItemMod,
    ItemTrait, Stmt, TraitItem,
    spanned::Spanned,
    visit::{self, Visit},
};

use super::super::{
    context::Context,
    diagnostic::Diagnostic,
    fix,
    source::{self, Comment},
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Group {
    Imports,
    Modules,
    Constants,
}

struct Check<'a> {
    context: &'a Context<'a>,
    comments: Vec<Comment<'a>>,
    findings: Vec<Diagnostic>,
}

impl Check<'_> {
    fn pairs<T: Spanned>(&mut self, items: &[T], group: fn(&T) -> Option<Group>) {
        for pair in items.windows(2) {
            self.pair(&pair[0], &pair[1], group);
        }
    }

    fn pair<T: Spanned>(&mut self, previous: &T, current: &T, group: fn(&T) -> Option<Group>) {
        let previous_span = previous.span();
        let current_span = current.span();
        let end = previous_span.byte_range().end;
        let start = current_span.byte_range().start;
        let comments: Vec<_> = self
            .comments
            .iter()
            .filter(|comment| comment.range.start >= end && comment.range.end <= start)
            .collect();
        let leading = comments
            .iter()
            .find(|comment| self.context.source[end..comment.range.start].contains('\n'));
        let grouped = group(previous).is_some() && group(previous) == group(current);
        if grouped && leading.is_none() {
            return;
        }

        let boundary = leading.map_or(start, |comment| comment.range.start);
        let mut offset = end;
        for (index, line) in self.context.source[end..boundary]
            .split_inclusive('\n')
            .enumerate()
        {
            let line_end = offset + line.len();
            let inside_comment = comments
                .iter()
                .any(|comment| comment.range.start < line_end && comment.range.end > offset);
            if index > 0 && line.ends_with('\n') && line.trim().is_empty() && !inside_comment {
                return;
            }
            offset = line_end;
        }
        let line_start = self.context.source[..boundary]
            .rfind('\n')
            .map_or(0, |p| p + 1);
        let newline = fix::newline(self.context.source, boundary);
        let (fix_start, fix_end, replacement) = if line_start >= end
            && self.context.source[line_start..boundary]
                .chars()
                .all(char::is_whitespace)
        {
            (line_start, line_start, newline.to_owned())
        } else {
            let prefix = &self.context.source[end..boundary];
            let start = end + prefix.trim_end_matches([' ', '\t']).len();
            (start, boundary, newline.repeat(2))
        };
        self.findings.push(self.context.report(
            "require-readable-spacing", current_span, "missing blank line between declarations",
            "Insert a blank line before this declaration and its attached documentation, attributes, or leading comments.",
        ).with_fix(fix_start, fix_end, replacement));
    }
}

impl<'ast> Visit<'ast> for Check<'_> {
    fn visit_file(&mut self, file: &'ast File) {
        self.pairs(&file.items, item_group);
        visit::visit_file(self, file);
    }

    fn visit_item_mod(&mut self, module: &'ast ItemMod) {
        if let Some((_, items)) = &module.content {
            self.pairs(items, item_group);
        }
        visit::visit_item_mod(self, module);
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        self.pairs(&item.items, |item| match item {
            ImplItem::Const(item) => compact_group(item.span(), &item.attrs, Group::Constants),
            _ => None,
        });
        visit::visit_item_impl(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast ItemTrait) {
        self.pairs(&item.items, |item| match item {
            TraitItem::Const(item) => compact_group(item.span(), &item.attrs, Group::Constants),
            _ => None,
        });
        visit::visit_item_trait(self, item);
    }

    fn visit_item_foreign_mod(&mut self, item: &'ast ItemForeignMod) {
        self.pairs(&item.items, |item| match item {
            ForeignItem::Static(item) => compact_group(item.span(), &item.attrs, Group::Constants),
            _ => None,
        });
        visit::visit_item_foreign_mod(self, item);
    }

    fn visit_block(&mut self, block: &'ast Block) {
        for pair in block.stmts.windows(2) {
            if let (Stmt::Item(previous), Stmt::Item(current)) = (&pair[0], &pair[1]) {
                self.pair(previous, current, item_group);
            }
        }
        visit::visit_block(self, block);
    }
}

pub fn check(file: &File, context: &Context<'_>) -> Vec<Diagnostic> {
    let mut check = Check {
        context,
        comments: source::comments(context.source),
        findings: Vec::new(),
    };
    check.visit_file(file);
    check.findings
}

fn item_group(item: &Item) -> Option<Group> {
    match item {
        Item::Use(item) => compact_group(item.span(), &item.attrs, Group::Imports),
        Item::ExternCrate(item) => compact_group(item.span(), &item.attrs, Group::Imports),
        Item::Mod(item) if item.content.is_none() => {
            compact_group(item.span(), &item.attrs, Group::Modules)
        }
        Item::Const(item) => compact_group(item.span(), &item.attrs, Group::Constants),
        Item::Static(item) => compact_group(item.span(), &item.attrs, Group::Constants),
        _ => None,
    }
}

fn compact_group(span: Span, attrs: &[Attribute], group: Group) -> Option<Group> {
    if attrs.iter().any(|attr| attr.path().is_ident("doc")) {
        return None;
    }
    if group == Group::Constants && span.start().line != span.end().line {
        return None;
    }
    Some(group)
}
