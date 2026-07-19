import { isRecord } from "../../shared/contracts";
import {
  INSTRUCTION_COLORS,
  type InstructionColor,
  type InstructionPreset,
} from "./model";

const STORAGE_VERSION = 2;

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

function parseVersionTwoInstruction(value: unknown): InstructionPreset | null {
  if (!isRecord(value)) return null;

  const base = parseBaseFields(value);
  if (
    typeof value.beforeText !== "string" ||
    (value.afterText !== undefined && typeof value.afterText !== "string")
  ) {
    return null;
  }
  const beforeText = value.beforeText.trim();
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

// Tolerant decoder for every payload shape Prompter has ever persisted:
// the original raw array, the version-1 object, and the current version 2.
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
    return parseInstructionList(value.instructions, parseVersionTwoInstruction);
  }

  return [];
}
