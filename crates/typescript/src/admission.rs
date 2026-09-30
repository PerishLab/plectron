use oxc_ast::AstKind;
use oxc_semantic::Semantic;
use oxc_span::GetSpan;
use project::{Diagnostic, Source};

pub(super) fn check(source: &Source, semantic: &Semantic<'_>) -> Result<(), Diagnostic> {
    let failure = semantic
        .nodes()
        .iter()
        .filter_map(|node| {
            let kind = node.kind();
            excluded(kind).map(|message| (kind.span(), message))
        })
        .min_by_key(|(span, _)| span.start);
    match failure {
        Some((span, message)) => Err(Diagnostic::new(
            source.module.clone(),
            super::diagnostic::span(span),
            message,
        )),
        None => Ok(()),
    }
}

fn excluded(kind: AstKind<'_>) -> Option<&'static str> {
    Some(match kind {
        AstKind::TSEnumDeclaration(_) => "enum is excluded from the erasable profile",
        AstKind::TSNamespaceDeclaration(_)
        | AstKind::TSExternalModuleDeclaration(_)
        | AstKind::TSGlobalDeclaration(_)
        | AstKind::TSNamespaceExportDeclaration(_) => {
            "namespace and module declarations are excluded from the erasable profile"
        }
        AstKind::TSImportEqualsDeclaration(_) | AstKind::TSExportAssignment(_) => {
            "import/export assignments are excluded from the erasable profile"
        }
        AstKind::TSTypeAssertion(_) => "prefix type assertions are excluded; use as",
        AstKind::Decorator(_) => "decorators are excluded from the erasable profile",
        AstKind::JSXElement(_) | AstKind::JSXFragment(_) => {
            "TSX is excluded from the erasable profile"
        }
        AstKind::FormalParameter(parameter)
            if parameter.accessibility.is_some() || parameter.readonly || parameter.r#override =>
        {
            "parameter properties are excluded from the erasable profile"
        }
        AstKind::AccessorProperty(_) => {
            "auto-accessor lowering is not a verified erasable capability"
        }
        _ => return None,
    })
}
