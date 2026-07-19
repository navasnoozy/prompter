import { useState, type FormEvent } from "react";
import { Icon } from "../../shared/Icon";
import { ModalBackdrop } from "../../shared/ModalBackdrop";
import type { InstructionDraft } from "./model";
import { useInstructionStore } from "./store";

const TITLE_ID = "instruction-editor-title";

export function InstructionEditorDialog() {
  const editorTarget = useInstructionStore((state) => state.editorTarget);
  const canDelete = useInstructionStore(
    (state) => state.library.instructions.length > 1,
  );
  const saveDraft = useInstructionStore((state) => state.saveDraft);
  const deleteInstruction = useInstructionStore(
    (state) => state.deleteInstruction,
  );
  const closeEditor = useInstructionStore((state) => state.closeEditor);

  const instruction = editorTarget === "new" ? null : editorTarget;
  const [draft, setDraft] = useState<InstructionDraft>(() =>
    instruction
      ? { ...instruction }
      : { name: "", beforeText: "", afterText: "", color: "rose" },
  );
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  if (editorTarget === null) return null;

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft.name.trim() || !draft.beforeText.trim()) return;
    saveDraft(draft);
  }

  return (
    <ModalBackdrop onClose={closeEditor}>
      <form
        aria-labelledby={TITLE_ID}
        aria-modal="true"
        className="preset-modal instruction-editor-modal"
        onSubmit={submit}
        role="dialog"
      >
        <div className="modal-header">
          <div>
            <p className="eyebrow">Custom behaviour</p>
            <h2 id={TITLE_ID}>
              {instruction ? "Edit instruction" : "Create instruction"}
            </h2>
          </div>
          <button
            aria-label="Close"
            className="icon-button"
            onClick={closeEditor}
            type="button"
          >
            <Icon name="close" />
          </button>
        </div>

        <label>
          Name
          <input
            autoFocus
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                name: event.target.value,
              }))
            }
            placeholder="Example: Friendly email"
            required
            value={draft.name}
          />
        </label>
        <p className="instruction-editor-helper">
          Prompter places your text between these instructions.
        </p>
        <label>
          Instruction before text
          <textarea
            className="instruction-before-text"
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                beforeText: event.target.value,
              }))
            }
            placeholder="Rewrite the text in a friendly and helpful tone…"
            required
            value={draft.beforeText}
          />
        </label>
        <label>
          Instruction after text (optional)
          <textarea
            className="instruction-after-text"
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                afterText: event.target.value,
              }))
            }
            placeholder="Example: Return only the rewritten text."
            value={draft.afterText}
          />
        </label>

        <div className="modal-actions">
          {instruction && (
            <button
              className="danger-button"
              disabled={!canDelete}
              onClick={() =>
                confirmingDelete
                  ? deleteInstruction(instruction.id)
                  : setConfirmingDelete(true)
              }
              title={
                canDelete ? undefined : "Prompter needs at least one instruction"
              }
              type="button"
            >
              <Icon name="trash" size={16} />{" "}
              {confirmingDelete ? "Confirm delete" : "Delete"}
            </button>
          )}
          <span />
          <button
            className="secondary-button"
            onClick={closeEditor}
            type="button"
          >
            Cancel
          </button>
          <button className="primary-button" type="submit">
            Save instruction
          </button>
        </div>
      </form>
    </ModalBackdrop>
  );
}
