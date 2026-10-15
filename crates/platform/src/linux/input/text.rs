pub(super) fn text(keysym: u32) -> Option<String> {
    let scalar = match keysym {
        0x20..=0x7e | 0xa0..=0xff => keysym,
        0xffaa..=0xffb9 => keysym - 0xff80,
        0x0100_0100..=0x0110_ffff => keysym - 0x0100_0000,
        _ => return None,
    };
    let value = char::from_u32(scalar)?;
    if value.is_control() {
        return None;
    }
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scalar() {
        assert_eq!(text(0x61).as_deref(), Some("a"));
        assert_eq!(text(0x41).as_deref(), Some("A"));
        assert_eq!(text(0xe9).as_deref(), Some("é"));
        assert_eq!(text(0xffb5).as_deref(), Some("5"));
        assert_eq!(text(0x0100_4e2d).as_deref(), Some("中"));
        assert_eq!(text(0x0101_f642).as_deref(), Some("🙂"));
        assert_eq!(text(0x0100_d800), None);
        assert_eq!(text(0x0100_0041), None);
        assert_eq!(text(0x0100_0085), None);
        assert_eq!(text(0x0111_0000), None);
        assert_eq!(text(0xff0d), None);
        assert_eq!(text(0xfe51), None);
        assert_eq!(text(0x01a1), None);
    }
}
