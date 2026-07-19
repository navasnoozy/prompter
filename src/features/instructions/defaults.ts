import type { InstructionPreset } from "./model";

export const DEFAULT_INSTRUCTIONS = [
  {
    id: "clearer",
    name: "Make it clearer",
    beforeText:
      "Rewrite the following text so it is clear, easy to understand, and well structured. Keep the original meaning.",
    afterText: "",
    color: "violet",
  },
  {
    id: "grammar",
    name: "Fix grammar",
    beforeText:
      "Correct the grammar, spelling, and punctuation in the following text. Do not change its meaning or tone.",
    afterText: "",
    color: "blue",
  },
  {
    id: "professional",
    name: "Professional tone",
    beforeText:
      "Rewrite the following text in a confident, polished, and professional tone. Keep it natural and concise.",
    afterText: "",
    color: "amber",
  },
  {
    id: "concise",
    name: "Make it concise",
    beforeText:
      "Rewrite the following text using fewer words. Remove repetition and unnecessary details while preserving all important information.",
    afterText: "",
    color: "green",
  },
] as const satisfies readonly InstructionPreset[];

export function createDefaultInstructions(): InstructionPreset[] {
  return DEFAULT_INSTRUCTIONS.map((instruction) => ({ ...instruction }));
}
