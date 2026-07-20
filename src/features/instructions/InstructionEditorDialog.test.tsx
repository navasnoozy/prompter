// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { InstructionEditorDialog } from "./InstructionEditorDialog";
import { useInstructionStore } from "./store";

const defaultSaveDraft = useInstructionStore.getState().saveDraft;

function DialogHarness() {
  return (
    <>
      <button
        onClick={() => useInstructionStore.getState().openEditor("new")}
        type="button"
      >
        Create instruction
      </button>
      <InstructionEditorDialog />
    </>
  );
}

describe("InstructionEditorDialog", () => {
  afterEach(() => {
    cleanup();
    useInstructionStore.setState({
      editorTarget: null,
      saveDraft: defaultSaveDraft,
    });
  });

  it("reports whitespace-only required fields inside the dialog", () => {
    const saveDraft = vi.fn();
    useInstructionStore.setState({ editorTarget: "new", saveDraft });
    render(<InstructionEditorDialog />);

    fireEvent.change(screen.getByLabelText("Name"), {
      target: { value: "   " },
    });
    fireEvent.change(screen.getByLabelText("Instruction before text"), {
      target: { value: "\n  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save instruction" }));

    expect(screen.getByRole("alert").textContent).toContain(
      "Enter a name and an instruction",
    );
    expect(screen.getByLabelText("Name").getAttribute("aria-invalid")).toBe(
      "true",
    );
    expect(saveDraft).not.toHaveBeenCalled();
  });

  it("restores focus to the opener when cancelled", () => {
    render(<DialogHarness />);
    const opener = screen.getByRole("button", { name: "Create instruction" });
    opener.focus();
    fireEvent.click(opener);

    expect(document.activeElement).toBe(screen.getByLabelText("Name"));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(document.activeElement).toBe(opener);
  });

  it("restores focus to the opener after a successful save", () => {
    const saveDraft = vi.fn(() => {
      useInstructionStore.setState({ editorTarget: null });
    });
    useInstructionStore.setState({ saveDraft });
    render(<DialogHarness />);
    const opener = screen.getByRole("button", { name: "Create instruction" });
    opener.focus();
    fireEvent.click(opener);
    fireEvent.change(screen.getByLabelText("Name"), {
      target: { value: "Focused" },
    });
    fireEvent.change(screen.getByLabelText("Instruction before text"), {
      target: { value: "Rewrite clearly" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Save instruction" }));

    expect(saveDraft).toHaveBeenCalledOnce();
    expect(document.activeElement).toBe(opener);
  });
});
