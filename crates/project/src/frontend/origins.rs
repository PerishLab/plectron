use crate::{Mapping, Origin, Span};

pub(crate) fn locate(origins: &[Origin], span: Span) -> Option<Origin> {
    if span.start > span.end {
        return None;
    }
    let mut pieces = origins.iter().filter(|origin| {
        let range = origin.generated.start < span.end && span.start < origin.generated.end;
        let point = span.start == span.end;
        let inside = origin.generated.start <= span.start && span.start < origin.generated.end;
        range || point && inside
    });
    let first = pieces.next()?;
    if first.generated.start > span.start {
        return None;
    }
    let mut mapped = first.project(Span {
        start: span.start,
        end: span.end.min(first.generated.end),
    });
    for piece in pieces {
        if mapped.generated.end != piece.generated.start || mapped.module != piece.module {
            return None;
        }
        let next = piece.project(Span {
            start: piece.generated.start,
            end: span.end.min(piece.generated.end),
        });
        mapped.mapping = Mapping::Range;
        mapped.generated.end = next.generated.end;
        mapped.source.start = mapped.source.start.min(next.source.start);
        mapped.source.end = mapped.source.end.max(next.source.end);
    }
    (mapped.generated.end == span.end).then_some(mapped)
}
