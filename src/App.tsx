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
import type {
  PromptComposition,
  Provider,
} from "./features/providers/model";
import { useEmbeddedProvider } from "./features/providers/useEmbeddedProvider";
import { usePromptPlacement } from "./features/providers/usePromptPlacement";
import { useQuickCapture } from "./features/quickCapture/useQuickCapture";
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
  const quickCapture = useQuickCapture({
    onNotice: setNotice,
    onPermissionRequired: () => setShowSettings(true),
  });
  const { sourceText, setSourceText, captureClipboard } = quickCapture;
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
  const promptComposition: PromptComposition = {
    beforeText: instructionLibrary.selectedInstruction.beforeText,
    text: sourceText,
    afterText: instructionLibrary.selectedInstruction.afterText,
  };

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
        quickCaptureStatus={quickCapture.status}
        selectedId={instructionLibrary.selectedId}
      />

      <main className="workspace">
        <ProviderBrowser hostRef={hostRef} provider={provider} />

        <PromptDock
          composition={promptComposition}
          isWorking={isWorking}
          onCaptureClipboard={() => void captureClipboard()}
          onPlacePrompt={(composition) => void placePrompt(composition)}
          onProviderChange={setProvider}
          onSourceTextChange={setSourceText}
          provider={provider}
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
          isRequestingPermission={quickCapture.isRequestingPermission}
          isRetryingRegistration={quickCapture.isRetryingRegistration}
          onOpenSystemSettings={() => void quickCapture.openSystemSettings()}
          onRefreshQuickCapture={() => void quickCapture.refreshStatus()}
          onRequestPermission={() => void quickCapture.requestPermission()}
          onRetryRegistration={() => void quickCapture.retryRegistration()}
          onThemeChange={setTheme}
          quickCaptureStatus={quickCapture.status}
          theme={theme}
        />
      )}
    </div>
  );
}

export default App;
