import { useEffect, useState } from "react";
import "./styles/index.css";
import { InstructionEditorDialog } from "./features/instructions/InstructionEditorDialog";
import { InstructionSidebar } from "./features/instructions/InstructionSidebar";
import type {
  InstructionDraft,
  InstructionPreset,
} from "./features/instructions/model";
import { useInstructionLibrary } from "./features/instructions/useInstructionLibrary";
import { useAppLifecycle } from "./features/lifecycle/useAppLifecycle";
import { PromptDock } from "./features/providers/PromptDock";
import { ProviderBrowser } from "./features/providers/ProviderBrowser";
import type {
  PromptComposition,
  Provider,
} from "./features/providers/model";
import {
  loadStoredProvider,
  saveStoredProvider,
} from "./features/providers/storage";
import { useEmbeddedProvider } from "./features/providers/useEmbeddedProvider";
import { usePromptPlacement } from "./features/providers/usePromptPlacement";
import { useQuickCapture } from "./features/quickCapture/useQuickCapture";
import { SettingsDialog } from "./features/settings/SettingsDialog";
import { useTheme } from "./features/settings/useTheme";

type EditorTarget = "new" | InstructionPreset | null;

function App() {
  const [provider, setProvider] = useState<Provider>(loadStoredProvider);
  const [notice, setNotice] = useState("Ready");
  const [editorTarget, setEditorTarget] = useState<EditorTarget>(null);
  const [showSettings, setShowSettings] = useState(false);

  useEffect(() => {
    saveStoredProvider(provider);
  }, [provider]);

  const instructionLibrary = useInstructionLibrary({
    onPersistError: () =>
      setNotice(
        "Instructions could not be saved. Changes may be lost when Prompter closes.",
      ),
  });
  const { theme, setTheme } = useTheme();
  const appLifecycle = useAppLifecycle({ onNotice: setNotice });
  const quickCapture = useQuickCapture({
    onNotice: setNotice,
    onPermissionRequired: () => setShowSettings(true),
  });
  const { sourceText, setSourceText, captureClipboard } = quickCapture;
  const { hostRef, ensureProvider } = useEmbeddedProvider({
    provider,
    visible:
      appLifecycle.status?.mainWindowVisible === true &&
      editorTarget === null &&
      !showSettings,
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
    try {
      instructionLibrary.saveInstruction(draft);
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error));
      return;
    }
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
          isUpdatingLaunchAtLogin={appLifecycle.isUpdatingLaunchAtLogin}
          isRequestingPermission={quickCapture.isRequestingPermission}
          isRetryingRegistration={quickCapture.isRetryingRegistration}
          launchAtLogin={appLifecycle.status?.launchAtLogin ?? null}
          onClose={() => setShowSettings(false)}
          onLaunchAtLoginChange={(enabled) =>
            void appLifecycle.setLaunchAtLogin(enabled)
          }
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
