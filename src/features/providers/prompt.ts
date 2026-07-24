import type { PromptComposition } from "./model";

export const MAX_PROMPT_BYTES = 1_048_576;

const encoder = new TextEncoder();

export function promptByteLength(composition: PromptComposition): number {
  const beforeText = composition.beforeText.trim();
  const text = composition.text.trim();
  const afterText = composition.afterText.trim();
  const separatorBytes = afterText ? 4 : 2;

  return (
    encoder.encode(beforeText).byteLength +
    encoder.encode(text).byteLength +
    encoder.encode(afterText).byteLength +
    separatorBytes
  );
}

export function isPromptTooLarge(composition: PromptComposition): boolean {
  return promptByteLength(composition) > MAX_PROMPT_BYTES;
}
