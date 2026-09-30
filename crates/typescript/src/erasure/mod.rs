mod buffer;
mod members;
mod modules;
mod repairs;

use oxc_ast::AstKind;
use oxc_parser::{Parser, Token};
use oxc_semantic::Semantic;
use oxc_span::{GetSpan, SourceType, Span};
use project::{Diagnostic, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lowered {
    pub text: String,
    pub repairs: Vec<Repair>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repair {
    pub generated: project::Span,
    pub source: project::Span,
}

pub(super) fn build(
    source: &Source,
    semantic: &Semantic<'_>,
    tokens: &[Token],
) -> Result<Lowered, Diagnostic> {
    let mut buffer = buffer::Buffer::new(&source.text);
    for node in semantic.nodes().iter() {
        if !buffer.retired(node.span()) && retired(node.kind()) {
            buffer.retire(node.span());
        }
    }
    for node in semantic.nodes().iter() {
        if buffer.retired(node.span()) {
            continue;
        }
        lower(node.kind(), &mut buffer, tokens);
    }
    repairs::apply(semantic, tokens, &mut buffer);
    let lowered = buffer.finish();
    validate(source, &lowered)?;
    Ok(lowered)
}

fn retired(kind: AstKind<'_>) -> bool {
    match kind {
        AstKind::TSTypeAliasDeclaration(_) | AstKind::TSInterfaceDeclaration(_) => true,
        AstKind::Function(value) => value.declare || value.body.is_none(),
        AstKind::VariableDeclaration(value) => value.declare,
        _ => members::retired(kind) || modules::retired(kind),
    }
}

fn lower(kind: AstKind<'_>, buffer: &mut buffer::Buffer, tokens: &[Token]) {
    match kind {
        AstKind::TSTypeAnnotation(value) => buffer.erase(value.span),
        AstKind::TSTypeParameterDeclaration(value) => buffer.erase(value.span),
        AstKind::TSTypeParameterInstantiation(value) => buffer.erase(value.span),
        AstKind::TSAsExpression(value) => {
            buffer.erase(Span::new(value.expression.span().end, value.span.end));
        }
        AstKind::TSSatisfiesExpression(value) => {
            buffer.erase(Span::new(value.expression.span().end, value.span.end));
        }
        AstKind::TSNonNullExpression(value) => {
            buffer.erase(Span::new(value.expression.span().end, value.span.end));
        }
        AstKind::FormalParameter(value) if value.optional => {
            punctuation(
                Span::new(value.pattern.span().end, value.span.end),
                "?",
                buffer,
                tokens,
            );
        }
        AstKind::VariableDeclarator(value) if value.definite => {
            punctuation(
                Span::new(value.id.span().end, value.span.end),
                "!",
                buffer,
                tokens,
            );
        }
        AstKind::TSThisParameter(value) => {
            buffer.erase(value.span);
            if let Some(token) = tokens.iter().find(|token| token.start() >= value.span.end)
                && buffer.original(token.span()) == ","
            {
                buffer.erase(token.span());
            }
        }
        _ => {
            members::lower(kind, buffer, tokens);
            modules::lower(kind, buffer, tokens);
        }
    }
}

fn punctuation(span: Span, expected: &str, buffer: &mut buffer::Buffer, tokens: &[Token]) {
    if let Some(token) = tokens.iter().find(|token| token.start() >= span.start)
        && token.end() <= span.end
        && buffer.original(token.span()) == expected
    {
        buffer.erase(token.span());
    }
}

fn validate(source: &Source, lowered: &Lowered) -> Result<(), Diagnostic> {
    let arena = oxc_allocator::Allocator::default();
    let parsed = Parser::new(&arena, &lowered.text, SourceType::mjs()).parse();
    super::diagnostic::check(source, &parsed.diagnostics)
        .map_err(|error| rejection(source, lowered, error))?;
    if parsed.fatal_error {
        return Err(Diagnostic::new(
            source.module.clone(),
            project::Span::default(),
            "unverified lowering: JavaScript parser aborted",
        ));
    }
    let semantic = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    super::diagnostic::check(source, &semantic.diagnostics)
        .map_err(|error| rejection(source, lowered, error))
}

fn rejection(source: &Source, lowered: &Lowered, mut error: Diagnostic) -> Diagnostic {
    if let Some(repair) = lowered.repairs.iter().find(|repair| {
        repair.generated.start <= error.span.start && error.span.end <= repair.generated.end
    }) {
        error.span = repair.source;
    }
    while !source.text.is_char_boundary(error.span.start) {
        error.span.start -= 1;
    }
    while !source.text.is_char_boundary(error.span.end) {
        error.span.end += 1;
    }
    error.message = format!("unverified lowering: {}", error.message);
    error
}
