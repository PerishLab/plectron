use crate::{
    Fault,
    style::{
        Typography,
        typography::{Whitespace, Words},
    },
};
use textengine::breaking;

pub(super) fn opportunities(
    ranges: &[std::ops::Range<usize>],
    source: &str,
    style: &Typography,
) -> Result<Vec<bool>, Fault> {
    let points = breaking::opportunities(source)?;
    let mut opportunities = Vec::with_capacity(ranges.len());
    let mut previous = None;
    for range in ranges {
        let current = source[range.clone()].chars().next().unwrap();
        let mut allowed = points
            .binary_search_by_key(&range.start, |point| point.offset)
            .is_ok();
        if style.words == Words::Keep
            && previous.is_some_and(breaking::word)
            && breaking::word(current)
        {
            allowed = false;
        }
        if style.whitespace == Whitespace::Spaces
            && previous.is_some_and(|character| {
                matches!(character, ' ' | '\t') || breaking::separator(character)
            })
        {
            allowed = true;
        }
        opportunities.push(allowed);
        previous = Some(current);
    }
    Ok(opportunities)
}

pub(super) fn normalize(source: &str, whitespace: Whitespace) -> String {
    source
        .chars()
        .map(|character| {
            if whitespace.collapse() && matches!(character, ' ' | '\t' | '\r' | '\n' | '\u{000c}') {
                ' '
            } else {
                character
            }
        })
        .collect()
}
