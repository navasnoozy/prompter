import { create } from "zustand";
import { createId } from "../../shared/ids";
import { publishNotice } from "../../shared/notices";
import { settingsGateway, SETTINGS_KEYS } from "../../shared/settingsGateway";
import {
  addInstruction,
  createInstructionLibrary,
  getSelectedInstruction,
  removeInstruction,
  selectInstruction,
  updateInstruction,
} from "./collection";
import { createDefaultInstructions } from "./defaults";
import type {
  InstructionDraft,
  InstructionLibrary,
  InstructionPreset,
} from "./model";
import { normalizeInstructionDraft } from "./model";

export type EditorTarget = "new" | InstructionPreset | null;

type InstructionState = {
  library: InstructionLibrary;
  editorTarget: EditorTarget;
  select: (id: string) => void;
  saveDraft: (draft: InstructionDraft) => void;
  deleteInstruction: (id: string) => void;
  openEditor: (target: Exclude<EditorTarget, null>) => void;
  closeEditor: () => void;
};

function persistLibrary(library: InstructionLibrary): void {
  void settingsGateway.writeMany({
    [SETTINGS_KEYS.presets]: {
      version: 2,
      instructions: library.instructions,
    },
    [SETTINGS_KEYS.selectedInstructionId]: library.selectedId,
  });
}

export const useInstructionStore = create<InstructionState>()((set, get) => ({
  library: createInstructionLibrary(createDefaultInstructions()),
  editorTarget: null,

  select: (id) => {
    set((state) => ({ library: selectInstruction(state.library, id) }));
    void settingsGateway.write(
      SETTINGS_KEYS.selectedInstructionId,
      get().library.selectedId,
    );
  },

  saveDraft: (draft) => {
    const normalized = normalizeInstructionDraft(draft);
    if (!normalized.name || !normalized.beforeText) {
      publishNotice(
        "error",
        "An instruction needs both a name and before-text instruction.",
      );
      return;
    }

    const instruction: InstructionPreset = {
      ...normalized,
      id: normalized.id ?? createId(),
    };
    set((state) => ({
      library: normalized.id
        ? updateInstruction(state.library, instruction)
        : addInstruction(state.library, instruction),
      editorTarget: null,
    }));
    persistLibrary(get().library);
    publishNotice("success", "Instruction saved");
  },

  deleteInstruction: (id) => {
    set((state) => ({
      library: removeInstruction(state.library, id),
      editorTarget: null,
    }));
    persistLibrary(get().library);
    publishNotice("success", "Instruction deleted");
  },

  openEditor: (target) => set({ editorTarget: target }),
  closeEditor: () => set({ editorTarget: null }),
}));

export function selectedInstructionOf(state: {
  library: InstructionLibrary;
}): InstructionPreset {
  return getSelectedInstruction(state.library);
}

export function initializeInstructionStore(
  instructions: InstructionPreset[],
  selectedId: string | undefined,
): void {
  useInstructionStore.setState({
    library: createInstructionLibrary(instructions, selectedId),
  });
}
