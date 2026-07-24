// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { ProviderNavigationState } from "../providers/model";
import {
  initializeProviderStore,
  useProviderStore,
} from "../providers/store";
import { InstructionSidebar } from "./InstructionSidebar";

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

describe("InstructionSidebar", () => {
  beforeEach(() => {
    initializeProviderStore("chatgpt");
  });

  afterEach(cleanup);

  it("hosts the browser navigation controls once the native pane is available", () => {
    act(() => {
      useProviderStore.getState().updateNavigationState(navigation());
      useProviderStore.setState({ panelOpen: true });
    });

    render(<InstructionSidebar />);

    expect(
      screen.getByRole("navigation", { name: "ChatGPT browser controls" }),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Go back" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Go forward" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Reload page" })).toBeTruthy();
  });

  it("omits the controls until the panel is open", () => {
    act(() => {
      useProviderStore.getState().updateNavigationState(navigation());
      useProviderStore.setState({ panelOpen: false });
    });

    render(<InstructionSidebar />);

    expect(
      screen.queryByRole("navigation", { name: "ChatGPT browser controls" }),
    ).toBeNull();
  });

  it("omits the controls before the native pane reports availability", () => {
    act(() => {
      useProviderStore
        .getState()
        .updateNavigationState(navigation({ available: false }));
      useProviderStore.setState({ panelOpen: true });
    });

    render(<InstructionSidebar />);

    expect(
      screen.queryByRole("navigation", { name: "ChatGPT browser controls" }),
    ).toBeNull();
  });
});
