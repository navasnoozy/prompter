// @vitest-environment jsdom
import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useInstructionStore } from "../features/instructions/store";
import { useProviderStore } from "../features/providers/store";
import { useSettingsStore } from "../features/settings/store";
import { useAppShortcuts } from "./useAppShortcuts";

const native = vi.hoisted(() => ({
  handler: undefined as ((event: { payload: unknown }) => void) | undefined,
  unlisten: vi.fn(),
  placeCurrentPrompt: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (_event: string, handler: (event: { payload: unknown }) => void) => {
      native.handler = handler;
      return native.unlisten;
    },
  ),
}));

vi.mock("../features/providers/placement", () => ({
  placeCurrentPrompt: native.placeCurrentPrompt,
}));

describe("application shortcuts", () => {
  beforeEach(() => {
    native.handler = undefined;
    native.unlisten.mockReset();
    native.placeCurrentPrompt.mockReset();
    useSettingsStore.setState({ showSettings: false });
    useInstructionStore.setState({ editorTarget: null });
    useProviderStore.setState({ isPlacing: false });
  });

  it("handles native menu shortcuts even without a DOM key event", async () => {
    const setProvider = vi.fn();
    useProviderStore.setState({ setProvider });
    const { unmount } = renderHook(() => useAppShortcuts());
    await waitFor(() => expect(native.handler).toBeTypeOf("function"));

    native.handler?.({
      payload: { version: 1, action: "select_gemini" },
    });
    native.handler?.({ payload: { version: 1, action: "place_prompt" } });

    expect(setProvider).toHaveBeenCalledWith("gemini");
    expect(native.placeCurrentPrompt).toHaveBeenCalledTimes(1);
    unmount();
    expect(native.unlisten).toHaveBeenCalledTimes(1);
  });

  it("ignores malformed events and shortcuts while a dialog is open", async () => {
    const setProvider = vi.fn();
    useProviderStore.setState({ setProvider });
    useSettingsStore.setState({ showSettings: true });
    renderHook(() => useAppShortcuts());
    await waitFor(() => expect(native.handler).toBeTypeOf("function"));

    native.handler?.({ payload: { version: 99, action: "select_gemini" } });
    native.handler?.({
      payload: { version: 1, action: "select_gemini" },
    });

    expect(setProvider).not.toHaveBeenCalled();
  });
});
