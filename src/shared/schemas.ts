import { z } from "zod";

export const SettingsLoadResponseSchema = z.strictObject({
  version: z.literal(1),
  sessionId: z.int().min(1),
  entries: z.record(z.string(), z.unknown()),
});

export const ProviderSchema = z.enum(["chatgpt", "gemini"]);

export const ProviderErrorCodeSchema = z.enum([
  "window_missing",
  "webview_missing",
  "webview_operation_failed",
  "invalid_bounds",
  "invalid_request",
  "wrong_host",
  "editor_not_found",
  "editor_update_failed",
  "missing_instruction",
  "missing_text",
  "prompt_too_large",
  "internal",
]);

export const PromptFilledEventSchema = z.strictObject({
  version: z.literal(1),
  provider: ProviderSchema,
  requestId: z.string().trim().min(1),
});

export const ProviderCommandErrorSchema = z.strictObject({
  version: z.literal(1),
  code: ProviderErrorCodeSchema,
  message: z.string().trim().min(1),
});

export const ProviderErrorEventSchema = z.strictObject({
  version: z.literal(1),
  provider: ProviderSchema,
  requestId: z.string().trim().min(1),
  code: ProviderErrorCodeSchema,
  message: z.string().trim().min(1),
});

export const QuickCaptureStatusSchema = z.strictObject({
  version: z.literal(1),
  shortcut: z.strictObject({
    accelerator: z.string().trim().min(1),
    display: z.string().trim().min(1),
  }),
  registration: z.enum(["registered", "unavailable"]),
  permission: z.enum(["granted", "required"]),
});

export const CaptureWarningSchema = z.strictObject({
  code: z.enum(["clipboard_restore_failed", "window_unavailable"]),
  message: z.string().trim().min(1),
});

export const CaptureSuccessSchema = z.strictObject({
  kind: z.literal("success"),
  version: z.literal(1),
  requestId: z.string().trim().min(1),
  text: z.string().trim().min(1),
  warnings: z.array(CaptureWarningSchema),
  durationMs: z.number().min(0),
});

export const CaptureFailureSchema = z.strictObject({
  kind: z.literal("failure"),
  version: z.literal(1),
  requestId: z.string().trim().min(1),
  code: z.enum([
    "permission_required",
    "invalid_request",
    "clipboard_unavailable",
    "clipboard_changed",
    "clipboard_too_large",
    "shortcut_keys_held",
    "copy_failed",
    "copy_timed_out",
    "no_text",
    "selection_too_large",
    "internal",
  ]),
  message: z.string().trim().min(1),
  permission: z.enum(["granted", "required"]),
  durationMs: z.number().min(0),
});

export const CaptureOutcomeSchema = z.discriminatedUnion("kind", [
  CaptureSuccessSchema,
  CaptureFailureSchema,
]);

export const AppLifecycleStatusSchema = z.strictObject({
  version: z.literal(1),
  launchAtLogin: z.enum(["enabled", "disabled", "unavailable"]),
  mainWindowVisible: z.boolean(),
});
