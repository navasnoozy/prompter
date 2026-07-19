import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createId } from "../../shared/ids";
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
import {
  loadInstructions,
  loadSelectedInstructionId,
  saveInstructions,
  saveSelectedInstructionId,
} from "./storage";

type UseInstructionLibraryOptions = {
  onPersistError?: () => void;
};

export function useInstructionLibrary({
  onPersistError,
}: UseInstructionLibraryOptions = {}) {
  const [library, setLibrary] = useState(() =>
    createInstructionLibrary(loadInstructions(), loadSelectedInstructionId()),
  );
  const onPersistErrorRef = useRef(onPersistError);
  useLayoutEffect(() => {
    onPersistErrorRef.current = onPersistError;
  });

  useEffect(() => {
    if (!saveInstructions(library.instructions)) {
      onPersistErrorRef.current?.();
    }
  }, [library.instructions]);

  useEffect(() => {
    saveSelectedInstructionId(library.selectedId);
  }, [library.selectedId]);

  const selectedInstruction = useMemo(
    () => getSelectedInstruction(library),
    [library],
  );

  function saveInstruction(draft: InstructionDraft): InstructionPreset {
    const normalized = normalizeInstructionDraft(draft);
    if (!normalized.name || !normalized.beforeText) {
      throw new Error("An instruction needs both a name and before-text instruction.");
    }

    const instruction: InstructionPreset = {
      ...normalized,
      id: normalized.id ?? createId(),
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
