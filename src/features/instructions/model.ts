export const INSTRUCTION_COLORS = [
  "violet",
  "blue",
  "amber",
  "green",
  "rose",
] as const;

export type InstructionColor = (typeof INSTRUCTION_COLORS)[number];

export type InstructionPreset = {
  id: string;
  name: string;
  instruction: string;
  color: InstructionColor;
};

export type InstructionDraft = Omit<InstructionPreset, "id"> & {
  id?: string;
};

export type InstructionLibrary = {
  instructions: InstructionPreset[];
  selectedId: string;
};

export function normalizeInstructionDraft(
  draft: InstructionDraft,
): InstructionDraft {
  return {
    ...draft,
    id: draft.id?.trim() || undefined,
    name: draft.name.trim(),
    instruction: draft.instruction.trim(),
  };
}
