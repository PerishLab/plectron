use oxc_ast::{AstKind, ast::*};
use oxc_parser::Token;
use oxc_span::{GetSpan, Span};

use super::buffer::Buffer;

pub(super) fn retired(kind: AstKind<'_>) -> bool {
    match kind {
        AstKind::Class(value) => value.declare,
        AstKind::PropertyDefinition(value) => {
            value.declare || value.r#type == PropertyDefinitionType::TSAbstractPropertyDefinition
        }
        AstKind::MethodDefinition(value) => value.value.body.is_none(),
        AstKind::TSIndexSignature(_) => true,
        _ => false,
    }
}

pub(super) fn lower(kind: AstKind<'_>, buffer: &mut Buffer<'_>, tokens: &[Token]) {
    match kind {
        AstKind::Class(value) => class(value, buffer, tokens),
        AstKind::PropertyDefinition(value) => {
            modifiers(
                Span::new(value.span.start, value.key.span().start),
                buffer,
                tokens,
            );
            let end = value.type_annotation.as_ref().map_or_else(
                || {
                    value
                        .value
                        .as_ref()
                        .map_or(value.span.end, |value| value.span().start)
                },
                |annotation| annotation.span.start,
            );
            markers(Span::new(value.key.span().end, end), buffer, tokens);
        }
        AstKind::MethodDefinition(value) => {
            modifiers(
                Span::new(value.span.start, value.key.span().start),
                buffer,
                tokens,
            );
            if value.optional {
                markers(
                    Span::new(value.key.span().end, value.value.params.span.start),
                    buffer,
                    tokens,
                );
            }
        }
        _ => {}
    }
}

fn class(value: &Class<'_>, buffer: &mut Buffer<'_>, tokens: &[Token]) {
    if value.r#abstract
        && let Some(token) = tokens
            .iter()
            .find(|token| token.start() >= value.span.start)
        && buffer.original(token.span()) == "abstract"
    {
        buffer.erase(token.span());
    }
    if let Some(first) = value.implements.first()
        && let Some(keyword) = tokens
            .iter()
            .rev()
            .find(|token| token.end() <= first.span.start)
    {
        buffer.erase(Span::new(keyword.start(), value.body.span.start));
    }
}

fn modifiers(span: Span, buffer: &mut Buffer<'_>, tokens: &[Token]) {
    for token in tokens
        .iter()
        .filter(|token| span.start <= token.start() && token.end() <= span.end)
    {
        if matches!(
            buffer.original(token.span()),
            "public" | "private" | "protected" | "readonly" | "override" | "abstract"
        ) {
            buffer.erase(token.span());
        }
    }
}

fn markers(span: Span, buffer: &mut Buffer<'_>, tokens: &[Token]) {
    for token in tokens
        .iter()
        .filter(|token| span.start <= token.start() && token.end() <= span.end)
    {
        if matches!(buffer.original(token.span()), "?" | "!") {
            buffer.erase(token.span());
        }
    }
}
