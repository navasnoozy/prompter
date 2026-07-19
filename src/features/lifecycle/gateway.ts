import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  parseAppLifecycleError,
  parseAppLifecycleStatus,
  parseMainWindowVisibility,
  type AppLifecycleError,
  type AppLifecycleStatus,
} from "./model";

export const APP_LIFECYCLE_COMMANDS = {
  getStatus: "get_app_lifecycle_status",
  setLaunchAtLogin: "set_launch_at_login",
} as const;

export const APP_LIFECYCLE_EVENTS = {
  mainWindowVisibility: "prompter://main-window-visibility",
} as const;

export class AppLifecycleProtocolError extends Error {
  constructor(contract: string) {
    super(`Invalid application lifecycle ${contract} response`);
    this.name = "AppLifecycleProtocolError";
  }
}

function parseStatus(value: unknown): AppLifecycleStatus {
  const status = parseAppLifecycleStatus(value);
  if (!status) throw new AppLifecycleProtocolError("status");
  return status;
}

export function normalizeAppLifecycleError(
  error: unknown,
): AppLifecycleError {
  return (
    parseAppLifecycleError(error) ?? {
      version: 1,
      code: "launch_at_login_unavailable",
      message: "Prompter could not update Launch at Login. Please try again.",
    }
  );
}

export const appLifecycleGateway = {
  async getStatus(): Promise<AppLifecycleStatus> {
    return parseStatus(await invoke<unknown>(APP_LIFECYCLE_COMMANDS.getStatus));
  },

  async setLaunchAtLogin(enabled: boolean): Promise<AppLifecycleStatus> {
    return parseStatus(
      await invoke<unknown>(APP_LIFECYCLE_COMMANDS.setLaunchAtLogin, {
        enabled,
      }),
    );
  },

  onMainWindowVisibility(
    handler: (visible: boolean) => void,
  ): Promise<UnlistenFn> {
    return listen<unknown>(
      APP_LIFECYCLE_EVENTS.mainWindowVisibility,
      (event) => {
        const payload = parseMainWindowVisibility(event.payload);
        if (payload) handler(payload.visible);
      },
    );
  },
};
