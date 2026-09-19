//! Prompt input state. Mirrors upstream `context/prompt.tsx`, reduced to the
//! single-line editing M1 needs.

/// The current prompt buffer.
#[derive(Default)]
pub struct Prompt {
    buffer: String,
}

impl Prompt {
    pub fn text(&self) -> &str {
        &self.buffer
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn push(&mut self, character: char) {
        self.buffer.push(character);
    }

    pub fn pop(&mut self) {
        self.buffer.pop();
    }

    /// Remove and return the buffer, leaving it empty.
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_and_taking_the_buffer() {
        let mut prompt = Prompt::default();
        assert!(prompt.is_empty());

        prompt.push('h');
        prompt.push('i');
        prompt.pop();
        assert_eq!(prompt.text(), "h");
        assert!(!prompt.is_empty());

        assert_eq!(prompt.take(), "h");
        assert!(prompt.is_empty());
    }
}
