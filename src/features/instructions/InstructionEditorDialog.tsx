import { useState, type FormEvent } from "react";
import { Icon } from "../../shared/Icon";
import { ModalBackdrop } from "../../shared/ModalBackdrop";
import type { InstructionDraft, InstructionPreset } from "./model";

const TITLE_ID = "instruction-editor-title";

type InstructionEditorDialogProps = {
  canDelete: boolean;
  instruction: InstructionPreset | null;
  onClose: () => void;
  onDelete: (id: string) => void;
  onSave: (draft: InstructionDraft) => void;
};

export function InstructionEditorDialog({
  canDelete,
  instruction,
  onClose,
  onDelete,
  onSave,
}: InstructionEditorDialogProps) {
  const [draft, setDraft] = useState<InstructionDraft>(() =>
    instruction
      ? { ...instruction }
      : { name: "", beforeText: "", afterText: "", color: "rose" },
  );
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft.name.trim() || !draft.beforeText.trim()) return;
    onSave(draft);
  }

  return (
    <ModalBackdrop onClose={onClose}>
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
            onClick={onClose}
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
                  ? onDelete(instruction.id)
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
            onClick={onClose}
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
