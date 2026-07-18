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
      : { name: "", instruction: "", color: "rose" },
  );

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft.name.trim() || !draft.instruction.trim()) return;
    onSave(draft);
  }

  return (
    <ModalBackdrop onClose={onClose}>
      <form
        aria-labelledby={TITLE_ID}
        aria-modal="true"
        className="preset-modal"
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
        <label>
          Instruction sent to AI
          <textarea
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                instruction: event.target.value,
              }))
            }
            placeholder="Rewrite the text in a friendly and helpful tone…"
            required
            value={draft.instruction}
          />
        </label>

        <div className="modal-actions">
          {instruction && (
            <button
              className="danger-button"
              disabled={!canDelete}
              onClick={() => onDelete(instruction.id)}
              title={
                canDelete ? undefined : "Prompter needs at least one instruction"
              }
              type="button"
            >
              <Icon name="trash" size={16} /> Delete
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
