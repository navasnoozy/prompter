import { describe, expect, it } from "vitest";
import { decodeStoredInstructions } from "./storage";

const validInstruction = {
  id: "friendly",
  name: " Friendly email ",
  instruction: " Rewrite this warmly. ",
  color: "rose",
};

describe("instruction storage decoder", () => {
  it("migrates the original array format and trims values", () => {
    expect(decodeStoredInstructions([validInstruction])).toEqual([
      {
        id: "friendly",
        name: "Friendly email",
        instruction: "Rewrite this warmly.",
        color: "rose",
      },
    ]);
  });

  it("reads the current versioned format", () => {
    expect(
      decodeStoredInstructions({
        version: 1,
        instructions: [validInstruction],
      }),
    ).toHaveLength(1);
  });

  it("drops malformed entries and duplicate identifiers", () => {
    expect(
      decodeStoredInstructions({
        version: 1,
        instructions: [
          validInstruction,
          { ...validInstruction, name: "Duplicate" },
          { id: "empty", name: "", instruction: "Text", color: "blue" },
          { id: "bad-color", name: "Bad", instruction: "Text", color: "red" },
          null,
        ],
      }),
    ).toHaveLength(1);
  });

  it("rejects unknown storage versions and invalid roots", () => {
    expect(
      decodeStoredInstructions({ version: 2, instructions: [validInstruction] }),
    ).toEqual([]);
    expect(decodeStoredInstructions("invalid")).toEqual([]);
    expect(decodeStoredInstructions(null)).toEqual([]);
  });
});
