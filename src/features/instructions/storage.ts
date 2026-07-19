import { isRecord } from "../../shared/contracts";
import { createDefaultInstructions } from "./defaults";
import {
  INSTRUCTION_COLORS,
  type InstructionColor,
  type InstructionPreset,
} from "./model";

const STORAGE_KEY = "prompter.presets.v1";
const SELECTION_KEY = "prompter.selection.v1";
const STORAGE_VERSION = 2;

type StoredInstructionLibrary = {
  version: typeof STORAGE_VERSION;
  instructions: InstructionPreset[];
};

function isInstructionColor(value: unknown): value is InstructionColor {
  return (
    typeof value === "string" &&
    (INSTRUCTION_COLORS as readonly string[]).includes(value)
  );
}

function parseBaseFields(value: Record<string, unknown>) {
  const id = typeof value.id === "string" ? value.id.trim() : "";
  const name = typeof value.name === "string" ? value.name.trim() : "";

  if (!id || !name || !isInstructionColor(value.color)) return null;
  return { id, name, color: value.color };
}

function parseVersionTwoInstruction(
  value: unknown,
): InstructionPreset | null {
  if (!isRecord(value)) return null;

  const base = parseBaseFields(value);
  if (
    typeof value.beforeText !== "string" ||
    (value.afterText !== undefined && typeof value.afterText !== "string")
  ) {
    return null;
  }
  const beforeText =
    value.beforeText.trim();
  const afterText =
    typeof value.afterText === "string" ? value.afterText.trim() : "";

  if (!base || !beforeText) return null;
  return { ...base, beforeText, afterText };
}

function migrateLegacyInstruction(value: unknown): InstructionPreset | null {
  if (!isRecord(value)) return null;

  const base = parseBaseFields(value);
  const beforeText =
    typeof value.instruction === "string" ? value.instruction.trim() : "";

  if (!base || !beforeText) return null;
  return { ...base, beforeText, afterText: "" };
}

function parseInstructionList(
  value: unknown,
  parseEntry: (entry: unknown) => InstructionPreset | null,
): InstructionPreset[] {
  if (!Array.isArray(value)) return [];

  const seenIds = new Set<string>();
  const instructions: InstructionPreset[] = [];

  for (const entry of value) {
    const instruction = parseEntry(entry);
    if (!instruction || seenIds.has(instruction.id)) continue;
    seenIds.add(instruction.id);
    instructions.push(instruction);
  }

  return instructions;
}

export function decodeStoredInstructions(value: unknown): InstructionPreset[] {
  // The first release stored its instruction array directly.
  if (Array.isArray(value)) {
    return parseInstructionList(value, migrateLegacyInstruction);
  }
  if (!isRecord(value)) return [];

  if (value.version === 1) {
    return parseInstructionList(
      value.instructions ?? value.presets,
      migrateLegacyInstruction,
    );
  }

  if (value.version === STORAGE_VERSION) {
    return parseInstructionList(
      value.instructions,
      parseVersionTwoInstruction,
    );
  }

  return [];
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

export function loadSelectedInstructionId(): string | undefined {
  try {
    return localStorage.getItem(SELECTION_KEY) ?? undefined;
  } catch {
    return undefined;
  }
}

export function saveSelectedInstructionId(id: string): void {
  try {
    localStorage.setItem(SELECTION_KEY, id);
  } catch {
    // Selection persistence is optional; the session keeps the choice.
  }
}
