import { create } from "zustand";
import { publishNotice } from "../../shared/notices";
import { appLifecycleGateway, normalizeAppLifecycleError } from "./gateway";
import type { AppLifecycleStatus } from "./model";

type LifecycleState = {
  status: AppLifecycleStatus | null;
  isUpdatingLaunchAtLogin: boolean;
  refreshStatus: () => Promise<void>;
  setLaunchAtLogin: (enabled: boolean) => Promise<void>;
  applyVisibility: (visible: boolean) => void;
};

let statusRequestGeneration = 0;
let launchUpdateGeneration = 0;
let visibilityGeneration = 0;

function invalidateStatusRequests(): void {
  statusRequestGeneration += 1;
}

export const useLifecycleStore = create<LifecycleState>()((set, get) => ({
  status: null,
  isUpdatingLaunchAtLogin: false,

  refreshStatus: async () => {
    const generation = ++statusRequestGeneration;
    try {
      const status = await appLifecycleGateway.getStatus();
      if (generation === statusRequestGeneration) set({ status });
    } catch {
      // The next window focus retries; the UI treats null as "checking".
    }
  },

  setLaunchAtLogin: async (enabled) => {
    const generation = ++launchUpdateGeneration;
    const visibilityAtStart = visibilityGeneration;
    invalidateStatusRequests();
    set({ isUpdatingLaunchAtLogin: true });
    try {
      const status = await appLifecycleGateway.setLaunchAtLogin(enabled);
      if (generation !== launchUpdateGeneration) return;

      invalidateStatusRequests();
      set((state) => ({
        status: {
          ...status,
          mainWindowVisible:
            visibilityGeneration !== visibilityAtStart && state.status
              ? state.status.mainWindowVisible
              : status.mainWindowVisible,
        },
      }));
      publishNotice(
        "success",
        enabled
          ? "Prompter will start quietly when you log in"
          : "Launch at Login is off",
      );
    } catch (error) {
      if (generation !== launchUpdateGeneration) return;
      publishNotice("error", normalizeAppLifecycleError(error).message);
      await get().refreshStatus();
    } finally {
      if (generation === launchUpdateGeneration) {
        set({ isUpdatingLaunchAtLogin: false });
      }
    }
  },

  applyVisibility: (visible) => {
    visibilityGeneration += 1;
    invalidateStatusRequests();
    set((state) => ({
      status: state.status
        ? { ...state.status, mainWindowVisible: visible }
        : state.status,
    }));
  },
}));
