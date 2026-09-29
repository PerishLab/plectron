const ELEMENTS: &[&str] = &[
    "html", "head", "body", "title", "link", "script", "style", "div", "p", "span", "strong", "a",
    "button", "input", "img", "h1", "h2",
];

pub(super) fn supported(name: &str) -> bool {
    if ELEMENTS.contains(&name) {
        return true;
    }
    if !name.contains('-') || !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
