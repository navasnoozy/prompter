import { afterEach, describe, expect, it, vi } from "vitest";
import { createId } from "./ids";

describe("createId", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("uses randomUUID when WebKit provides it", () => {
    const randomUUID = vi.fn(() => "123e4567-e89b-42d3-a456-426614174000");
    vi.stubGlobal("crypto", { randomUUID, getRandomValues: vi.fn() });

    expect(createId()).toBe("123e4567-e89b-42d3-a456-426614174000");
    expect(randomUUID).toHaveBeenCalledOnce();
  });

  it("creates an RFC 4122 version-4 identifier without randomUUID", () => {
    const getRandomValues = vi.fn((bytes: Uint8Array) => {
      bytes.fill(0xff);
      return bytes;
    });
    vi.stubGlobal("crypto", { getRandomValues });

    expect(createId()).toBe("ffffffff-ffff-4fff-bfff-ffffffffffff");
    expect(getRandomValues).toHaveBeenCalledOnce();
  });
});
