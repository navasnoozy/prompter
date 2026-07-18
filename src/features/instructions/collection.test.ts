import { describe, expect, it } from "vitest";
import {
  addInstruction,
  createInstructionLibrary,
  getSelectedInstruction,
  removeInstruction,
  selectInstruction,
  updateInstruction,
} from "./collection";
import type { InstructionPreset } from "./model";

const first: InstructionPreset = {
  id: "first",
  name: "First",
  instruction: "First instruction",
  color: "violet",
};

const second: InstructionPreset = {
  id: "second",
  name: "Second",
  instruction: "Second instruction",
  color: "blue",
};

describe("instruction collection", () => {
  it("always creates a valid selection", () => {
    const library = createInstructionLibrary([first, second], "missing");

    expect(library.selectedId).toBe(first.id);
    expect(getSelectedInstruction(library)).toEqual(first);
  });

  it("ignores an unknown selection", () => {
    const library = createInstructionLibrary([first, second], first.id);

    expect(selectInstruction(library, "missing")).toBe(library);
  });

  it("selects newly added and updated instructions", () => {
    const initial = createInstructionLibrary([first]);
    const added = addInstruction(initial, second);
    const updatedSecond = { ...second, name: "Updated" };
    const updated = updateInstruction(added, updatedSecond);

    expect(added.selectedId).toBe(second.id);
    expect(getSelectedInstruction(updated)).toEqual(updatedSecond);
  });

  it("moves selection safely when the selected instruction is deleted", () => {
    const library = createInstructionLibrary([first, second], second.id);
    const next = removeInstruction(library, second.id);

    expect(next.instructions).toEqual([first]);
    expect(next.selectedId).toBe(first.id);
  });

  it("never removes the final instruction", () => {
    const library = createInstructionLibrary([first]);

    expect(removeInstruction(library, first.id)).toBe(library);
  });
});
