import { useCallback, useEffect, useRef, useState } from "react";
import {
  normalizeQuickCaptureError,
  quickCaptureGateway,
} from "./gateway";
import type { CaptureOutcome, QuickCaptureStatus } from "./model";

type UseQuickCaptureOptions = {
  onNotice: (message: string) => void;
  onPermissionRequired: () => void;
};

export function useQuickCapture({
  onNotice,
  onPermissionRequired,
}: UseQuickCaptureOptions) {
  const [sourceText, setSourceText] = useState("");
  const [status, setStatus] = useState<QuickCaptureStatus | null>(null);
  const [isRequestingPermission, setIsRequestingPermission] = useState(false);
  const [isRetryingRegistration, setIsRetryingRegistration] = useState(false);
  const callbacksRef = useRef({ onNotice, onPermissionRequired });
  const drainRequestedRef = useRef(false);
  const isDrainingRef = useRef(false);
  callbacksRef.current = { onNotice, onPermissionRequired };

  const applyOutcome = useCallback((outcome: CaptureOutcome) => {
    if (outcome.kind === "success") {
      setSourceText(outcome.text);
      callbacksRef.current.onNotice(
        outcome.warnings[0]?.message ?? "Selected text captured",
      );
      return;
    }

    setStatus((current) =>
      current ? { ...current, permission: outcome.permission } : current,
    );
    callbacksRef.current.onNotice(outcome.message);
    if (outcome.code === "permission_required") {
      callbacksRef.current.onPermissionRequired();
    }
  }, []);

  const drainPendingOutcomes = useCallback(async () => {
    drainRequestedRef.current = true;
    if (isDrainingRef.current) return;

    isDrainingRef.current = true;
    try {
      while (drainRequestedRef.current) {
        drainRequestedRef.current = false;
        const outcomes = await quickCaptureGateway.takePendingOutcomes();
        outcomes.forEach(applyOutcome);
      }
    } catch (error) {
      callbacksRef.current.onNotice(normalizeQuickCaptureError(error).message);
    } finally {
      isDrainingRef.current = false;
    }
  }, [applyOutcome]);

  const refreshStatus = useCallback(async () => {
    try {
      const nextStatus = await quickCaptureGateway.getStatus();
      setStatus(nextStatus);
      return nextStatus;
    } catch {
      return null;
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void quickCaptureGateway
      .onReady(() => {
        if (!disposed) void drainPendingOutcomes();
      })
      .then((stopListening) => {
        if (disposed) {
          stopListening();
          return;
        }
        unlisten = stopListening;
        void drainPendingOutcomes();
      })
      .catch(() => {
        if (!disposed) {
          callbacksRef.current.onNotice("Quick Capture is unavailable.");
        }
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [drainPendingOutcomes]);

  useEffect(() => {
    void refreshStatus();
    const handleFocus = () => void refreshStatus();
    window.addEventListener("focus", handleFocus);
    return () => window.removeEventListener("focus", handleFocus);
  }, [refreshStatus]);

  const captureClipboard = useCallback(async () => {
    try {
      const payload = await quickCaptureGateway.readClipboardText();
      setSourceText(payload.text);
      callbacksRef.current.onNotice("Clipboard text captured");
    } catch (error) {
      callbacksRef.current.onNotice(normalizeQuickCaptureError(error).message);
    }
  }, []);

  const requestPermission = useCallback(async () => {
    setIsRequestingPermission(true);
    try {
      const nextStatus = await quickCaptureGateway.requestPermission();
      setStatus(nextStatus);
      callbacksRef.current.onNotice(
        nextStatus.permission === "granted"
          ? "Quick Capture is ready"
          : "Permission is still required. Open System Settings to enable Prompter.",
      );
    } catch (error) {
      callbacksRef.current.onNotice(normalizeQuickCaptureError(error).message);
    } finally {
      setIsRequestingPermission(false);
    }
  }, []);

  const openSystemSettings = useCallback(async () => {
    try {
      await quickCaptureGateway.openSystemSettings();
    } catch (error) {
      callbacksRef.current.onNotice(normalizeQuickCaptureError(error).message);
    }
  }, []);

  const retryRegistration = useCallback(async () => {
    setIsRetryingRegistration(true);
    try {
      const nextStatus = await quickCaptureGateway.retryRegistration();
      setStatus(nextStatus);
      callbacksRef.current.onNotice(
        nextStatus.registration === "registered"
          ? "Keyboard shortcut is ready"
          : "The keyboard shortcut is still unavailable.",
      );
    } catch (error) {
      callbacksRef.current.onNotice(normalizeQuickCaptureError(error).message);
    } finally {
      setIsRetryingRegistration(false);
    }
  }, []);

  return {
    sourceText,
    setSourceText,
    captureClipboard,
    status,
    refreshStatus,
    requestPermission,
    openSystemSettings,
    retryRegistration,
    isRequestingPermission,
    isRetryingRegistration,
  };
}
