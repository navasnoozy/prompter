import { useState } from "react";
import "./styles/index.css";
import { InstructionEditorDialog } from "./features/instructions/InstructionEditorDialog";
import { InstructionSidebar } from "./features/instructions/InstructionSidebar";
import type {
  InstructionDraft,
  InstructionPreset,
} from "./features/instructions/model";
import { useInstructionLibrary } from "./features/instructions/useInstructionLibrary";
import { PromptDock } from "./features/providers/PromptDock";
import { ProviderBrowser } from "./features/providers/ProviderBrowser";
import type { Provider } from "./features/providers/model";
import { useClipboardCapture } from "./features/providers/useClipboardCapture";
import { useEmbeddedProvider } from "./features/providers/useEmbeddedProvider";
import { usePromptPlacement } from "./features/providers/usePromptPlacement";
import { SettingsDialog } from "./features/settings/SettingsDialog";
import { useTheme } from "./features/settings/useTheme";

type EditorTarget = "new" | InstructionPreset | null;

function App() {
  const [provider, setProvider] = useState<Provider>("chatgpt");
  const [notice, setNotice] = useState("Ready");
  const [editorTarget, setEditorTarget] = useState<EditorTarget>(null);
  const [showSettings, setShowSettings] = useState(false);

  const instructionLibrary = useInstructionLibrary();
  const { theme, setTheme } = useTheme();
  const { sourceText, setSourceText, captureClipboard } = useClipboardCapture({
    onNotice: setNotice,
  });
  const { hostRef, ensureProvider } = useEmbeddedProvider({
    provider,
    visible: editorTarget === null && !showSettings,
    onNotice: setNotice,
  });
  const { isWorking, placePrompt } = usePromptPlacement({
    provider,
    ensureProvider,
    onNotice: setNotice,
  });

  function saveInstruction(draft: InstructionDraft) {
    instructionLibrary.saveInstruction(draft);
    setEditorTarget(null);
    setNotice("Instruction saved");
  }

  function deleteInstruction(id: string) {
    instructionLibrary.deleteInstruction(id);
    setEditorTarget(null);
    setNotice("Instruction deleted");
  }

  return (
    <div className={`app-shell theme-${theme}`}>
      <InstructionSidebar
        instructions={instructionLibrary.instructions}
        onCreate={() => setEditorTarget("new")}
        onEdit={setEditorTarget}
        onOpenSettings={() => setShowSettings(true)}
        onSelect={instructionLibrary.selectInstruction}
        selectedId={instructionLibrary.selectedId}
      />

      <main className="workspace">
        <ProviderBrowser hostRef={hostRef} provider={provider} />

        <PromptDock
          isWorking={isWorking}
          onCaptureClipboard={() => void captureClipboard()}
          onPlacePrompt={() =>
            void placePrompt(
              instructionLibrary.selectedInstruction.instruction,
              sourceText,
            )
          }
          onProviderChange={setProvider}
          onSourceTextChange={setSourceText}
          provider={provider}
          sourceText={sourceText}
        />

        <footer aria-live="polite" className="status-bar">
          <span className="status-dot" />
          <span>{notice}</span>
          <span className="status-spacer" />
          <span>No API key</span>
        </footer>
      </main>

      {editorTarget !== null && (
        <InstructionEditorDialog
          canDelete={instructionLibrary.instructions.length > 1}
          instruction={editorTarget === "new" ? null : editorTarget}
          key={editorTarget === "new" ? "new" : editorTarget.id}
          onClose={() => setEditorTarget(null)}
          onDelete={deleteInstruction}
          onSave={saveInstruction}
        />
      )}

      {showSettings && (
        <SettingsDialog
          onClose={() => setShowSettings(false)}
          onThemeChange={setTheme}
          theme={theme}
        />
      )}
    </div>
  );
}

export default App;
