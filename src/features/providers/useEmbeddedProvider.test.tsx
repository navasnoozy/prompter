// @vitest-environment jsdom
import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useInstructionStore } from "../instructions/store";
import { useLifecycleStore } from "../lifecycle/store";
import { useSettingsStore } from "../settings/store";
import { providerGateway } from "./gateway";
import { useProviderStore } from "./store";
import { useEmbeddedProvider } from "./useEmbeddedProvider";

vi.mock("./gateway", async (importOriginal) => {
  const original = await importOriginal<typeof import("./gateway")>();
  return {
    ...original,
    providerGateway: {
      ...original.providerGateway,
      show: vi.fn(),
      resize: vi.fn(),
      setVisibility: vi.fn(),
    },
  };
});

function Harness() {
  const { hostRef } = useEmbeddedProvider();
  return <div ref={hostRef} />;
}

describe("useEmbeddedProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(providerGateway.show).mockResolvedValue(undefined);
    vi.mocked(providerGateway.resize).mockResolvedValue(undefined);
    vi.mocked(providerGateway.setVisibility).mockResolvedValue(undefined);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      bottom: 520,
      height: 500,
      left: 10,
      right: 610,
      top: 20,
      width: 600,
      x: 10,
      y: 20,
      toJSON: () => ({}),
    });
    vi.stubGlobal("ResizeObserver", undefined);
    // An occluded or tray-hidden window stops being served animation frames.
    // Every test here runs under that condition, because placement must not
    // depend on a callback the platform is free to withhold.
    vi.stubGlobal(
      "requestAnimationFrame",
      vi.fn(() => 1),
    );
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    useLifecycleStore.setState({
      status: {
        version: 1,
        launchAtLogin: "disabled",
        mainWindowVisible: true,
      },
    });
    useInstructionStore.setState({ editorTarget: null });
    useSettingsStore.setState({ showSettings: false });
    useProviderStore.setState({ provider: "chatgpt", panelOpen: false });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("uses window resize events when ResizeObserver is unavailable", async () => {
    const view = render(<Harness />);
    await waitFor(() => expect(providerGateway.show).toHaveBeenCalledOnce());

    window.dispatchEvent(new Event("resize"));

    await waitFor(() =>
      expect(providerGateway.resize).toHaveBeenCalledWith("chatgpt", {
        x: 10,
        y: 20,
        width: 600,
        height: 500,
      }),
    );
    view.unmount();
  });

  it("reports layout changes even when animation frames are never served", async () => {
    const view = render(<Harness />);
    await waitFor(() => expect(providerGateway.show).toHaveBeenCalledOnce());

    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      bottom: 800,
      height: 700,
      left: 10,
      right: 1210,
      top: 20,
      width: 1200,
      x: 10,
      y: 20,
      toJSON: () => ({}),
    });
    window.dispatchEvent(new Event("resize"));

    await waitFor(() =>
      expect(providerGateway.resize).toHaveBeenCalledWith("chatgpt", {
        x: 10,
        y: 20,
        width: 1200,
        height: 700,
      }),
    );
    expect(window.requestAnimationFrame).not.toHaveBeenCalled();
    view.unmount();
  });

  it("collapses a burst of layout notifications into one report", async () => {
    const view = render(<Harness />);
    await waitFor(() => expect(providerGateway.show).toHaveBeenCalledOnce());

    for (let frame = 0; frame < 20; frame += 1) {
      window.dispatchEvent(new Event("resize"));
    }

    await waitFor(() => expect(providerGateway.resize).toHaveBeenCalled());
    expect(providerGateway.resize).toHaveBeenCalledTimes(1);
    view.unmount();
  });
});
