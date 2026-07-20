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
});
