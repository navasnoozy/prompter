const OUTPUT_INSTRUCTION: &str =
    "Return only the rewritten text unless the instruction asks for something else.";

#[tauri::command]
pub(crate) fn compose_prompt(instruction: String, text: String) -> Result<String, String> {
    let instruction = instruction.trim();
    let text = text.trim();

    if instruction.is_empty() {
        return Err("Choose an instruction first.".into());
    }

    if text.is_empty() {
        return Err("Add some text to rewrite first.".into());
    }

    Ok(format!(
        "{instruction}\n\nText to rewrite:\n---\n{text}\n---\n\n{OUTPUT_INSTRUCTION}"
    ))
}

#[cfg(test)]
mod tests {
    use super::compose_prompt;

    #[test]
    fn compose_prompt_trims_inputs_and_has_exact_structure() {
        let prompt = compose_prompt(
            "  Make this clearer  ".into(),
            "  This sentence needs help.  ".into(),
        )
        .expect("prompt should be composed");

        assert_eq!(
            prompt,
            "Make this clearer\n\nText to rewrite:\n---\nThis sentence needs help.\n---\n\nReturn only the rewritten text unless the instruction asks for something else."
        );
    }

    #[test]
    fn compose_prompt_rejects_empty_instruction() {
        assert_eq!(
            compose_prompt("  ".into(), "Text".into()),
            Err("Choose an instruction first.".into())
        );
    }

    #[test]
    fn compose_prompt_rejects_empty_text() {
        assert_eq!(
            compose_prompt("Make this clearer".into(), "  ".into()),
            Err("Add some text to rewrite first.".into())
        );
    }
}
