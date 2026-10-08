use project::{Artifact, Diagnostic, Kind, Mapping, Origin, Source, Span};

pub struct Typescript;

impl project::Adapter for Typescript {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        let lowered = crate::erase(&source)?;
        let origins = origins(&source, &lowered);
        Ok(Artifact {
            module: source.module,
            kind: Kind::Js,
            text: lowered.text,
            origins,
        })
    }
}

fn origins(source: &Source, lowered: &crate::Lowered) -> Vec<Origin> {
    let repairs: std::collections::BTreeMap<_, _> = lowered
        .repairs
        .iter()
        .map(|repair| (repair.generated.start, repair))
        .collect();
    let mut cuts: Vec<_> = source
        .text
        .char_indices()
        .map(|(offset, _)| offset)
        .collect();
    cuts.push(source.text.len());
    for repair in repairs.values() {
        cuts.extend([repair.generated.start, repair.generated.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut origins = Vec::new();
    for pair in cuts.windows(2) {
        let generated = Span {
            start: pair[0],
            end: pair[1],
        };
        let (mapping, original) = match repairs.get(&generated.start) {
            Some(repair) => (Mapping::Range, repair.source),
            None => classify(source, lowered, generated),
        };
        append(
            &mut origins,
            Origin {
                mapping,
                generated,
                module: source.module.clone(),
                source: original,
            },
        );
    }
    origins
}

fn classify(source: &Source, lowered: &crate::Lowered, generated: Span) -> (Mapping, Span) {
    let original = source.text.get(generated.start..generated.end);
    if original == lowered.text.get(generated.start..generated.end) {
        return (Mapping::Exact, generated);
    }
    let mut original = generated;
    while !source.text.is_char_boundary(original.start) {
        original.start -= 1;
    }
    while !source.text.is_char_boundary(original.end) {
        original.end += 1;
    }
    (Mapping::Range, original)
}

fn append(origins: &mut Vec<Origin>, origin: Origin) {
    let adjoining = origins.last().is_some_and(|previous| {
        previous.generated.end == origin.generated.start
            && previous.source.end == origin.source.start
    });
    if let Some(previous) = origins.last_mut()
        && previous.mapping == origin.mapping
        && adjoining
    {
        previous.generated.end = origin.generated.end;
        previous.source.end = origin.source.end;
        return;
    }
    origins.push(origin);
}
