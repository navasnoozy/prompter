import { useEffect, useMemo, useState } from "react";
import {
  addInstruction,
  createInstructionLibrary,
  getSelectedInstruction,
  removeInstruction,
  selectInstruction,
  updateInstruction,
} from "./collection";
import type { InstructionDraft, InstructionPreset } from "./model";
import { normalizeInstructionDraft } from "./model";
import { loadInstructions, saveInstructions } from "./storage";

export function useInstructionLibrary() {
  const [library, setLibrary] = useState(() =>
    createInstructionLibrary(loadInstructions()),
  );

  useEffect(() => {
    saveInstructions(library.instructions);
  }, [library.instructions]);

  const selectedInstruction = useMemo(
    () => getSelectedInstruction(library),
    [library],
  );

  function saveInstruction(draft: InstructionDraft): InstructionPreset {
    const normalized = normalizeInstructionDraft(draft);
    if (!normalized.name || !normalized.instruction) {
      throw new Error("An instruction needs both a name and AI instruction.");
    }

    const instruction: InstructionPreset = {
      ...normalized,
      id: normalized.id ?? crypto.randomUUID(),
    };

    setLibrary((current) =>
      normalized.id
        ? updateInstruction(current, instruction)
        : addInstruction(current, instruction),
    );

    return instruction;
  }

  return {
    instructions: library.instructions,
    selectedId: library.selectedId,
    selectedInstruction,
    selectInstruction: (id: string) =>
      setLibrary((current) => selectInstruction(current, id)),
    saveInstruction,
    deleteInstruction: (id: string) =>
      setLibrary((current) => removeInstruction(current, id)),
  };
}
