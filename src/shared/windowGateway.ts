import { invoke } from "@tauri-apps/api/core";
import { publishNotice } from "./notices";

/**
 * Mirrors the window permissions granted in
 * `src-tauri/capabilities/default.json`. A command missing there is rejected
 * by the ACL at runtime, so the two lists move together.
 */
export const WINDOW_COMMANDS = {
  startDragging: "plugin:window|start_dragging",
} as const;

/**
 * A drag that cannot start has no recovery and no retry, and the handler runs
 * on every press — so the first failure is worth reporting and the rest would
 * only be noise.
 */
let dragErrorReported = false;

/**
 * Hands a press to AppKit's own window-drag loop.
 *
 * Tauri injects a `data-tauri-drag-region` handler of its own, but it ignores
 * any mousedown whose click count is not 1 or 2, and on macOS it deliberately
 * skips the drag at 2 so it can watch for a double-click to zoom. Releasing
 * and grabbing again inside the system double-click interval therefore does
 * nothing at all — which is exactly the motion someone makes when nudging a
 * window into place, and it reads as the drag region being broken.
 *
 * Driving `start_dragging` from every primary press avoids that entirely.
 * `performWindowDragWithEvent:` runs the drag loop natively and applies the
 * viewer's own "double-click a window's title bar to" system setting, rather
 * than the fixed zoom Tauri's script hard-codes.
 */
export function beginWindowDrag(): void {
  void invoke(WINDOW_COMMANDS.startDragging).catch((error: unknown) => {
    if (dragErrorReported) return;
    dragErrorReported = true;
    publishNotice(
      "error",
      `The window could not be dragged: ${String(error)}`,
    );
  });
}
