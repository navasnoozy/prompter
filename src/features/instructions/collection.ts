import { createDefaultInstructions } from "./defaults";
import type { InstructionLibrary, InstructionPreset } from "./model";

export function createInstructionLibrary(
  instructions: InstructionPreset[],
  selectedId?: string,
): InstructionLibrary {
  const safeInstructions =
    instructions.length > 0 ? instructions : createDefaultInstructions();
  const validSelection = safeInstructions.some(
    (instruction) => instruction.id === selectedId,
  );

  return {
    instructions: safeInstructions,
    selectedId:
      validSelection && selectedId ? selectedId : safeInstructions[0].id,
  };
}

export function selectInstruction(
  library: InstructionLibrary,
  id: string,
): InstructionLibrary {
  if (!library.instructions.some((instruction) => instruction.id === id)) {
    return library;
  }
  return { ...library, selectedId: id };
}

export function addInstruction(
  library: InstructionLibrary,
  instruction: InstructionPreset,
): InstructionLibrary {
  return {
    instructions: [...library.instructions, instruction],
    selectedId: instruction.id,
  };
}

export function updateInstruction(
  library: InstructionLibrary,
  instruction: InstructionPreset,
): InstructionLibrary {
  if (!library.instructions.some((current) => current.id === instruction.id)) {
    return library;
  }

  return {
    instructions: library.instructions.map((current) =>
      current.id === instruction.id ? instruction : current,
    ),
    selectedId: instruction.id,
  };
}

export function removeInstruction(
  library: InstructionLibrary,
  id: string,
): InstructionLibrary {
  if (library.instructions.length <= 1) return library;

  const instructions = library.instructions.filter(
    (instruction) => instruction.id !== id,
  );
  if (instructions.length === library.instructions.length) return library;

  return createInstructionLibrary(instructions, library.selectedId);
}

export function getSelectedInstruction(
  library: InstructionLibrary,
): InstructionPreset {
  return (
    library.instructions.find(
      (instruction) => instruction.id === library.selectedId,
    ) ?? library.instructions[0]
  );
}
