import "./styles/index.css";
import { InstructionEditorDialog } from "./features/instructions/InstructionEditorDialog";
import { InstructionSidebar } from "./features/instructions/InstructionSidebar";
import { useInstructionStore } from "./features/instructions/store";
import { useAppLifecycle } from "./features/lifecycle/useAppLifecycle";
import { PromptDock } from "./features/providers/PromptDock";
import { ProviderBrowser } from "./features/providers/ProviderBrowser";
import { useEmbeddedProvider } from "./features/providers/useEmbeddedProvider";
import { usePromptPlacement } from "./features/providers/usePromptPlacement";
import { useProviderNavigation } from "./features/providers/useProviderNavigation";
import { useQuickCapture } from "./features/quickCapture/useQuickCapture";
import { SettingsDialog } from "./features/settings/SettingsDialog";
import { useSettingsStore } from "./features/settings/store";
import { StatusBar } from "./shared/StatusBar";
import { useAppShortcuts } from "./shared/useAppShortcuts";

// Composition root: binds native events to stores and lays out the shell.
// All state lives in feature stores; no data flows through props here.
function App() {
  useAppLifecycle();
  useQuickCapture();
  usePromptPlacement();
  useProviderNavigation();
  useAppShortcuts();
  const { hostRef } = useEmbeddedProvider();

  const theme = useSettingsStore((state) => state.theme);
  const showSettings = useSettingsStore((state) => state.showSettings);
  const editorTarget = useInstructionStore((state) => state.editorTarget);

  return (
    <div className={`app-shell theme-${theme}`}>
      <InstructionSidebar />

      <main className="workspace">
        <ProviderBrowser hostRef={hostRef} />
        <PromptDock />
        <StatusBar />
      </main>

      {editorTarget !== null && (
        <InstructionEditorDialog
          key={editorTarget === "new" ? "new" : editorTarget.id}
        />
      )}

      {showSettings && <SettingsDialog />}
    </div>
  );
}

export default App;
