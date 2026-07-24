// @vitest-environment jsdom
import {
  act,
  cleanup,
  render,
  waitFor,
} from "@testing-library/react";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { useNoticeStore } from "../../shared/notices";
import type { ProviderNavigationState } from "./model";
import { providerGateway } from "./gateway";
import { initializeProviderStore, useProviderStore } from "./store";
import { useProviderNavigation } from "./useProviderNavigation";

vi.mock("./gateway", async (importOriginal) => {
  const original = await importOriginal<typeof import("./gateway")>();
  return {
    ...original,
    providerGateway: {
      ...original.providerGateway,
      getNavigationState: vi.fn(),
      onNavigationState: vi.fn(),
    },
  };
});

function Harness() {
  useProviderNavigation();
  return null;
}

function navigation(
  overrides: Partial<ProviderNavigationState> = {},
): ProviderNavigationState {
  return {
    version: 1,
    provider: "chatgpt",
    generation: 1,
    revision: 1,
    available: true,
    canGoBack: false,
    canGoForward: false,
    isLoading: false,
    ...overrides,
  };
}

describe("useProviderNavigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    initializeProviderStore("chatgpt");
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("subscribes before requesting the race-closing snapshot", async () => {
    const stop = vi.fn();
    vi.mocked(providerGateway.onNavigationState).mockResolvedValue(stop);
    vi.mocked(providerGateway.getNavigationState).mockResolvedValue(
      navigation({ canGoBack: true }),
    );

    const view = render(<Harness />);

    await waitFor(() =>
      expect(
        useProviderStore.getState().navigationByProvider.chatgpt,
      ).toMatchObject({ available: true, canGoBack: true }),
    );
    expect(
      vi.mocked(providerGateway.onNavigationState).mock
        .invocationCallOrder[0],
    ).toBeLessThan(
      vi.mocked(providerGateway.getNavigationState).mock
        .invocationCallOrder[0],
    );

    view.unmount();
    expect(stop).toHaveBeenCalledOnce();
  });

  it("does not let a late snapshot overwrite a newer event", async () => {
    let eventHandler:
      | ((state: ProviderNavigationState) => void)
      | undefined;
    let resolveSnapshot:
      | ((state: ProviderNavigationState) => void)
      | undefined;
    vi.mocked(providerGateway.onNavigationState).mockImplementation(
      async (handler) => {
        eventHandler = handler;
        return () => {};
      },
    );
    vi.mocked(providerGateway.getNavigationState).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveSnapshot = resolve;
        }),
    );

    render(<Harness />);
    await waitFor(() =>
      expect(providerGateway.getNavigationState).toHaveBeenCalled(),
    );
    eventHandler?.(
      navigation({
        revision: 4,
        canGoBack: true,
        canGoForward: true,
      }),
    );
    resolveSnapshot?.(navigation({ revision: 2 }));

    await waitFor(() =>
      expect(
        useProviderStore.getState().navigationByProvider.chatgpt,
      ).toMatchObject({
        revision: 4,
        canGoBack: true,
        canGoForward: true,
      }),
    );
  });

  it("cleans up a listener whose registration finishes after unmount", async () => {
    let resolveBinding:
      | ((stop: () => void) => void)
      | undefined;
    const stop = vi.fn();
    vi.mocked(providerGateway.onNavigationState).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveBinding = resolve;
        }),
    );

    const view = render(<Harness />);
    await waitFor(() =>
      expect(providerGateway.onNavigationState).toHaveBeenCalledOnce(),
    );
    view.unmount();
    resolveBinding?.(stop);

    await waitFor(() => expect(stop).toHaveBeenCalledOnce());
    expect(providerGateway.getNavigationState).not.toHaveBeenCalled();
  });

  it("rebinds to the newly selected provider and ignores the old snapshot", async () => {
    const stopChatgpt = vi.fn();
    const stopGemini = vi.fn();
    let resolveChatgpt:
      | ((state: ProviderNavigationState) => void)
      | undefined;
    vi.mocked(providerGateway.onNavigationState)
      .mockResolvedValueOnce(stopChatgpt)
      .mockResolvedValueOnce(stopGemini);
    vi.mocked(providerGateway.getNavigationState)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveChatgpt = resolve;
          }),
      )
      .mockResolvedValueOnce(
        navigation({
          provider: "gemini",
          generation: 3,
          revision: 2,
          canGoForward: true,
        }),
      );

    render(<Harness />);
    await waitFor(() =>
      expect(providerGateway.getNavigationState).toHaveBeenCalledWith(
        "chatgpt",
      ),
    );

    act(() => {
      useProviderStore.setState({
        provider: "gemini",
        panelOpen: false,
      });
    });

    await waitFor(() => {
      expect(stopChatgpt).toHaveBeenCalledOnce();
      expect(providerGateway.getNavigationState).toHaveBeenCalledWith(
        "gemini",
      );
      expect(
        useProviderStore.getState().navigationByProvider.gemini,
      ).toMatchObject({ generation: 3, canGoForward: true });
    });

    resolveChatgpt?.(
      navigation({ generation: 9, revision: 9, canGoBack: true }),
    );
    await act(async () => {
      await Promise.resolve();
    });

    expect(
      useProviderStore.getState().navigationByProvider.chatgpt,
    ).toMatchObject({ available: false, generation: 0 });
  });

  it("retries a transient snapshot failure without dropping the listener", async () => {
    vi.useFakeTimers();
    const stop = vi.fn();
    vi.mocked(providerGateway.onNavigationState).mockResolvedValue(stop);
    vi.mocked(providerGateway.getNavigationState)
      .mockRejectedValueOnce(new Error("WebView is starting"))
      .mockResolvedValueOnce(navigation({ canGoBack: true }));

    render(<Harness />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(providerGateway.getNavigationState).toHaveBeenCalledOnce();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });

    expect(providerGateway.getNavigationState).toHaveBeenCalledTimes(2);
    expect(
      useProviderStore.getState().navigationByProvider.chatgpt,
    ).toMatchObject({ available: true, canGoBack: true });
    expect(useNoticeStore.getState().notice.kind).not.toBe("error");
  });
});
