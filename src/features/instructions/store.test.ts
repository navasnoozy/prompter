import { beforeEach, describe, expect, it, vi } from "vitest";
import { settingsGateway } from "../../shared/settingsGateway";
import { publishNotice, useNoticeStore } from "../../shared/notices";
import type { InstructionPreset } from "./model";
import { initializeInstructionStore, useInstructionStore } from "./store";

vi.mock("../../shared/settingsGateway", () => ({
  SETTINGS_KEYS: {
    presets: "presets",
    selectedInstructionId: "selectedInstructionId",
    theme: "theme",
    provider: "provider",
  },
  settingsGateway: {
    read: vi.fn(),
    write: vi.fn().mockResolvedValue(true),
    writeMany: vi.fn().mockResolvedValue(true),
  },
}));

const BASE: InstructionPreset = {
  id: "base",
  name: "Base",
  beforeText: "Rewrite clearly",
  afterText: "",
  color: "violet",
};

function libraryState() {
  return useInstructionStore.getState().library;
}

describe("instruction store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    initializeInstructionStore([BASE], "base");
    useInstructionStore.setState({ editorTarget: null });
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
  });

  it("announces a save only after the durable write succeeds", async () => {
    useInstructionStore.getState().openEditor("new");

    useInstructionStore.getState().saveDraft({
      name: " Friendly ",
      beforeText: " Rewrite warmly ",
      afterText: "",
      color: "rose",
    });

    const library = libraryState();
    expect(library.instructions).toHaveLength(2);
    const saved = library.instructions[1];
    expect(saved.name).toBe("Friendly");
    expect(library.selectedId).toBe(saved.id);
    expect(useInstructionStore.getState().editorTarget).toBeNull();
    expect(settingsGateway.writeMany).toHaveBeenCalledWith({
      presets: { version: 2, instructions: library.instructions },
      selectedInstructionId: saved.id,
    });
    expect(useNoticeStore.getState().notice.kind).toBe("progress");
    await vi.waitFor(() =>
      expect(useNoticeStore.getState().notice).toMatchObject({
        kind: "success",
        message: "Instruction saved",
      }),
    );
  });

  it("does not let an older save success replace a newer persistence error", async () => {
    let resolveWrite: ((saved: boolean) => void) | undefined;
    vi.mocked(settingsGateway.writeMany).mockReturnValueOnce(
      new Promise<boolean>((resolve) => {
        resolveWrite = resolve;
      }),
    );

    useInstructionStore.getState().saveDraft({
      name: "Deferred",
      beforeText: "Rewrite clearly",
      afterText: "",
      color: "blue",
    });
    expect(useNoticeStore.getState().notice.kind).toBe("progress");
    publishNotice("error", "A newer settings write failed");

    resolveWrite?.(true);
    await vi.waitFor(() =>
      expect(useNoticeStore.getState().notice).toMatchObject({
        kind: "error",
        message: "A newer settings write failed",
      }),
    );
  });

  it("rejects drafts without a name or before-text and keeps the editor open", () => {
    useInstructionStore.getState().openEditor("new");

    useInstructionStore.getState().saveDraft({
      name: "  ",
      beforeText: "Something",
      afterText: "",
      color: "blue",
    });

    expect(libraryState().instructions).toHaveLength(1);
    expect(useInstructionStore.getState().editorTarget).toBe("new");
    expect(useNoticeStore.getState().notice.kind).toBe("error");
    expect(settingsGateway.writeMany).not.toHaveBeenCalled();
  });

  it("updates an existing instruction in place", () => {
    useInstructionStore.getState().saveDraft({
      id: "base",
      name: "Renamed",
      beforeText: "New before",
      afterText: "New after",
      color: "green",
    });

    expect(libraryState().instructions).toEqual([
      {
        id: "base",
        name: "Renamed",
        beforeText: "New before",
        afterText: "New after",
        color: "green",
      },
    ]);
  });

  it("never deletes the last remaining instruction", () => {
    useInstructionStore.getState().deleteInstruction("base");

    expect(libraryState().instructions).toHaveLength(1);
  });

  it("persists selection changes", () => {
    initializeInstructionStore(
      [BASE, { ...BASE, id: "second", name: "Second" }],
      "base",
    );

    useInstructionStore.getState().select("second");

    expect(libraryState().selectedId).toBe("second");
    expect(settingsGateway.write).toHaveBeenCalledWith(
      "selectedInstructionId",
      "second",
    );
  });
});
