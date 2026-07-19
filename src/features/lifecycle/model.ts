export const APP_LIFECYCLE_CONTRACT_VERSION = 1;

export type LaunchAtLoginState = "enabled" | "disabled" | "unavailable";

export type AppLifecycleStatus = {
  version: 1;
  launchAtLogin: LaunchAtLoginState;
  mainWindowVisible: boolean;
};

export type MainWindowVisibilityPayload = {
  version: 1;
  visible: boolean;
};

export type AppLifecycleError = {
  version: 1;
  code: "launch_at_login_unavailable";
  message: string;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isVersion(value: unknown): value is 1 {
  return value === APP_LIFECYCLE_CONTRACT_VERSION;
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function isLaunchAtLoginState(value: unknown): value is LaunchAtLoginState {
  return value === "enabled" || value === "disabled" || value === "unavailable";
}

export function parseAppLifecycleStatus(
  value: unknown,
): AppLifecycleStatus | null {
  if (
    !isRecord(value) ||
    !isVersion(value.version) ||
    !isLaunchAtLoginState(value.launchAtLogin) ||
    typeof value.mainWindowVisible !== "boolean"
  ) {
    return null;
  }

  return {
    version: value.version,
    launchAtLogin: value.launchAtLogin,
    mainWindowVisible: value.mainWindowVisible,
  };
}

export function parseMainWindowVisibility(
  value: unknown,
): MainWindowVisibilityPayload | null {
  if (
    !isRecord(value) ||
    !isVersion(value.version) ||
    typeof value.visible !== "boolean"
  ) {
    return null;
  }
  return { version: value.version, visible: value.visible };
}

export function parseAppLifecycleError(
  value: unknown,
): AppLifecycleError | null {
  if (
    !isRecord(value) ||
    !isVersion(value.version) ||
    value.code !== "launch_at_login_unavailable" ||
    !isNonEmptyString(value.message)
  ) {
    return null;
  }
  return {
    version: value.version,
    code: value.code,
    message: value.message,
  };
}
