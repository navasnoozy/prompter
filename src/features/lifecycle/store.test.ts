import { beforeEach, describe, expect, it, vi } from "vitest";
import { useNoticeStore } from "../../shared/notices";
import { appLifecycleGateway } from "./gateway";
import type { AppLifecycleStatus } from "./model";
import { useLifecycleStore } from "./store";

vi.mock("./gateway", async (importOriginal) => {
  const original = await importOriginal<typeof import("./gateway")>();
  return {
    ...original,
    appLifecycleGateway: {
      getStatus: vi.fn(),
      setLaunchAtLogin: vi.fn(),
      onMainWindowVisibility: vi.fn(),
    },
  };
});

const lifecycleStatus = (
  launchAtLogin: "enabled" | "disabled",
  mainWindowVisible: boolean,
): AppLifecycleStatus => ({
  version: 1,
  launchAtLogin,
  mainWindowVisible,
});

describe("application lifecycle store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useLifecycleStore.setState({
      status: lifecycleStatus("disabled", true),
      isUpdatingLaunchAtLogin: false,
    });
    useNoticeStore.setState({
      notice: { id: 0, kind: "info", message: "Ready" },
    });
  });

  it("ignores a refresh that predates a Launch at Login update", async () => {
    let resolveRefresh: (value: AppLifecycleStatus) => void = () => {};
    vi.mocked(appLifecycleGateway.getStatus).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveRefresh = resolve;
      }),
    );
    vi.mocked(appLifecycleGateway.setLaunchAtLogin).mockResolvedValueOnce(
      lifecycleStatus("enabled", true),
    );

    const refresh = useLifecycleStore.getState().refreshStatus();
    await useLifecycleStore.getState().setLaunchAtLogin(true);
    resolveRefresh(lifecycleStatus("disabled", true));
    await refresh;

    expect(useLifecycleStore.getState().status).toEqual(
      lifecycleStatus("enabled", true),
    );
  });

  it("ignores a refresh that predates a visibility event", async () => {
    let resolveRefresh: (value: AppLifecycleStatus) => void = () => {};
    vi.mocked(appLifecycleGateway.getStatus).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveRefresh = resolve;
      }),
    );

    const refresh = useLifecycleStore.getState().refreshStatus();
    useLifecycleStore.getState().applyVisibility(false);
    resolveRefresh(lifecycleStatus("disabled", true));
    await refresh;

    expect(useLifecycleStore.getState().status).toEqual(
      lifecycleStatus("disabled", false),
    );
  });

  it("preserves a newer visibility event when a mutation response arrives", async () => {
    let resolveUpdate: (value: AppLifecycleStatus) => void = () => {};
    vi.mocked(appLifecycleGateway.setLaunchAtLogin).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveUpdate = resolve;
      }),
    );

    const update = useLifecycleStore.getState().setLaunchAtLogin(true);
    useLifecycleStore.getState().applyVisibility(false);
    resolveUpdate(lifecycleStatus("enabled", true));
    await update;

    expect(useLifecycleStore.getState().status).toEqual(
      lifecycleStatus("enabled", false),
    );
  });
});
