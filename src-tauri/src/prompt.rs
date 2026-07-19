use serde::Deserialize;

/// Upper bound for any prompt Prompter composes or places, shared with the
/// Quick Capture selection limit so every text path enforces the same cap.
pub(crate) const MAX_PROMPT_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptComposeError {
    MissingInstruction,
    MissingText,
    TooLarge,
}

impl PromptComposeError {
    pub(crate) fn user_message(self) -> &'static str {
        match self {
            Self::MissingInstruction => "Choose an instruction first.",
            Self::MissingText => "Add some text to rewrite first.",
            Self::TooLarge => "The prompt is too large to place. Shorten the text and try again.",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptInput {
    before_text: String,
    text: String,
    #[serde(default)]
    after_text: String,
}

impl PromptInput {
    pub(crate) fn compose(self) -> Result<String, PromptComposeError> {
        let before_text = self.before_text.trim();
        let text = self.text.trim();
        let after_text = self.after_text.trim();

        if before_text.is_empty() {
            return Err(PromptComposeError::MissingInstruction);
        }

        if text.is_empty() {
            return Err(PromptComposeError::MissingText);
        }

        let combined_length = before_text.len() + text.len() + after_text.len();
        if combined_length > MAX_PROMPT_BYTES {
            return Err(PromptComposeError::TooLarge);
        }

        if after_text.is_empty() {
            Ok(format!("{before_text}\n\n{text}"))
        } else {
            Ok(format!("{before_text}\n\n{text}\n\n{after_text}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PromptComposeError, PromptInput, MAX_PROMPT_BYTES};

    fn compose(
        before_text: &str,
        text: &str,
        after_text: &str,
    ) -> Result<String, PromptComposeError> {
        PromptInput {
            before_text: before_text.into(),
            text: text.into(),
            after_text: after_text.into(),
        }
        .compose()
    }

    #[test]
    fn compose_trims_and_orders_all_nonempty_sections() {
        let prompt = compose(
            "  Make this clearer  ",
            "  This sentence needs help.  ",
            "  Keep the original meaning.  ",
        )
        .expect("prompt should be composed");

        assert_eq!(
            prompt,
            "Make this clearer\n\nThis sentence needs help.\n\nKeep the original meaning."
        );
    }

    #[test]
    fn compose_omits_the_optional_empty_after_text() {
        let prompt = compose("  Rewrite clearly  ", "  Source text  ", "  ").unwrap();

        assert_eq!(prompt, "Rewrite clearly\n\nSource text");
        assert!(!prompt.ends_with('\n'));
    }

    #[test]
    fn compose_adds_no_hidden_labels_or_instructions() {
        let prompt = compose("Before section", "User section", "After section").unwrap();

        assert_eq!(prompt, "Before section\n\nUser section\n\nAfter section");
        assert!(!prompt.contains("Text:"));
        assert!(!prompt.contains("---"));
        assert!(!prompt.contains("Return only"));
    }

    #[test]
    fn compose_rejects_empty_before_text() {
        assert_eq!(
            compose("  ", "Text", "Optional suffix"),
            Err(PromptComposeError::MissingInstruction)
        );
    }

    #[test]
    fn compose_rejects_empty_text() {
        assert_eq!(
            compose("Make this clearer", "  ", "Optional suffix"),
            Err(PromptComposeError::MissingText)
        );
    }

    #[test]
    fn compose_rejects_oversized_input() {
        let oversized = "x".repeat(MAX_PROMPT_BYTES + 1);

        assert_eq!(
            compose("Rewrite clearly", &oversized, ""),
            Err(PromptComposeError::TooLarge)
        );
    }

    #[test]
    fn composition_deserializes_camel_case_with_optional_after_text() {
        let input: PromptInput =
            serde_json::from_str(r#"{"beforeText":"Rewrite clearly","text":"Source text"}"#)
                .unwrap();

        assert_eq!(input.compose().unwrap(), "Rewrite clearly\n\nSource text");
    }
}
