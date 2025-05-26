use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};

#[derive(Debug)]
pub struct SubscriptionToken(String);

impl SubscriptionToken {
    pub fn new() -> SubscriptionToken {
        let mut rng = thread_rng();
        let token = std::iter::repeat_with(|| rng.sample(Alphanumeric))
            .map(char::from)
            .take(25)
            .collect();

        SubscriptionToken(token)
    }

    pub fn parse(s: String) -> Result<SubscriptionToken, String> {
        let is_empty_or_whitespace = s.trim().is_empty();
        let is_not_alphanumeric = !s.chars().all(|c| c.is_ascii_alphanumeric());
        let is_not_correct_lenght = s.chars().count() != 25;

        if is_empty_or_whitespace || is_not_alphanumeric || is_not_correct_lenght {
            Err(format!("{s} is not a valid subscription token"))
        } else {
            Ok(Self(s))
        }
    }
}

impl Default for SubscriptionToken {
    fn default() -> Self {
        Self::new()
    }
}

impl AsRef<str> for SubscriptionToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriptionToken;
    use claims::{assert_err, assert_ok};

    #[test]
    fn a_25_chars_long_alphanumeric_token_is_valid() {
        let token = "a".repeat(25);
        assert_ok!(SubscriptionToken::parse(token));
    }

    #[test]
    fn a_token_longer_than_25_chars_is_rejected() {
        let token = "a".repeat(26);
        assert_err!(SubscriptionToken::parse(token));
    }

    #[test]
    fn a_token_less_than_25_chars_is_rejected() {
        let token = "a".repeat(24);
        assert_err!(SubscriptionToken::parse(token));
    }

    #[test]
    fn empty_token_is_rejected() {
        let token = "".to_string();
        assert_err!(SubscriptionToken::parse(token));
    }

    #[test]
    fn whitespace_only_tokens_are_rejected() {
        let token: String = " ".repeat(25);
        assert_err!(SubscriptionToken::parse(token));
    }

    #[test]
    fn tokens_containing_an_invalid_character_are_rejected() {
        for token in &['*', '@', 'ё', '🦀'] {
            let token = format!("{token}").repeat(25);
            assert_err!(SubscriptionToken::parse(token));
        }
    }

    #[test]
    fn a_valid_token_is_parsed_successfully() {
        let token = "1".repeat(25);
        assert_ok!(SubscriptionToken::parse(token));
    }
}
