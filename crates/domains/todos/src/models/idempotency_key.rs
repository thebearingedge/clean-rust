//! Client-supplied idempotency key identifying a create request across
//! retries.

/// The shortest valid key: one visible ASCII character.
pub const MIN_LENGTH: usize = 1;

/// The longest valid key: 64 visible ASCII characters.
pub const MAX_LENGTH: usize = 64;

/// A regex admitting exactly the keys [`IdempotencyKey::parse`] accepts: one
/// or more visible ASCII characters (the `!`–`~` range that
/// `u8::is_ascii_graphic` covers).
pub const VISIBLE_ASCII_PATTERN: &str = "^[!-~]+$";

/// A client-supplied idempotency key: visible ASCII, 1 to 64 characters.
///
/// Construct via [`IdempotencyKey::parse`]; invalid input is rejected at the
/// boundary rather than stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Parses a raw key: visible ASCII, 1 to 64 characters. Returns
    /// [`Option::None`] when the input is shorter than [`MIN_LENGTH`],
    /// longer than [`MAX_LENGTH`], or contains characters outside the
    /// [`VISIBLE_ASCII_PATTERN`] range.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let valid = (MIN_LENGTH..=MAX_LENGTH).contains(&raw.len())
            && raw.bytes().all(|b| b.is_ascii_graphic());
        valid.then(|| Self(raw.to_owned()))
    }

    /// The key as it was supplied by the client.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_single_visible_character() {
        assert_eq!(IdempotencyKey::parse("a"), Some(IdempotencyKey("a".into())));
    }

    #[test]
    fn accepts_exactly_64_characters() {
        let raw = "k".repeat(64);
        assert_eq!(IdempotencyKey::parse(&raw), Some(IdempotencyKey(raw)),);
    }

    #[test]
    fn rejects_an_empty_key() {
        assert_eq!(IdempotencyKey::parse(""), None);
    }

    #[test]
    fn rejects_a_65_character_key() {
        assert_eq!(IdempotencyKey::parse(&"k".repeat(65)), None);
    }

    #[test]
    fn rejects_spaces_and_non_ascii() {
        assert_eq!(IdempotencyKey::parse("a b"), None);
        assert_eq!(IdempotencyKey::parse("café"), None);
        assert_eq!(IdempotencyKey::parse("é"), None);
    }

    #[test]
    fn rejects_control_characters() {
        assert_eq!(IdempotencyKey::parse("a\tb"), None);
        assert_eq!(IdempotencyKey::parse("a\nb"), None);
        assert_eq!(IdempotencyKey::parse("\u{7f}"), None);
    }
}
