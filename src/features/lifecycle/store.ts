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

export const useLifecycleStore = create<LifecycleState>()((set, get) => ({
  status: null,
  isUpdatingLaunchAtLogin: false,

  refreshStatus: async () => {
    try {
      set({ status: await appLifecycleGateway.getStatus() });
    } catch {
      // The next window focus retries; the UI treats null as "checking".
    }
  },

  setLaunchAtLogin: async (enabled) => {
    set({ isUpdatingLaunchAtLogin: true });
    try {
      const status = await appLifecycleGateway.setLaunchAtLogin(enabled);
      set({ status });
      publishNotice(
        "success",
        enabled
          ? "Prompter will start quietly when you log in"
          : "Launch at Login is off",
      );
    } catch (error) {
      publishNotice("error", normalizeAppLifecycleError(error).message);
      await get().refreshStatus();
    } finally {
      set({ isUpdatingLaunchAtLogin: false });
    }
  },

  applyVisibility: (visible) => {
    set((state) => ({
      status: state.status
        ? { ...state.status, mainWindowVisible: visible }
        : state.status,
    }));
  },
}));
