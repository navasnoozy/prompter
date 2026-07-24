// @vitest-environment jsdom
import { createRef } from "react";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { ProviderNavigationState } from "./model";
import { ProviderBrowser } from "./ProviderBrowser";
import {
  initializeProviderStore,
  useProviderStore,
} from "./store";

function navigation(
  overrides: Partial<ProviderNavigationState> = {},
): ProviderNavigationState {
  return {
    version: 1,
    provider: "chatgpt",
    generation: 4,
    revision: 2,
    available: true,
    canGoBack: true,
    canGoForward: false,
    isLoading: false,
    ...overrides,
  };
}

describe("ProviderBrowser", () => {
  beforeEach(() => {
    initializeProviderStore("chatgpt");
  });

  afterEach(cleanup);

  it("never exposes controls for a hidden native pane", () => {
    act(() => {
      useProviderStore
        .getState()
        .updateNavigationState(navigation());
      useProviderStore.setState({ panelOpen: false });
    });

    render(<ProviderBrowser hostRef={createRef<HTMLDivElement>()} />);

    expect(
      screen.queryByRole("navigation", {
        name: "ChatGPT browser controls",
      }),
    ).toBeNull();
    expect(
      screen
        .getByText("Opening ChatGPT…")
        .closest("[role='status']")
        ?.getAttribute("aria-hidden"),
    ).toBe("false");
  });

  it("scopes busy semantics to browser content and keeps Stop accessible", () => {
    act(() => {
      useProviderStore
        .getState()
        .updateNavigationState(navigation({ isLoading: true }));
      useProviderStore.setState({ panelOpen: true });
    });

    const { container } = render(
      <ProviderBrowser hostRef={createRef<HTMLDivElement>()} />,
    );

    const region = screen.getByRole("region", {
      name: "ChatGPT browser",
    });
    const host = container.querySelector("#provider-browser-content");
    expect(region.hasAttribute("aria-busy")).toBe(false);
    expect(host?.getAttribute("aria-busy")).toBe("true");
    fireEvent.focus(screen.getByRole("button", { name: "Go back" }));
    expect(
      screen.getByRole("button", { name: "Stop loading page" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("status", {
        name: "",
      }).textContent,
    ).toContain("ChatGPT page is loading.");
  });
});
