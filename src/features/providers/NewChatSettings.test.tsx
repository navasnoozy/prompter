// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { settingsGateway } from "../../shared/settingsGateway";
import { NewChatSettings } from "./NewChatSettings";
import { initializeProviderStore, useProviderStore } from "./store";

vi.mock("../../shared/settingsGateway", async (importOriginal) => {
  const original =
    await importOriginal<typeof import("../../shared/settingsGateway")>();
  return {
    ...original,
    settingsGateway: {
      load: vi.fn(),
      write: vi.fn().mockResolvedValue(true),
      writeMany: vi.fn().mockResolvedValue(true),
    },
  };
});

const CHATGPT_NEW_CHAT =
  '<a class="__menu-item" data-sidebar-item="true" data-testid="create-new-chat-button" href="/">' +
  "<div>New chat</div><kbd>⇧⌘O</kbd></a>";

function addressField(): HTMLInputElement {
  return screen.getAllByLabelText("New chat address")[0] as HTMLInputElement;
}

function elementField(): HTMLTextAreaElement {
  return screen.getAllByLabelText(
    "New chat button",
  )[0] as HTMLTextAreaElement;
}

function openAdvanced(): void {
  fireEvent.click(
    screen.getByText("Advanced — set the address and the button yourself"),
  );
}

describe("NewChatSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(settingsGateway.write).mockResolvedValue(true);
    initializeProviderStore("chatgpt", "auto", {});
  });

  afterEach(() => {
    cleanup();
  });

  it("saves the chosen mode durably", () => {
    render(<NewChatSettings />);

    fireEvent.change(screen.getByLabelText("How Prompter starts it"), {
      target: { value: "url" },
    });

    expect(useProviderStore.getState().newChatMode).toBe("url");
    expect(settingsGateway.write).toHaveBeenCalledWith("newChatMode", "url");
  });

  it("hides the per-provider fields when the reset is switched off", () => {
    render(<NewChatSettings />);

    fireEvent.change(screen.getByLabelText("How Prompter starts it"), {
      target: { value: "off" },
    });

    expect(screen.queryByText(/Advanced/)).toBeNull();
  });

  it("refuses an address that would leave the provider", () => {
    render(<NewChatSettings />);
    openAdvanced();

    fireEvent.change(addressField(), {
      target: { value: "https://evil.invalid/new" },
    });
    fireEvent.click(screen.getAllByRole("button", { name: "Save address" })[0]);

    expect(screen.getByRole("alert").textContent).toContain("chatgpt.com");
    expect(useProviderStore.getState().newChatOverrides.chatgpt).toBeUndefined();
    expect(settingsGateway.write).not.toHaveBeenCalled();
  });

  it("saves a same-origin address", () => {
    render(<NewChatSettings />);
    openAdvanced();

    fireEvent.change(addressField(), {
      target: { value: "https://chatgpt.com/new" },
    });
    fireEvent.click(screen.getAllByRole("button", { name: "Save address" })[0]);

    expect(useProviderStore.getState().newChatOverrides.chatgpt).toEqual({
      url: "https://chatgpt.com/new",
    });
    expect(settingsGateway.write).toHaveBeenCalledWith("newChatOverrides", {
      chatgpt: { url: "https://chatgpt.com/new" },
    });
  });

  it("turns a pasted element into signals and reports what it found", () => {
    render(<NewChatSettings />);
    openAdvanced();

    fireEvent.change(elementField(), { target: { value: CHATGPT_NEW_CHAT } });
    fireEvent.click(screen.getAllByRole("button", { name: "Save button" })[0]);

    expect(useProviderStore.getState().newChatOverrides.chatgpt?.matcher)
      .toEqual({
        testId: "create-new-chat-button",
        label: "New chat",
        href: "/",
        attributes: [{ name: "data-sidebar-item", value: "true" }],
      });
    const status = screen.getByRole("status").textContent ?? "";
    expect(status).toContain("create-new-chat-button");
    expect(status).toContain("New chat");
  });

  it("explains a paste it cannot use and changes nothing", () => {
    render(<NewChatSettings />);
    openAdvanced();

    fireEvent.change(elementField(), { target: { value: "New chat" } });
    fireEvent.click(screen.getAllByRole("button", { name: "Save button" })[0]);

    expect(screen.getByRole("alert").textContent).toBeTruthy();
    expect(useProviderStore.getState().newChatOverrides.chatgpt).toBeUndefined();
  });

  it("keeps the address when only the button description is cleared", () => {
    initializeProviderStore("chatgpt", "auto", {
      chatgpt: {
        url: "https://chatgpt.com/new",
        matcher: { testId: "create-new-chat-button" },
      },
    });
    render(<NewChatSettings />);
    openAdvanced();

    fireEvent.click(
      screen.getAllByRole("button", { name: "Use the built-in button" })[0],
    );

    expect(useProviderStore.getState().newChatOverrides.chatgpt).toEqual({
      url: "https://chatgpt.com/new",
    });
  });

  it("drops the provider entirely once both overrides are cleared", () => {
    initializeProviderStore("chatgpt", "auto", {
      chatgpt: { url: "https://chatgpt.com/new" },
    });
    render(<NewChatSettings />);
    openAdvanced();

    fireEvent.click(
      screen.getAllByRole("button", { name: "Use the built-in address" })[0],
    );

    expect(useProviderStore.getState().newChatOverrides).toEqual({});
  });

  it("edits each provider independently", () => {
    render(<NewChatSettings />);
    openAdvanced();

    const geminiAddress = screen.getAllByLabelText(
      "New chat address",
    )[1] as HTMLInputElement;
    fireEvent.change(geminiAddress, {
      target: { value: "https://gemini.google.com/u/1/app/new" },
    });
    fireEvent.click(screen.getAllByRole("button", { name: "Save address" })[1]);

    expect(useProviderStore.getState().newChatOverrides).toEqual({
      gemini: { url: "https://gemini.google.com/u/1/app/new" },
    });
  });
});
