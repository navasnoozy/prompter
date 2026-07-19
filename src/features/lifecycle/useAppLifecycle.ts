import { useCallback, useEffect, useRef, useState } from "react";
import {
  appLifecycleGateway,
  normalizeAppLifecycleError,
} from "./gateway";
import type { AppLifecycleStatus } from "./model";

type UseAppLifecycleOptions = {
  onNotice: (message: string) => void;
};

export function useAppLifecycle({ onNotice }: UseAppLifecycleOptions) {
  const [status, setStatus] = useState<AppLifecycleStatus | null>(null);
  const [isUpdatingLaunchAtLogin, setIsUpdatingLaunchAtLogin] =
    useState(false);
  const onNoticeRef = useRef(onNotice);
  onNoticeRef.current = onNotice;

  const refreshStatus = useCallback(async () => {
    try {
      const nextStatus = await appLifecycleGateway.getStatus();
      setStatus(nextStatus);
      return nextStatus;
    } catch {
      return null;
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void appLifecycleGateway
      .onMainWindowVisibility((visible) => {
        if (!disposed) {
          setStatus((current) =>
            current ? { ...current, mainWindowVisible: visible } : current,
          );
        }
      })
      .then((stopListening) => {
        if (disposed) {
          stopListening();
          return;
        }
        unlisten = stopListening;
        void refreshStatus();
      })
      .catch(() => {
        if (!disposed) void refreshStatus();
      });

    const handleFocus = () => void refreshStatus();
    window.addEventListener("focus", handleFocus);
    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("focus", handleFocus);
    };
  }, [refreshStatus]);

  const setLaunchAtLogin = useCallback(
    async (enabled: boolean) => {
      setIsUpdatingLaunchAtLogin(true);
      try {
        const nextStatus = await appLifecycleGateway.setLaunchAtLogin(enabled);
        setStatus(nextStatus);
        onNoticeRef.current(
          enabled
            ? "Prompter will start quietly when you log in"
            : "Launch at Login is off",
        );
      } catch (error) {
        onNoticeRef.current(normalizeAppLifecycleError(error).message);
        await refreshStatus();
      } finally {
        setIsUpdatingLaunchAtLogin(false);
      }
    },
    [refreshStatus],
  );

  return {
    status,
    refreshStatus,
    setLaunchAtLogin,
    isUpdatingLaunchAtLogin,
  };
}
