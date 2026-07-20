use serde::Serialize;
use tauri::{
    menu::{Menu, MenuEvent, MenuItem, Submenu},
    AppHandle, Emitter, Runtime,
};

pub(crate) const APP_SHORTCUT_EVENT: &str = "prompter://app-shortcut";
const PLACE_PROMPT_ID: &str = "prompter.place-prompt";
const SELECT_CHATGPT_ID: &str = "prompter.select-chatgpt";
const SELECT_GEMINI_ID: &str = "prompter.select-gemini";

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum AppShortcutAction {
    PlacePrompt,
    SelectChatgpt,
    SelectGemini,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppShortcutEvent {
    version: u8,
    action: AppShortcutAction,
}

pub(crate) fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    let place_prompt = MenuItem::with_id(
        app,
        PLACE_PROMPT_ID,
        "Place Prompt",
        true,
        Some("CmdOrCtrl+Enter"),
    )?;
    let select_chatgpt = MenuItem::with_id(
        app,
        SELECT_CHATGPT_ID,
        "Use ChatGPT",
        true,
        Some("CmdOrCtrl+1"),
    )?;
    let select_gemini = MenuItem::with_id(
        app,
        SELECT_GEMINI_ID,
        "Use Gemini",
        true,
        Some("CmdOrCtrl+2"),
    )?;
    let actions = Submenu::with_items(
        app,
        "Actions",
        true,
        &[&place_prompt, &select_chatgpt, &select_gemini],
    )?;

    // Default macOS order is App, File, Edit, View, Window, Help. Keeping the
    // default menu preserves native edit, window, and quit behavior.
    menu.insert(&actions, 3)?;
    Ok(menu)
}

pub(crate) fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let action = if event.id() == PLACE_PROMPT_ID {
        AppShortcutAction::PlacePrompt
    } else if event.id() == SELECT_CHATGPT_ID {
        AppShortcutAction::SelectChatgpt
    } else if event.id() == SELECT_GEMINI_ID {
        AppShortcutAction::SelectGemini
    } else {
        return;
    };

    if let Err(error) = app.emit_to(
        crate::MAIN_WINDOW_LABEL,
        APP_SHORTCUT_EVENT,
        AppShortcutEvent { version: 1, action },
    ) {
        log::warn!(
            target: "prompter::shortcuts",
            "event=app_shortcut_emit_failed reason={error}"
        );
    }
}
