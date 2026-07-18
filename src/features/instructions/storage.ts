import { createDefaultInstructions } from "./defaults";
import {
  INSTRUCTION_COLORS,
  type InstructionColor,
  type InstructionPreset,
} from "./model";

const STORAGE_KEY = "prompter.presets.v1";
const STORAGE_VERSION = 1;

type StoredInstructionLibrary = {
  version: typeof STORAGE_VERSION;
  instructions: InstructionPreset[];
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isInstructionColor(value: unknown): value is InstructionColor {
  return (
    typeof value === "string" &&
    (INSTRUCTION_COLORS as readonly string[]).includes(value)
  );
}

function parseInstruction(value: unknown): InstructionPreset | null {
  if (!isRecord(value)) return null;

  const id = typeof value.id === "string" ? value.id.trim() : "";
  const name = typeof value.name === "string" ? value.name.trim() : "";
  const instruction =
    typeof value.instruction === "string" ? value.instruction.trim() : "";

  if (!id || !name || !instruction || !isInstructionColor(value.color)) {
    return null;
  }

  return { id, name, instruction, color: value.color };
}

function parseInstructionList(value: unknown): InstructionPreset[] {
  if (!Array.isArray(value)) return [];

  const seenIds = new Set<string>();
  const instructions: InstructionPreset[] = [];

  for (const entry of value) {
    const instruction = parseInstruction(entry);
    if (!instruction || seenIds.has(instruction.id)) continue;
    seenIds.add(instruction.id);
    instructions.push(instruction);
  }

  return instructions;
}

export function decodeStoredInstructions(value: unknown): InstructionPreset[] {
  // The first release stored the array directly. Keep that format readable so
  // existing custom instructions migrate without user action.
  if (Array.isArray(value)) return parseInstructionList(value);
  if (!isRecord(value) || value.version !== STORAGE_VERSION) return [];
  return parseInstructionList(value.instructions);
}

export function loadInstructions(): InstructionPreset[] {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (!saved) return createDefaultInstructions();

    const instructions = decodeStoredInstructions(JSON.parse(saved));
    return instructions.length > 0 ? instructions : createDefaultInstructions();
  } catch {
    return createDefaultInstructions();
  }
}

export function saveInstructions(instructions: InstructionPreset[]): boolean {
  const payload: StoredInstructionLibrary = {
    version: STORAGE_VERSION,
    instructions,
  };

  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
    return true;
  } catch {
    return false;
  }
}
