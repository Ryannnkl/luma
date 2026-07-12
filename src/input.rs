use zeroize::{Zeroize, Zeroizing};

/// Holds the current password input without exposing its text to callers.
pub struct InputState {
    bytes: Zeroizing<Vec<u8>>,
    character_count: usize,
    max_characters: usize,
}

impl InputState {
    #[must_use]
    pub fn new(max_characters: usize) -> Self {
        Self {
            bytes: Zeroizing::new(Vec::new()),
            character_count: 0,
            max_characters,
        }
    }

    /// Adds printable Unicode text while respecting the character limit.
    pub fn push_text(&mut self, text: &str) {
        for character in text.chars().filter(|character| !character.is_control()) {
            if self.character_count == self.max_characters {
                break;
            }

            let mut encoded = [0_u8; 4];
            self.bytes
                .extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            encoded.zeroize();
            self.character_count += 1;
        }
    }

    /// Removes the most recently entered Unicode scalar value.
    pub fn backspace(&mut self) {
        let Some(last_character) = self
            .bytes
            .iter()
            .rposition(|byte| (*byte & 0b1100_0000) != 0b1000_0000)
        else {
            return;
        };

        self.bytes[last_character..].zeroize();
        self.bytes.truncate(last_character);
        self.character_count = self.character_count.saturating_sub(1);
    }

    /// Removes all password bytes from memory.
    pub fn clear(&mut self) {
        self.bytes.zeroize();
        self.bytes.clear();
        self.character_count = 0;
    }

    /// Transfers the current secret into an authentication-only container.
    ///
    /// An empty input never produces an authentication attempt.
    pub fn submit(&mut self) -> Option<PasswordAttempt> {
        if self.bytes.is_empty() {
            return None;
        }

        self.character_count = 0;
        Some(PasswordAttempt {
            bytes: std::mem::replace(&mut self.bytes, Zeroizing::new(Vec::new())),
        })
    }

    #[must_use]
    pub const fn character_count(&self) -> usize {
        self.character_count
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.character_count == 0
    }
}

/// A password handoff for the future PAM boundary.
///
/// The bytes are available only within the crate and are zeroized on drop.
pub struct PasswordAttempt {
    bytes: Zeroizing<Vec<u8>>,
}

impl PasswordAttempt {
    /// Borrows the password only for the authentication boundary.
    #[must_use]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for PasswordAttempt {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

#[cfg(test)]
impl PasswordAttempt {
    fn bytes_for_test(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::InputState;

    #[test]
    fn ignores_control_characters() {
        let mut input = InputState::new(8);

        input.push_text("a\nb\tc");

        assert_eq!(input.character_count(), 3);
    }

    #[test]
    fn backspace_removes_a_whole_unicode_scalar() {
        let mut input = InputState::new(8);
        input.push_text("aé");

        input.backspace();

        assert_eq!(input.character_count(), 1);
        let attempt = input.submit().expect("input should contain one character");
        assert_eq!(attempt.bytes_for_test(), b"a");
    }

    #[test]
    fn enforces_the_character_limit() {
        let mut input = InputState::new(2);

        input.push_text("abc");

        assert_eq!(input.character_count(), 2);
    }

    #[test]
    fn submitting_moves_the_secret_and_resets_input() {
        let mut input = InputState::new(8);
        input.push_text("secret");

        let attempt = input.submit().expect("input should submit");

        assert_eq!(attempt.bytes_for_test(), b"secret");
        assert!(input.is_empty());
        assert_eq!(input.character_count(), 0);
    }

    #[test]
    fn empty_input_does_not_submit() {
        assert!(InputState::new(8).submit().is_none());
    }

    #[test]
    fn clear_removes_visible_input_state() {
        let mut input = InputState::new(8);
        input.push_text("secret");

        input.clear();

        assert!(input.is_empty());
    }
}
