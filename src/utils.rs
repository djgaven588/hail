pub(crate) fn take_while(accept: impl Fn(char) -> bool, s: &str) -> (&str, &str) {
    let extracted_end = s
        .char_indices()
        .find_map(|(idx, c)| if accept(c) { None } else { Some(idx) })
        .unwrap_or(s.len());

    let extracted = &s[..extracted_end];
    let remainder = &s[extracted_end..];
    (remainder, extracted)
}

pub(crate) fn extract_digits(s: &str) -> (&str, &str) {
    take_while(|char| char.is_ascii_digit(), s)
}

pub(crate) fn extract_whitespace(s: &str) -> (&str, &str) {
    take_while(|char| char.is_whitespace(), s)
}

pub(crate) fn extract_op(s: &str) -> (&str, &str) {
    match &s[0..1] {
        "+" | "-" | "*" | "/" => {}
        _ => panic!("bad operator"),
    }

    (&s[1..], &s[0..1])
}

mod tests {
    use super::*;

    #[test]
    fn do_not_extract_anything_from_empty_input() {
        assert_eq!(extract_digits(""), ("", ""));
    }

    #[test]
    fn extract_digits_with_no_remainder() {
        assert_eq!(extract_digits("100"), ("", "100"));
    }

    #[test]
    fn extract_spaces() {
        assert_eq!(extract_whitespace("   2"), ("2", "   "));
    }

    #[test]
    fn extract_tabs() {
        assert_eq!(extract_whitespace("		2"), ("2", "		"));
    }
}
