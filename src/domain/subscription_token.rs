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
        let is_not_alphanumeric = !s.chars().all(char::is_alphanumeric);
        let is_not_correct_lenght = s.chars().count() != 25;

        if is_empty_or_whitespace || is_not_alphanumeric || is_not_correct_lenght {
            Err(format!("{s} is not a valid subscription token"))
        } else {
            Ok(Self(s))
        }
    }
}

impl AsRef<str> for SubscriptionToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
