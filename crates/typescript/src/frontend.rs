use oxc_allocator::Allocator;
use oxc_parser::{Parser, Token, config::TokensParserConfig};
use oxc_semantic::{Semantic, SemanticBuilder};
use oxc_span::SourceType;
use project::{Diagnostic, Kind, Source, Span};

pub(super) fn read<T>(
    source: &Source,
    action: impl FnOnce(&Semantic<'_>, &[Token]) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    if source.kind != Kind::Ts {
        return Err(Diagnostic::new(
            source.module.clone(),
            Span::default(),
            "expected TypeScript source",
        ));
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &source.text, SourceType::ts())
        .with_config(TokensParserConfig)
        .parse();
    super::diagnostic::check(source, &parsed.diagnostics)?;
    if parsed.fatal_error {
        return Err(Diagnostic::new(
            source.module.clone(),
            Span::default(),
            "TypeScript parser aborted",
        ));
    }
    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .with_check_syntax_error(true)
        .build(&parsed.program);
    super::diagnostic::check(source, &semantic.diagnostics)?;
    super::admission::check(source, &semantic.semantic)?;
    action(&semantic.semantic, &parsed.tokens)
}
