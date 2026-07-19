import { describe, expect, it } from "vitest";
import { decodeStoredInstructions } from "./storage";

const legacyInstruction = {
  id: "friendly",
  name: " Friendly email ",
  instruction: " Rewrite this warmly. ",
  color: "rose",
};

const versionTwoInstruction = {
  id: "concise",
  name: " Concise ",
  beforeText: " Make this shorter. ",
  afterText: " Return only the result. ",
  color: "green",
};

describe("instruction storage decoder", () => {
  it("migrates the original raw-array format into before and after fields", () => {
    expect(decodeStoredInstructions([legacyInstruction])).toEqual([
      {
        id: "friendly",
        name: "Friendly email",
        beforeText: "Rewrite this warmly.",
        afterText: "",
        color: "rose",
      },
    ]);
  });

  it("migrates version-one objects without losing custom instructions", () => {
    expect(
      decodeStoredInstructions({
        version: 1,
        instructions: [legacyInstruction],
      }),
    ).toEqual([
      {
        id: "friendly",
        name: "Friendly email",
        beforeText: "Rewrite this warmly.",
        afterText: "",
        color: "rose",
      },
    ]);
  });

  it("reads version two, trims fields, and accepts an empty after instruction", () => {
    expect(
      decodeStoredInstructions({
        version: 2,
        instructions: [
          versionTwoInstruction,
          {
            ...versionTwoInstruction,
            id: "no-after",
            name: "No after",
            afterText: "   ",
          },
        ],
      }),
    ).toEqual([
      {
        id: "concise",
        name: "Concise",
        beforeText: "Make this shorter.",
        afterText: "Return only the result.",
        color: "green",
      },
      {
        id: "no-after",
        name: "No after",
        beforeText: "Make this shorter.",
        afterText: "",
        color: "green",
      },
    ]);
  });

  it("drops malformed version-two entries and duplicate identifiers", () => {
    expect(
      decodeStoredInstructions({
        version: 2,
        instructions: [
          versionTwoInstruction,
          { ...versionTwoInstruction, name: "Duplicate" },
          { ...versionTwoInstruction, id: "empty-before", beforeText: " " },
          { ...versionTwoInstruction, id: "bad-after", afterText: 42 },
          { ...versionTwoInstruction, id: "bad-color", color: "red" },
          null,
        ],
      }),
    ).toHaveLength(1);
  });

  it("rejects unknown storage versions and invalid roots", () => {
    expect(
      decodeStoredInstructions({
        version: 3,
        instructions: [versionTwoInstruction],
      }),
    ).toEqual([]);
    expect(decodeStoredInstructions("invalid")).toEqual([]);
    expect(decodeStoredInstructions(null)).toEqual([]);
  });
});
