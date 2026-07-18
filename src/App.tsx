import { FormEvent, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import "./App.css";

type Provider = "chatgpt" | "gemini";

type Preset = {
  id: string;
  name: string;
  instruction: string;
  description: string;
  color: "violet" | "blue" | "amber" | "green" | "rose";
};

const DEFAULT_PRESETS: Preset[] = [
  {
    id: "clearer",
    name: "Make it clearer",
    instruction:
      "Rewrite the following text so it is clear, easy to understand, and well structured. Keep the original meaning.",
    description: "Simple, structured language",
    color: "violet",
  },
  {
    id: "grammar",
    name: "Fix grammar",
    instruction:
      "Correct the grammar, spelling, and punctuation in the following text. Do not change its meaning or tone.",
    description: "Clean up mistakes only",
    color: "blue",
  },
  {
    id: "professional",
    name: "Professional tone",
    instruction:
      "Rewrite the following text in a confident, polished, and professional tone. Keep it natural and concise.",
    description: "Polished and confident",
    color: "amber",
  },
  {
    id: "concise",
    name: "Make it concise",
    instruction:
      "Rewrite the following text using fewer words. Remove repetition and unnecessary details while preserving all important information.",
    description: "Shorter without losing meaning",
    color: "green",
  },
];

const PRESET_STORAGE_KEY = "prompter.presets.v1";

function loadPresets(): Preset[] {
  try {
    const saved = localStorage.getItem(PRESET_STORAGE_KEY);
    if (!saved) return DEFAULT_PRESETS;
    const parsed = JSON.parse(saved) as Preset[];
    return Array.isArray(parsed) && parsed.length > 0 ? parsed : DEFAULT_PRESETS;
  } catch {
    return DEFAULT_PRESETS;
  }
}

function Icon({ name, size = 18 }: { name: string; size?: number }) {
  const paths: Record<string, React.ReactNode> = {
    sparkle: <path d="M12 2l1.35 4.15L17.5 7.5l-4.15 1.35L12 13l-1.35-4.15L6.5 7.5l4.15-1.35L12 2Zm6 10 .9 2.6 2.6.9-2.6.9L18 19l-.9-2.6-2.6-.9 2.6-.9L18 12ZM5 13l1.05 2.95L9 17l-2.95 1.05L5 21l-1.05-2.95L1 17l2.95-1.05L5 13Z" />,
    wand: <path d="m4 20 12.8-12.8m-10.6 9.6 1 1M14.8 8.2l1 1M15 2l.7 2.3L18 5l-2.3.7L15 8l-.7-2.3L12 5l2.3-.7L15 2Zm5 7 .5 1.5L22 11l-1.5.5L20 13l-.5-1.5L18 11l1.5-.5L20 9ZM5 3l.7 2.3L8 6l-2.3.7L5 9l-.7-2.3L2 6l2.3-.7L5 3Z" />,
    clipboard: <path d="M9 5h6m-6 0a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2m-6 0a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2m0 0h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2" />,
    plus: <path d="M12 5v14M5 12h14" />,
    settings: <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm0-12v2m0 13v2m8.5-8.5h-2m-13 0h-2m14.5-6-1.4 1.4M7.4 16.6 6 18m12 0-1.4-1.4M7.4 7.4 6 6" />,
    chevron: <path d="m9 18 6-6-6-6" />,
    copy: <path d="M8 8h11v11H8zM5 16H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h11a1 1 0 0 1 1 1v1" />,
    replace: <path d="M20 7h-9a4 4 0 0 0-4 4v1m-3-3 3 3-3 3m0 2h9a4 4 0 0 0 4-4v-1m3 3-3-3 3-3" />,
    external: <path d="M14 3h7v7m0-7-9 9M10 5H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-5" />,
    edit: <path d="m4 20 4.2-1 10.6-10.6a2.1 2.1 0 0 0-3-3L5.2 16 4 20Zm10.5-13.5 3 3" />,
    trash: <path d="M4 7h16M9 7V4h6v3m3 0-1 14H7L6 7m4 4v6m4-6v6" />,
    close: <path d="m6 6 12 12M18 6 6 18" />,
    check: <path d="m5 12 4 4L19 6" />,
  };

  return (
    <svg
      aria-hidden="true"
      className="icon"
      fill="none"
      height={size}
      viewBox="0 0 24 24"
      width={size}
    >
      <g
        fill={name === "sparkle" ? "currentColor" : "none"}
        stroke={name === "sparkle" ? "none" : "currentColor"}
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.8"
      >
        {paths[name]}
      </g>
    </svg>
  );
}

function App() {
  const [presets, setPresets] = useState<Preset[]>(loadPresets);
  const [selectedPresetId, setSelectedPresetId] = useState(DEFAULT_PRESETS[0].id);
  const [provider, setProvider] = useState<Provider>("chatgpt");
  const [sourceText, setSourceText] = useState("");
  const [result, setResult] = useState("");
  const [notice, setNotice] = useState("Ready");
  const [isWorking, setIsWorking] = useState(false);
  const [showEditor, setShowEditor] = useState(false);
  const [editingPreset, setEditingPreset] = useState<Preset | null>(null);

  const selectedPreset = useMemo(
    () => presets.find((preset) => preset.id === selectedPresetId) ?? presets[0],
    [presets, selectedPresetId],
  );

  useEffect(() => {
    localStorage.setItem(PRESET_STORAGE_KEY, JSON.stringify(presets));
  }, [presets]);

  useEffect(() => {
    const unlistenPromise = listen<string>("prompter://clipboard-captured", (event) => {
      if (event.payload.trim()) {
        setSourceText(event.payload);
        setNotice("Copied text captured");
      }
    });

    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const unlistenResult = listen<{ provider: Provider; text: string }>(
      "prompter://rewrite-result",
      (event) => {
        setResult(event.payload.text);
        setIsWorking(false);
        setNotice(`Rewrite completed with ${event.payload.provider === "chatgpt" ? "ChatGPT" : "Gemini"}`);
      },
    );
    const unlistenError = listen<string>("prompter://provider-error", (event) => {
      setIsWorking(false);
      setNotice(event.payload);
    });

    return () => {
      void unlistenResult.then((unlisten) => unlisten());
      void unlistenError.then((unlisten) => unlisten());
    };
  }, []);

  async function captureClipboard() {
    try {
      const text = await readText();
      if (!text.trim()) {
        setNotice("Clipboard is empty");
        return;
      }
      setSourceText(text);
      setNotice("Clipboard text captured");
    } catch {
      setNotice("Copy text first, then try again");
    }
  }

  async function openProvider() {
    try {
      await invoke("open_provider_window", { provider });
      setNotice(`${provider === "chatgpt" ? "ChatGPT" : "Gemini"} opened inside Prompter`);
    } catch (error) {
      setNotice(String(error));
    }
  }

  async function prepareRewrite() {
    if (!sourceText.trim()) {
      setNotice("Add or capture some text first");
      return;
    }

    setIsWorking(true);
    setResult("");
    let waitingForProvider = false;
    try {
      const prompt = await invoke<string>("compose_prompt", {
        instruction: selectedPreset.instruction,
        text: sourceText,
      });
      await writeText(prompt);
      await invoke("send_prompt_to_provider", { provider, prompt });
      waitingForProvider = true;
      setNotice(`Sent to ${provider === "chatgpt" ? "ChatGPT" : "Gemini"} — waiting for the response`);
    } catch (error) {
      const message = String(error);
      if (message.includes("sign in first")) {
        await invoke("open_provider_window", { provider });
        setNotice("Sign in inside the AI window, keep it open, then press Rewrite again");
      } else {
        setNotice(`Could not rewrite: ${message}`);
      }
    } finally {
      if (!waitingForProvider) setIsWorking(false);
    }
  }

  async function copyResult() {
    if (!result) return;
    await writeText(result);
    setNotice("Result copied");
  }

  function beginCreatePreset() {
    setEditingPreset({
      id: "",
      name: "",
      instruction: "",
      description: "",
      color: "rose",
    });
    setShowEditor(true);
  }

  function beginEditPreset(preset: Preset) {
    setEditingPreset({ ...preset });
    setShowEditor(true);
  }

  function savePreset(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingPreset?.name.trim() || !editingPreset.instruction.trim()) return;

    if (editingPreset.id) {
      setPresets((current) =>
        current.map((preset) =>
          preset.id === editingPreset.id ? editingPreset : preset,
        ),
      );
      setSelectedPresetId(editingPreset.id);
    } else {
      const created = {
        ...editingPreset,
        id: `custom-${Date.now()}`,
        description: editingPreset.description || "Custom instruction",
      };
      setPresets((current) => [...current, created]);
      setSelectedPresetId(created.id);
    }

    setShowEditor(false);
    setEditingPreset(null);
    setNotice("Instruction saved");
  }

  function deletePreset(id: string) {
    if (presets.length === 1) return;
    const next = presets.filter((preset) => preset.id !== id);
    setPresets(next);
    if (selectedPresetId === id) setSelectedPresetId(next[0].id);
    setShowEditor(false);
    setEditingPreset(null);
    setNotice("Instruction deleted");
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark"><Icon name="sparkle" size={19} /></span>
          <span>Prompter</span>
        </div>

        <nav className="nav-list" aria-label="Main navigation">
          <button className="nav-item active" type="button">
            <Icon name="wand" /> Rewrite
          </button>
          <button className="nav-item" onClick={beginCreatePreset} type="button">
            <Icon name="plus" /> New instruction
          </button>
          <button className="nav-item" onClick={openProvider} type="button">
            <Icon name="external" /> AI account
          </button>
        </nav>

        <div className="sidebar-tip">
          <span className="tip-kicker">Quick capture</span>
          <strong>Select text, then press</strong>
          <div className="shortcut-row">
            <kbd>⌘</kbd><kbd>⇧</kbd><kbd>P</kbd>
          </div>
          <small>Select text first. Prompter copies it automatically.</small>
        </div>

        <button className="nav-item settings-link" type="button">
          <Icon name="settings" /> Settings
        </button>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div>
            <p className="eyebrow">Rewrite workspace</p>
            <h1>Turn rough text into the right words.</h1>
          </div>
          <div className="topbar-actions">
            <span className="privacy-pill"><span className="status-dot" /> Local presets</span>
            <button className="icon-button" title="Settings" type="button"><Icon name="settings" /></button>
          </div>
        </header>

        <section className="provider-bar" aria-label="AI provider">
          <div>
            <span className="section-label">AI provider</span>
            <p>Your login stays inside the provider WebView.</p>
          </div>
          <div className="provider-options">
            <button
              className={`provider-button ${provider === "chatgpt" ? "selected" : ""}`}
              onClick={() => setProvider("chatgpt")}
              type="button"
            >
              <span className="provider-logo chatgpt">◎</span>
              ChatGPT
              {provider === "chatgpt" && <Icon name="check" size={15} />}
            </button>
            <button
              className={`provider-button ${provider === "gemini" ? "selected" : ""}`}
              onClick={() => setProvider("gemini")}
              type="button"
            >
              <span className="provider-logo gemini">✦</span>
              Gemini
              {provider === "gemini" && <Icon name="check" size={15} />}
            </button>
            <button className="text-button" onClick={openProvider} type="button">
              Open login <Icon name="external" size={15} />
            </button>
          </div>
        </section>

        <div className="content-grid">
          <section className="composer-card">
            <div className="card-heading">
              <div>
                <span className="step-number">1</span>
                <span className="section-label">Your text</span>
              </div>
              <button className="text-button" onClick={captureClipboard} type="button">
                <Icon name="clipboard" size={16} /> Capture clipboard
              </button>
            </div>

            <textarea
              aria-label="Text to rewrite"
              onChange={(event) => setSourceText(event.target.value)}
              placeholder="Copy text from Notion, Apple Notes, Mail, or any other app…"
              value={sourceText}
            />
            <div className="textarea-meta">
              <span>{sourceText.length} characters</span>
              {sourceText && (
                <button onClick={() => setSourceText("")} type="button">Clear</button>
              )}
            </div>

            <div className="instruction-heading">
              <div>
                <span className="step-number">2</span>
                <span className="section-label">Choose an instruction</span>
              </div>
              <button className="text-button" onClick={beginCreatePreset} type="button">
                <Icon name="plus" size={16} /> Create custom
              </button>
            </div>

            <div className="preset-list">
              {presets.map((preset) => (
                <div
                  className={`preset-row ${selectedPresetId === preset.id ? "selected" : ""}`}
                  key={preset.id}
                >
                  <button
                    className="preset-main"
                    onClick={() => setSelectedPresetId(preset.id)}
                    type="button"
                  >
                    <span className={`preset-icon ${preset.color}`}><Icon name="wand" size={17} /></span>
                    <span>
                      <strong>{preset.name}</strong>
                      <small>{preset.description}</small>
                    </span>
                  </button>
                  <button
                    aria-label={`Edit ${preset.name}`}
                    className="edit-preset"
                    onClick={() => beginEditPreset(preset)}
                    type="button"
                  >
                    <Icon name="edit" size={15} />
                  </button>
                </div>
              ))}
            </div>

            <button
              className="rewrite-button"
              disabled={isWorking || !sourceText.trim()}
              onClick={prepareRewrite}
              type="button"
            >
              <Icon name="sparkle" size={18} />
              {isWorking ? "Preparing…" : `Rewrite with ${provider === "chatgpt" ? "ChatGPT" : "Gemini"}`}
              <span className="button-arrow"><Icon name="chevron" size={16} /></span>
            </button>
          </section>

          <section className="result-card">
            <div className="card-heading result-heading">
              <div>
                <span className="step-number">3</span>
                <span className="section-label">Result</span>
              </div>
              <span className="result-state">{result ? "Complete" : "Waiting"}</span>
            </div>

            {result ? (
              <div className="result-content">{result}</div>
            ) : (
              <div className="empty-result">
                <span className="empty-orbit"><Icon name="sparkle" size={27} /></span>
                <strong>Your rewritten text will appear here</strong>
                <p>Select an instruction and press Rewrite. Prompter will return the AI response here.</p>
              </div>
            )}

            <div className="result-actions">
              <button disabled={!result} onClick={copyResult} type="button">
                <Icon name="copy" size={16} /> Copy
              </button>
              <button className="replace-button" disabled={!result} type="button">
                <Icon name="replace" size={16} /> Replace original
              </button>
            </div>
          </section>
        </div>

        <footer className="status-bar">
          <span className="status-dot" />
          <span>{notice}</span>
          <span className="status-spacer" />
          <span>No API key</span>
        </footer>
      </main>

      {showEditor && editingPreset && (
        <div className="modal-backdrop" role="presentation">
          <form className="preset-modal" onSubmit={savePreset}>
            <div className="modal-header">
              <div>
                <p className="eyebrow">Custom behaviour</p>
                <h2>{editingPreset.id ? "Edit instruction" : "Create instruction"}</h2>
              </div>
              <button
                aria-label="Close"
                className="icon-button"
                onClick={() => setShowEditor(false)}
                type="button"
              >
                <Icon name="close" />
              </button>
            </div>

            <label>
              Name
              <input
                autoFocus
                onChange={(event) => setEditingPreset({ ...editingPreset, name: event.target.value })}
                placeholder="Example: Friendly email"
                value={editingPreset.name}
              />
            </label>
            <label>
              Short description
              <input
                onChange={(event) => setEditingPreset({ ...editingPreset, description: event.target.value })}
                placeholder="What this instruction does"
                value={editingPreset.description}
              />
            </label>
            <label>
              Instruction sent to AI
              <textarea
                onChange={(event) => setEditingPreset({ ...editingPreset, instruction: event.target.value })}
                placeholder="Rewrite the text in a friendly and helpful tone…"
                value={editingPreset.instruction}
              />
            </label>

            <div className="modal-actions">
              {editingPreset.id && (
                <button
                  className="danger-button"
                  onClick={() => deletePreset(editingPreset.id)}
                  type="button"
                >
                  <Icon name="trash" size={16} /> Delete
                </button>
              )}
              <span />
              <button className="secondary-button" onClick={() => setShowEditor(false)} type="button">Cancel</button>
              <button className="primary-button" type="submit">Save instruction</button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}

export default App;
