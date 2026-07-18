import {
  FormEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import "./App.css";

type Provider = "chatgpt" | "gemini";
type AppTheme = "light" | "dark";

type ProviderBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type Preset = {
  id: string;
  name: string;
  instruction: string;
  color: "violet" | "blue" | "amber" | "green" | "rose";
};

const DEFAULT_PRESETS: Preset[] = [
  {
    id: "clearer",
    name: "Make it clearer",
    instruction:
      "Rewrite the following text so it is clear, easy to understand, and well structured. Keep the original meaning.",
    color: "violet",
  },
  {
    id: "grammar",
    name: "Fix grammar",
    instruction:
      "Correct the grammar, spelling, and punctuation in the following text. Do not change its meaning or tone.",
    color: "blue",
  },
  {
    id: "professional",
    name: "Professional tone",
    instruction:
      "Rewrite the following text in a confident, polished, and professional tone. Keep it natural and concise.",
    color: "amber",
  },
  {
    id: "concise",
    name: "Make it concise",
    instruction:
      "Rewrite the following text using fewer words. Remove repetition and unnecessary details while preserving all important information.",
    color: "green",
  },
];

const PRESET_STORAGE_KEY = "prompter.presets.v1";
const THEME_STORAGE_KEY = "prompter.theme.v1";

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

function loadTheme(): AppTheme {
  return localStorage.getItem(THEME_STORAGE_KEY) === "dark" ? "dark" : "light";
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
    edit: <path d="m4 20 4.2-1 10.6-10.6a2.1 2.1 0 0 0-3-3L5.2 16 4 20Zm10.5-13.5 3 3" />,
    trash: <path d="M4 7h16M9 7V4h6v3m3 0-1 14H7L6 7m4 4v6m4-6v6" />,
    close: <path d="m6 6 12 12M18 6 6 18" />,
    check: <path d="m5 12 4 4L19 6" />,
    moon: <path d="M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5 8.5 8.5 0 1 0 20.5 14.2Z" />,
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
  const [notice, setNotice] = useState("Ready");
  const [isWorking, setIsWorking] = useState(false);
  const [providerReady, setProviderReady] = useState(false);
  const [theme, setTheme] = useState<AppTheme>(loadTheme);
  const [showEditor, setShowEditor] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [editingPreset, setEditingPreset] = useState<Preset | null>(null);
  const providerHostRef = useRef<HTMLDivElement | null>(null);

  const selectedPreset = useMemo(
    () => presets.find((preset) => preset.id === selectedPresetId) ?? presets[0],
    [presets, selectedPresetId],
  );

  useEffect(() => {
    localStorage.setItem(PRESET_STORAGE_KEY, JSON.stringify(presets));
  }, [presets]);

  useEffect(() => {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
    document.documentElement.style.colorScheme = theme;
  }, [theme]);

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
    const unlistenFilled = listen<Provider>(
      "prompter://prompt-filled",
      (event) => {
        setIsWorking(false);
        setNotice(
          `Prompt ready in ${event.payload === "chatgpt" ? "ChatGPT" : "Gemini"} — review it and press Send`,
        );
      },
    );
    const unlistenError = listen<string>("prompter://provider-error", (event) => {
      setIsWorking(false);
      setNotice(event.payload);
    });

    return () => {
      void unlistenFilled.then((unlisten) => unlisten());
      void unlistenError.then((unlisten) => unlisten());
    };
  }, []);

  const readProviderBounds = useCallback((): ProviderBounds | null => {
    const host = providerHostRef.current;
    if (!host) return null;
    const rect = host.getBoundingClientRect();
    if (rect.width < 240 || rect.height < 240) return null;

    return {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
    };
  }, []);

  const ensureProviderWebview = useCallback(async () => {
    const bounds = readProviderBounds();
    if (!bounds) throw new Error("The embedded browser area is not ready yet.");
    await invoke("show_provider_webview", { provider, bounds });
    setProviderReady(true);
  }, [provider, readProviderBounds]);

  useLayoutEffect(() => {
    let disposed = false;
    let animationFrame = 0;

    const showProvider = async () => {
      setProviderReady(false);
      try {
        await ensureProviderWebview();
        if (!disposed) {
          setNotice(`${provider === "chatgpt" ? "ChatGPT" : "Gemini"} is ready inside Prompter`);
        }
      } catch (error) {
        if (!disposed) setNotice(String(error));
      }
    };

    const resizeProvider = () => {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = window.requestAnimationFrame(() => {
        const bounds = readProviderBounds();
        if (bounds) {
          void invoke("resize_provider_webview", { provider, bounds });
        }
      });
    };

    void showProvider();
    const observer = new ResizeObserver(resizeProvider);
    if (providerHostRef.current) observer.observe(providerHostRef.current);
    window.addEventListener("resize", resizeProvider);

    return () => {
      disposed = true;
      observer.disconnect();
      window.removeEventListener("resize", resizeProvider);
      window.cancelAnimationFrame(animationFrame);
    };
  }, [ensureProviderWebview, provider, readProviderBounds]);

  useEffect(() => {
    void invoke("set_provider_visibility", {
      provider,
      visible: !showEditor && !showSettings,
    });
  }, [provider, showEditor, showSettings]);

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

  async function prepareRewrite() {
    if (!sourceText.trim()) {
      setNotice("Add or capture some text first");
      return;
    }

    setIsWorking(true);
    try {
      await ensureProviderWebview();
      const prompt = await invoke<string>("compose_prompt", {
        instruction: selectedPreset.instruction,
        text: sourceText,
      });
      await invoke("fill_provider_prompt", { provider, prompt });
      setNotice(`Placing the prompt in ${provider === "chatgpt" ? "ChatGPT" : "Gemini"}…`);
    } catch (error) {
      setIsWorking(false);
      setNotice(`Could not prepare the prompt: ${String(error)}`);
    }
  }

  function beginCreatePreset() {
    setEditingPreset({
      id: "",
      name: "",
      instruction: "",
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
    <div className={`app-shell theme-${theme}`}>
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark"><Icon name="sparkle" size={19} /></span>
          <span>Prompter</span>
        </div>

        <section className="sidebar-instructions" aria-label="Instructions">
          <div className="sidebar-section-heading">
            <span>Instructions</span>
            <button
              aria-label="Create instruction"
              onClick={beginCreatePreset}
              title="Create instruction"
              type="button"
            >
              <Icon name="plus" size={15} />
            </button>
          </div>

          <div className="preset-list sidebar-preset-list">
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
                  <span className={`preset-icon ${preset.color}`}><Icon name="wand" size={16} /></span>
                  <span>
                    <strong>{preset.name}</strong>
                  </span>
                </button>
                <button
                  aria-label={`Edit ${preset.name}`}
                  className="edit-preset"
                  onClick={() => beginEditPreset(preset)}
                  type="button"
                >
                  <Icon name="edit" size={14} />
                </button>
              </div>
            ))}
          </div>
        </section>

        <div className="sidebar-tip">
          <span className="tip-kicker">Quick capture</span>
          <strong>Select text, then press</strong>
          <div className="shortcut-row">
            <kbd>⌘</kbd><kbd>⇧</kbd><kbd>P</kbd>
          </div>
          <small>Select text first. Prompter copies it automatically.</small>
        </div>

        <button
          className="nav-item settings-link"
          onClick={() => setShowSettings(true)}
          type="button"
        >
          <Icon name="settings" /> Settings
        </button>
      </aside>

      <main className="workspace">
        <section className="provider-bar" aria-label="AI provider">
          <div>
            <span className="section-label">AI provider</span>
            <p>Login, chat and responses stay inside this Prompter window.</p>
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
          </div>
        </section>

        <div className="content-grid">
          <section className="composer-card">
            <div className="card-heading">
              <div>
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

            <button
              className="rewrite-button"
              disabled={isWorking || !sourceText.trim()}
              onClick={prepareRewrite}
              type="button"
            >
              <Icon name="sparkle" size={18} />
              {isWorking ? "Placing prompt…" : `Place in ${provider === "chatgpt" ? "ChatGPT" : "Gemini"}`}
              <span className="button-arrow"><Icon name="chevron" size={16} /></span>
            </button>
            <p className="manual-send-note">
              Prompter fills the AI input box. You review it and press Send.
            </p>
          </section>

          <section className="provider-card">
            <div className="card-heading provider-card-heading">
              <div>
                <span className="section-label">
                  {provider === "chatgpt" ? "ChatGPT" : "Gemini"}
                </span>
              </div>
              <span className={`provider-state ${providerReady ? "ready" : ""}`}>
                <span className="status-dot" />
                {providerReady ? "Ready" : "Loading"}
              </span>
            </div>

            <div className="provider-webview-frame">
              <div className="provider-loading-placeholder">
                <span className="empty-orbit"><Icon name="sparkle" size={25} /></span>
                <strong>Opening {provider === "chatgpt" ? "ChatGPT" : "Gemini"}…</strong>
                <p>Sign in here once, then use the same panel for every rewrite.</p>
              </div>
              <div className="provider-webview-host" ref={providerHostRef} />
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

      {showSettings && (
        <div className="modal-backdrop" role="presentation">
          <section className="preset-modal settings-modal" aria-label="Settings">
            <div className="modal-header">
              <div>
                <p className="eyebrow">Prompter</p>
                <h2>Settings</h2>
              </div>
              <button
                aria-label="Close settings"
                className="icon-button"
                onClick={() => setShowSettings(false)}
                type="button"
              >
                <Icon name="close" />
              </button>
            </div>

            <div className="settings-section">
              <div>
                <strong>Appearance</strong>
                <p>Choose how the Prompter interface looks.</p>
              </div>
              <div className="theme-options" role="group" aria-label="Appearance">
                <button
                  className={theme === "light" ? "selected" : ""}
                  onClick={() => setTheme("light")}
                  type="button"
                >
                  <Icon name="settings" size={16} /> Light
                </button>
                <button
                  className={theme === "dark" ? "selected" : ""}
                  onClick={() => setTheme("dark")}
                  type="button"
                >
                  <Icon name="moon" size={16} /> Dark
                </button>
              </div>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

export default App;
