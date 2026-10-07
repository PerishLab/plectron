use oxc_ast::{AstKind, ast::*};
use oxc_parser::Token;
use oxc_span::Span;

use super::buffer::Buffer;

pub(super) fn retired(kind: AstKind<'_>) -> bool {
    match kind {
        AstKind::ImportDeclaration(value) => value.import_kind == ImportOrExportKind::Type,
        AstKind::ExportNamedDeclaration(value) => value.export_kind == ImportOrExportKind::Type,
        AstKind::ExportFromDeclaration(value) => value.export_kind == ImportOrExportKind::Type,
        AstKind::ExportAllDeclaration(value) => value.export_kind == ImportOrExportKind::Type,
        AstKind::ExportDeclaration(value) => declaration(&value.declaration),
        AstKind::ExportDefaultDeclaration(value) => match &value.declaration {
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => true,
            ExportDefaultDeclarationKind::ClassDeclaration(value) => value.declare,
            ExportDefaultDeclarationKind::FunctionDeclaration(value) => value.body.is_none(),
            _ => false,
        },
        _ => false,
    }
}

fn declaration(value: &Declaration<'_>) -> bool {
    match value {
        Declaration::TSTypeAliasDeclaration(_) | Declaration::TSInterfaceDeclaration(_) => true,
        Declaration::ClassDeclaration(value) => value.declare,
        Declaration::FunctionDeclaration(value) => value.declare || value.body.is_none(),
        Declaration::VariableDeclaration(value) => value.declare,
        _ => false,
    }
}

pub(super) fn lower(kind: AstKind<'_>, buffer: &mut Buffer<'_>, tokens: &[Token]) {
    let entries = match kind {
        AstKind::ImportDeclaration(value) => imports(value),
        AstKind::ExportNamedDeclaration(value) => exports(&value.specifiers),
        AstKind::ExportFromDeclaration(value) => exports(&value.specifiers),
        _ => return,
    };
    list(&entries, buffer, tokens);
}

fn imports(value: &ImportDeclaration<'_>) -> Vec<(Span, bool)> {
    value
        .specifiers
        .as_ref()
        .map_or_else(Vec::new, |items| items.iter().filter_map(entry).collect())
}

fn entry(item: &ImportDeclarationSpecifier<'_>) -> Option<(Span, bool)> {
    match item {
        ImportDeclarationSpecifier::ImportSpecifier(value) => {
            Some((value.span, value.import_kind == ImportOrExportKind::Type))
        }
        _ => None,
    }
}

fn exports(items: &[ExportSpecifier<'_>]) -> Vec<(Span, bool)> {
    items
        .iter()
        .map(|value| (value.span, value.export_kind == ImportOrExportKind::Type))
        .collect()
}

fn list(entries: &[(Span, bool)], buffer: &mut Buffer<'_>, tokens: &[Token]) {
    if !entries.iter().any(|(_, removed)| *removed) {
        return;
    }
    let first = entries.first().unwrap().0;
    let last = entries.last().unwrap().0;
    let closing = tokens
        .iter()
        .find(|token| token.start() >= last.end && buffer.original(token.span()) == "}")
        .unwrap();
    let kept = entries.iter().rposition(|(_, removed)| !removed);
    for (span, removed) in entries {
        if *removed {
            buffer.erase(*span);
        }
    }
    for token in tokens
        .iter()
        .filter(|token| first.start <= token.start() && token.end() <= closing.start())
    {
        if buffer.original(token.span()) == "," && !separator(entries, token.start(), kept) {
            buffer.erase(token.span());
        }
    }
}

fn separator(entries: &[(Span, bool)], position: u32, kept: Option<usize>) -> bool {
    entries
        .iter()
        .rposition(|(span, _)| span.end <= position)
        .is_some_and(|index| !entries[index].1 && kept.is_some_and(|last| index < last))
}
