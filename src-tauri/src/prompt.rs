use serde::Deserialize;

/// Upper bound for any prompt Prompter composes or places, shared with the
/// Quick Capture selection limit so every text path enforces the same cap.
pub(crate) const MAX_PROMPT_BYTES: usize = 1_048_576;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptInput {
    before_text: String,
    text: String,
    #[serde(default)]
    after_text: String,
}

impl PromptInput {
    fn compose(self) -> Result<String, String> {
        let before_text = self.before_text.trim();
        let text = self.text.trim();
        let after_text = self.after_text.trim();

        if before_text.is_empty() {
            return Err("Choose an instruction first.".into());
        }

        if text.is_empty() {
            return Err("Add some text to rewrite first.".into());
        }

        let combined_length = before_text.len() + text.len() + after_text.len();
        if combined_length > MAX_PROMPT_BYTES {
            return Err("The prompt is too large to place. Shorten the text and try again.".into());
        }

        if after_text.is_empty() {
            Ok(format!("{before_text}\n\n{text}"))
        } else {
            Ok(format!("{before_text}\n\n{text}\n\n{after_text}"))
        }
    }
}

#[tauri::command]
pub(crate) fn compose_prompt(
    before_text: String,
    text: String,
    after_text: Option<String>,
) -> Result<String, String> {
    PromptInput {
        before_text,
        text,
        after_text: after_text.unwrap_or_default(),
    }
    .compose()
}

#[cfg(test)]
mod tests {
    use super::{compose_prompt, PromptInput};

    fn compose(before_text: &str, text: &str, after_text: Option<&str>) -> Result<String, String> {
        compose_prompt(
            before_text.into(),
            text.into(),
            after_text.map(str::to_string),
        )
    }

    #[test]
    fn compose_prompt_trims_and_orders_all_nonempty_sections() {
        let prompt = compose(
            "  Make this clearer  ",
            "  This sentence needs help.  ",
            Some("  Keep the original meaning.  "),
        )
        .expect("prompt should be composed");

        assert_eq!(
            prompt,
            "Make this clearer\n\nThis sentence needs help.\n\nKeep the original meaning."
        );
    }

    #[test]
    fn compose_prompt_omits_the_optional_empty_after_text() {
        let prompt = compose("  Rewrite clearly  ", "  Source text  ", Some("  ")).unwrap();

        assert_eq!(prompt, "Rewrite clearly\n\nSource text");
        assert!(!prompt.ends_with('\n'));
    }

    #[test]
    fn compose_prompt_adds_no_hidden_labels_or_instructions() {
        let prompt = compose("Before section", "User section", Some("After section")).unwrap();

        assert_eq!(prompt, "Before section\n\nUser section\n\nAfter section");
        assert!(!prompt.contains("Text:"));
        assert!(!prompt.contains("---"));
        assert!(!prompt.contains("Return only"));
    }

    #[test]
    fn compose_prompt_rejects_empty_before_text() {
        assert_eq!(
            compose("  ", "Text", Some("Optional suffix")),
            Err("Choose an instruction first.".into())
        );
    }

    #[test]
    fn compose_prompt_rejects_oversized_input() {
        let oversized = "x".repeat(super::MAX_PROMPT_BYTES + 1);

        assert_eq!(
            compose("Rewrite clearly", &oversized, None),
            Err("The prompt is too large to place. Shorten the text and try again.".into())
        );
    }

    #[test]
    fn compose_prompt_rejects_empty_text() {
        assert_eq!(
            compose("Make this clearer", "  ", Some("Optional suffix")),
            Err("Add some text to rewrite first.".into())
        );
    }

    #[test]
    fn after_text_defaults_to_empty_when_omitted_from_json() {
        let input: PromptInput =
            serde_json::from_str(r#"{"beforeText":"Rewrite clearly","text":"Source text"}"#)
                .unwrap();

        assert_eq!(input.compose().unwrap(), "Rewrite clearly\n\nSource text");
    }

    #[test]
    fn flat_command_contract_accepts_an_omitted_after_text() {
        assert_eq!(
            compose("Rewrite clearly", "Source text", None).unwrap(),
            "Rewrite clearly\n\nSource text"
        );
    }
}
