import { useEffect, useRef, type ReactNode } from "react";

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), textarea, input, select, [tabindex]:not([tabindex="-1"])';

type ModalBackdropProps = {
  children: ReactNode;
  onClose: () => void;
};

export function ModalBackdrop({ children, onClose }: ModalBackdropProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  // Capture during render, before React commits a child's autoFocus. Reading
  // this in an effect would record the dialog input instead of its opener.
  const previouslyFocusedRef = useRef<HTMLElement | null>(
    typeof document !== "undefined" && document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );

  useEffect(() => {
    const container = containerRef.current;
    const previouslyFocused = previouslyFocusedRef.current;

    const focusableElements = () =>
      Array.from(
        container?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR) ?? [],
      );

    // React applies autoFocus before effects run; only move focus into the
    // dialog when nothing inside it holds focus yet.
    if (container && !container.contains(document.activeElement)) {
      focusableElements()[0]?.focus();
    }

    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
        return;
      }
      if (event.key !== "Tab") return;

      const elements = focusableElements();
      if (elements.length === 0) return;

      const first = elements[0];
      const last = elements[elements.length - 1];
      const active = document.activeElement;
      const activeInside =
        active instanceof HTMLElement && container?.contains(active);

      if (event.shiftKey) {
        if (!activeInside || active === first) {
          event.preventDefault();
          last.focus();
        }
      } else if (!activeInside || active === last) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", handleKeydown);
    return () => {
      window.removeEventListener("keydown", handleKeydown);
      if (previouslyFocused?.isConnected) previouslyFocused.focus();
    };
  }, [onClose]);

  return (
    <div className="modal-backdrop" ref={containerRef} role="presentation">
      {children}
    </div>
  );
}
