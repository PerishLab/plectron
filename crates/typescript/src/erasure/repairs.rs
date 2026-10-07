use oxc_ast::{AstKind, ast::ArrowFunctionExpression};
use oxc_parser::Token;
use oxc_semantic::Semantic;
use oxc_span::{GetSpan, Span};

use super::buffer::Buffer;

pub(super) fn apply(semantic: &Semantic<'_>, tokens: &[Token], buffer: &mut Buffer<'_>) {
    for node in semantic.nodes().iter() {
        if buffer.retired(node.span()) {
            continue;
        }
        match node.kind() {
            AstKind::ArrowFunctionExpression(value) => arrow(value, tokens, buffer),
            AstKind::ExpressionStatement(_)
            | AstKind::ReturnStatement(_)
            | AstKind::ThrowStatement(_)
            | AstKind::ExportDefaultDeclaration(_)
            | AstKind::PropertyDefinition(_) => terminate(node.span(), tokens, buffer),
            AstKind::VariableDeclaration(_)
                if !matches!(
                    semantic.nodes().parent_kind(node.id()),
                    AstKind::ForStatement(_)
                        | AstKind::ForInStatement(_)
                        | AstKind::ForOfStatement(_)
                ) =>
            {
                terminate(node.span(), tokens, buffer)
            }
            _ => {}
        }
    }
}

fn arrow(value: &ArrowFunctionExpression<'_>, tokens: &[Token], buffer: &mut Buffer<'_>) {
    let Some(annotation) = &value.return_type else {
        return;
    };
    let Some(closing) = tokens
        .iter()
        .rev()
        .find(|token| token.end() <= annotation.span.start)
    else {
        return;
    };
    if buffer.original(closing.span()) != ")" {
        return;
    }
    let gap = Span::new(closing.end(), annotation.span.end);
    if buffer
        .original(gap)
        .contains(['\r', '\n', '\u{2028}', '\u{2029}'])
    {
        buffer.erase(closing.span());
        buffer.write(annotation.span.end - 1, b')', closing.span());
    }
}

fn terminate(span: Span, tokens: &[Token], buffer: &mut Buffer<'_>) {
    let Some(last) = tokens.iter().rev().find(|token| token.end() <= span.end) else {
        return;
    };
    if last.start() >= span.start && buffer.removed(last.span()) {
        buffer.write(last.end() - 1, b';', span);
    }
}
