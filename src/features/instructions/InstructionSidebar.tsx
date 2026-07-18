import { Icon } from "../../shared/Icon";
import type { InstructionPreset } from "./model";

type InstructionSidebarProps = {
  instructions: InstructionPreset[];
  selectedId: string;
  onCreate: () => void;
  onEdit: (instruction: InstructionPreset) => void;
  onOpenSettings: () => void;
  onSelect: (id: string) => void;
};

export function InstructionSidebar({
  instructions,
  selectedId,
  onCreate,
  onEdit,
  onOpenSettings,
  onSelect,
}: InstructionSidebarProps) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">
          <Icon name="sparkle" size={19} />
        </span>
        <span>Prompter</span>
      </div>

      <section className="sidebar-instructions" aria-label="Instructions">
        <div className="sidebar-section-heading">
          <span>Instructions</span>
          <button
            aria-label="Create instruction"
            onClick={onCreate}
            title="Create instruction"
            type="button"
          >
            <Icon name="plus" size={15} />
          </button>
        </div>

        <div className="preset-list sidebar-preset-list">
          {instructions.map((instruction) => (
            <div
              className={`preset-row ${selectedId === instruction.id ? "selected" : ""}`}
              key={instruction.id}
            >
              <button
                className="preset-main"
                onClick={() => onSelect(instruction.id)}
                type="button"
              >
                <span className={`preset-icon ${instruction.color}`}>
                  <Icon name="wand" size={16} />
                </span>
                <span>
                  <strong>{instruction.name}</strong>
                </span>
              </button>
              <button
                aria-label={`Edit ${instruction.name}`}
                className="edit-preset"
                onClick={() => onEdit(instruction)}
                type="button"
              >
                <Icon name="edit" size={14} />
              </button>
            </div>
          ))}
        </div>
      </section>

      <div className="sidebar-tip">
        <span className="tip-kicker">Quick capture</span>
        <strong>Select text, then press</strong>
        <div className="shortcut-row">
          <kbd>⌘</kbd>
          <kbd>⇧</kbd>
          <kbd>P</kbd>
        </div>
        <small>Select text first. Prompter copies it automatically.</small>
      </div>

      <button
        className="nav-item settings-link"
        onClick={onOpenSettings}
        type="button"
      >
        <Icon name="settings" /> Settings
      </button>
    </aside>
  );
}
