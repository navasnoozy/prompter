import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  APP_LIFECYCLE_COMMANDS,
  APP_LIFECYCLE_EVENTS,
  appLifecycleGateway,
  AppLifecycleProtocolError,
  normalizeAppLifecycleError,
} from "./gateway";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

const validStatus = {
  version: 1 as const,
  launchAtLogin: "disabled" as const,
  mainWindowVisible: true,
};

describe("application lifecycle gateway", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it("reads the authoritative native lifecycle status", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(validStatus);

    await expect(appLifecycleGateway.getStatus()).resolves.toEqual(validStatus);
    expect(invoke).toHaveBeenCalledWith(APP_LIFECYCLE_COMMANDS.getStatus);
  });

  it("sets Launch at Login through the versioned native command", async () => {
    const enabledStatus = { ...validStatus, launchAtLogin: "enabled" as const };
    vi.mocked(invoke).mockResolvedValueOnce(enabledStatus);

    await expect(
      appLifecycleGateway.setLaunchAtLogin(true),
    ).resolves.toEqual(enabledStatus);
    expect(invoke).toHaveBeenCalledWith(
      APP_LIFECYCLE_COMMANDS.setLaunchAtLogin,
      { enabled: true },
    );
  });

  it("rejects malformed native status", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ version: 99 });

    await expect(appLifecycleGateway.getStatus()).rejects.toBeInstanceOf(
      AppLifecycleProtocolError,
    );
  });

  it("subscribes to valid visibility events only", async () => {
    let nativeHandler: ((event: { payload: unknown }) => void) | undefined;
    const unlisten = vi.fn();
    vi.mocked(listen).mockImplementationOnce(async (_event, handler) => {
      nativeHandler = handler as (event: { payload: unknown }) => void;
      return unlisten;
    });
    const handler = vi.fn();

    await expect(
      appLifecycleGateway.onMainWindowVisibility(handler),
    ).resolves.toBe(unlisten);
    expect(listen).toHaveBeenCalledWith(
      APP_LIFECYCLE_EVENTS.mainWindowVisibility,
      expect.any(Function),
    );

    nativeHandler?.({ payload: { version: 2, visible: true } });
    nativeHandler?.({ payload: { version: 1, visible: true } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith(true);
  });

  it("does not expose unknown native error details", () => {
    expect(normalizeAppLifecycleError("private native error")).toEqual({
      version: 1,
      code: "launch_at_login_unavailable",
      message: "Prompter could not update Launch at Login. Please try again.",
    });
  });
});
