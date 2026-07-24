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
    vi.unstubAllGlobals();
  });

  it("shows Back when collapsed and reveals the remaining controls on hover", () => {
    render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation()}
        provider="chatgpt"
      />,
    );

    const controls = screen.getByRole("navigation", {
      name: "ChatGPT browser controls",
    });
    expect(screen.getByRole("button", { name: "Go back" })).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Go forward" }),
    ).toBeNull();

    fireEvent.pointerEnter(controls, { pointerType: "mouse" });
    expect(
      screen.getByRole("button", { name: "Go forward" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Reload page" }),
    ).toBeTruthy();

    fireEvent.pointerLeave(controls, { pointerType: "mouse" });
    expect(
      screen.queryByRole("button", { name: "Go forward" }),
    ).toBeNull();
  });

  it("expands for keyboard focus and Escape collapses back to Back", () => {
    render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation()}
        provider="chatgpt"
      />,
    );

    const back = screen.getByRole("button", { name: "Go back" });
    fireEvent.focus(back);
    const forward = screen.getByRole("button", { name: "Go forward" });
    forward.focus();
    fireEvent.keyDown(forward, { key: "Escape" });

    expect(document.activeElement).toBe(back);
    expect(
      screen.queryByRole("button", { name: "Go forward" }),
    ).toBeNull();
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
    const controls = screen.getByRole("navigation");
    fireEvent.pointerEnter(controls, { pointerType: "mouse" });
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
    fireEvent.pointerEnter(screen.getByRole("navigation"), {
      pointerType: "mouse",
    });

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

  it("stays expanded while focus remains inside and collapses on outside focus", () => {
    render(
      <>
        <button type="button">Outside</button>
        <ProviderNavigationCapsule
          isPlacing={false}
          navigation={navigation()}
          provider="chatgpt"
        />
      </>,
    );
    const controls = screen.getByRole("navigation");
    fireEvent.pointerEnter(controls, { pointerType: "mouse" });
    const forward = screen.getByRole("button", { name: "Go forward" });
    forward.focus();
    fireEvent.pointerLeave(controls, { pointerType: "mouse" });

    expect(
      screen.getByRole("button", { name: "Reload page" }),
    ).toBeTruthy();

    const outside = screen.getByRole("button", { name: "Outside" });
    fireEvent.blur(forward, { relatedTarget: outside });
    outside.focus();
    expect(controls.getAttribute("data-expanded")).toBe("false");
    expect(forward.tabIndex).toBe(-1);
  });

  it("keeps all 44px controls exposed for any coarse pointer", () => {
    const addEventListener = vi.fn();
    const removeEventListener = vi.fn();
    const matchMedia = vi.fn().mockReturnValue({
      matches: true,
      media: "",
      onchange: null,
      addEventListener,
      removeEventListener,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    });
    vi.stubGlobal("matchMedia", matchMedia);

    render(
      <ProviderNavigationCapsule
        isPlacing={false}
        navigation={navigation()}
        provider="chatgpt"
      />,
    );

    expect(matchMedia).toHaveBeenCalledWith(
      "(hover: none), (pointer: coarse), (any-pointer: coarse)",
    );
    expect(
      screen.getByRole("button", { name: "Go forward" }).tabIndex,
    ).toBe(0);
    expect(
      screen.getByRole("button", { name: "Reload page" }).tabIndex,
    ).toBe(0);
  });
});
