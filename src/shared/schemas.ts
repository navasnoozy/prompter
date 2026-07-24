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
  "navigation_blocked",
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

export const ProviderNavigationStateSchema = z
  .strictObject({
    version: z.literal(1),
    provider: ProviderSchema,
    generation: z.int().min(0).max(4_294_967_295),
    revision: z.int().min(0).max(4_294_967_295),
    available: z.boolean(),
    canGoBack: z.boolean(),
    canGoForward: z.boolean(),
    isLoading: z.boolean(),
  })
  .superRefine((navigation, context) => {
    const isNeverCreated =
      navigation.generation === 0 && navigation.revision === 0;
    const hasVersionedGeneration =
      navigation.generation > 0 && navigation.revision > 0;

    if (!isNeverCreated && !hasVersionedGeneration) {
      context.addIssue({
        code: "custom",
        message:
          "Navigation generation and revision must both be zero or both be positive.",
      });
    }
    if (navigation.available && !hasVersionedGeneration) {
      context.addIssue({
        code: "custom",
        message: "Available navigation requires a versioned generation.",
      });
    }
    if (
      !navigation.available &&
      (navigation.canGoBack ||
        navigation.canGoForward ||
        navigation.isLoading)
    ) {
      context.addIssue({
        code: "custom",
        message: "Unavailable navigation cannot expose active browser state.",
      });
    }
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
