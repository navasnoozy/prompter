import { useEffect, type ReactNode } from "react";

type ModalBackdropProps = {
  children: ReactNode;
  onClose: () => void;
};

export function ModalBackdrop({ children, onClose }: ModalBackdropProps) {
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  return (
    <div className="modal-backdrop" role="presentation">
      {children}
    </div>
  );
}
