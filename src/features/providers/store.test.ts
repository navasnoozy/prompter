import { beforeEach, describe, expect, it, vi } from "vitest";
import { settingsGateway } from "../../shared/settingsGateway";
import { initializeProviderStore, useProviderStore } from "./store";

vi.mock("../../shared/settingsGateway", () => ({
  SETTINGS_KEYS: { provider: "provider" },
  settingsGateway: { write: vi.fn().mockResolvedValue(true) },
}));

describe("provider store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    initializeProviderStore("chatgpt");
    useProviderStore.setState({ panelOpen: true });
  });

  it("keeps the ready panel open when the active provider is reselected", () => {
    useProviderStore.getState().setProvider("chatgpt");

    expect(useProviderStore.getState()).toMatchObject({
      provider: "chatgpt",
      panelOpen: true,
    });
    expect(settingsGateway.write).not.toHaveBeenCalled();
  });

  it("closes readiness and persists when switching providers", () => {
    useProviderStore.getState().setProvider("gemini");

    expect(useProviderStore.getState()).toMatchObject({
      provider: "gemini",
      panelOpen: false,
    });
    expect(settingsGateway.write).toHaveBeenCalledWith("provider", "gemini");
  });

  it("isolates navigation state by provider and rejects stale updates", () => {
    const update = useProviderStore.getState().updateNavigationState;
    update({
      version: 1,
      provider: "chatgpt",
      generation: 3,
      revision: 2,
      available: true,
      canGoBack: true,
      canGoForward: false,
      isLoading: false,
    });
    update({
      version: 1,
      provider: "gemini",
      generation: 2,
      revision: 7,
      available: true,
      canGoBack: false,
      canGoForward: true,
      isLoading: true,
    });
    update({
      version: 1,
      provider: "chatgpt",
      generation: 3,
      revision: 1,
      available: false,
      canGoBack: false,
      canGoForward: false,
      isLoading: false,
    });

    expect(useProviderStore.getState().navigationByProvider).toMatchObject({
      chatgpt: {
        generation: 3,
        revision: 2,
        available: true,
        canGoBack: true,
      },
      gemini: {
        generation: 2,
        revision: 7,
        available: true,
        canGoForward: true,
        isLoading: true,
      },
    });
  });

  it("accepts a replacement generation even when its revision is lower", () => {
    const update = useProviderStore.getState().updateNavigationState;
    update({
      version: 1,
      provider: "chatgpt",
      generation: 4,
      revision: 12,
      available: false,
      canGoBack: false,
      canGoForward: false,
      isLoading: false,
    });
    update({
      version: 1,
      provider: "chatgpt",
      generation: 5,
      revision: 1,
      available: true,
      canGoBack: false,
      canGoForward: false,
      isLoading: true,
    });

    expect(
      useProviderStore.getState().navigationByProvider.chatgpt,
    ).toMatchObject({
      generation: 5,
      revision: 1,
      available: true,
      isLoading: true,
    });
  });
});
