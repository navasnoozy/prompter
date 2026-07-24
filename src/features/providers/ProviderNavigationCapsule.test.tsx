// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
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
import { providerGateway } from "./gateway";
import type { ProviderNavigationState } from "./model";
import { ProviderNavigationCapsule } from "./ProviderNavigationCapsule";
import { initializeProviderStore } from "./store";

vi.mock("./gateway", async (importOriginal) => {
  const original = await importOriginal<typeof import("./gateway")>();
  return {
    ...original,
    providerGateway: {
      ...original.providerGateway,
      controlNavigation: vi.fn(),
    },
  };
});

function navigation(
  overrides: Partial<ProviderNavigationState> = {},
): ProviderNavigationState {
  return {
    version: 1,
    provider: "chatgpt",
    generation: 8,
    revision: 3,
    available: true,
    canGoBack: true,
    canGoForward: true,
    isLoading: false,
    ...overrides,
  };
}

describe("ProviderNavigationCapsule", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    initializeProviderStore("chatgpt");
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
    vi.mocked(providerGateway.controlNavigation).mockResolvedValue(
      navigation({ revision: 4 }),
    );
  });

  afterEach(() => {
    cleanup();
  });

  it("always shows all three controls", () => {
    render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation()}
        provider="chatgpt"
      />,
    );

    expect(
      screen.getByRole("navigation", { name: "ChatGPT browser controls" }),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Go back" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Go forward" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Reload page" })).toBeTruthy();
  });

  it.each([
    ["Go back", "back"],
    ["Go forward", "forward"],
    ["Reload page", "reload"],
  ] as const)("dispatches %s exactly once", async (label, action) => {
    render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation()}
        provider="chatgpt"
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: label }));

    await waitFor(() =>
      expect(providerGateway.controlNavigation).toHaveBeenCalledWith(
        "chatgpt",
        8,
        action,
      ),
    );
    expect(providerGateway.controlNavigation).toHaveBeenCalledOnce();
  });

  it("replaces Reload with a neutral Stop action while loading", async () => {
    render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation({ isLoading: true })}
        provider="chatgpt"
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Reload page" }),
    ).toBeNull();
    fireEvent.click(
      screen.getByRole("button", { name: "Stop loading page" }),
    );

    await waitFor(() =>
      expect(providerGateway.controlNavigation).toHaveBeenCalledWith(
        "chatgpt",
        8,
        "stop",
      ),
    );
  });

  it("keeps disabled actions focusable but never dispatches them", () => {
    render(
      <ProviderNavigationCapsule
        isPlacing
        navigation={navigation({
          canGoBack: false,
          canGoForward: false,
        })}
        provider="chatgpt"
      />,
    );

    const back = screen.getByRole("button", { name: "Go back" });
    expect(back.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(back);
    fireEvent.focus(back);
    const forward = screen.getByRole("button", { name: "Go forward" });
    expect(forward.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(forward);

    expect(providerGateway.controlNavigation).not.toHaveBeenCalled();
  });

  it("renders no dead controls before the native pane is available", () => {
    const { container } = render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation({ available: false })}
        provider="chatgpt"
      />,
    );

    expect(container.childElementCount).toBe(0);
  });

  it("serializes rapid clicks until the native action is acknowledged", async () => {
    let acknowledge:
      | ((state: ProviderNavigationState) => void)
      | undefined;
    vi.mocked(providerGateway.controlNavigation).mockImplementation(
      () =>
        new Promise((resolve) => {
          acknowledge = resolve;
        }),
    );
    render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation()}
        provider="chatgpt"
      />,
    );

    const back = screen.getByRole("button", { name: "Go back" });
    fireEvent.click(back);
    fireEvent.click(back);

    expect(providerGateway.controlNavigation).toHaveBeenCalledOnce();
    expect(back.getAttribute("aria-disabled")).toBe("true");

    acknowledge?.(navigation({ revision: 4, canGoBack: false }));
    await waitFor(() =>
      expect(back.getAttribute("aria-disabled")).toBe("false"),
    );
  });

  it("publishes a safe error and unlocks after a rejected action", async () => {
    vi.mocked(providerGateway.controlNavigation).mockRejectedValueOnce({
      version: 1,
      code: "navigation_blocked",
      message: "Wait for prompt placement to finish before navigating.",
    });
    render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation()}
        provider="chatgpt"
      />,
    );

    const back = screen.getByRole("button", { name: "Go back" });
    fireEvent.click(back);

    await waitFor(() =>
      expect(useNoticeStore.getState().notice).toMatchObject({
        kind: "error",
        message: "Wait for prompt placement to finish before navigating.",
      }),
    );
    expect(back.getAttribute("aria-disabled")).toBe("false");

    fireEvent.click(back);
    await waitFor(() =>
      expect(providerGateway.controlNavigation).toHaveBeenCalledTimes(2),
    );
  });
});
