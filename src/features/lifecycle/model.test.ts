import { describe, expect, it } from "vitest";
import {
  parseAppLifecycleError,
  parseAppLifecycleStatus,
  parseMainWindowVisibility,
} from "./model";

describe("application lifecycle native contracts", () => {
  it("parses lifecycle status and rejects unknown versions", () => {
    const status = {
      version: 1,
      launchAtLogin: "disabled",
      mainWindowVisible: true,
    };

    expect(parseAppLifecycleStatus(status)).toEqual(status);
    expect(parseAppLifecycleStatus({ ...status, version: 2 })).toBeNull();
  });

  it("validates window visibility events", () => {
    expect(parseMainWindowVisibility({ version: 1, visible: false })).toEqual({
      version: 1,
      visible: false,
    });
    expect(parseMainWindowVisibility({ version: 1, visible: "false" })).toBeNull();
  });

  it("accepts only the public lifecycle error contract", () => {
    expect(
      parseAppLifecycleError({
        version: 1,
        code: "launch_at_login_unavailable",
        message: "Please try again.",
      }),
    ).toEqual({
      version: 1,
      code: "launch_at_login_unavailable",
      message: "Please try again.",
    });
    expect(
      parseAppLifecycleError({
        version: 1,
        code: "native_details",
        message: "Sensitive",
      }),
    ).toBeNull();
  });
});
