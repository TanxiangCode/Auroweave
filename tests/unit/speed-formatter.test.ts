import { describe, it, expect } from "vitest";

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec.toFixed(0)} B/s`;
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
  if (bytesPerSec < 1024 * 1024 * 1024) return `${(bytesPerSec / (1024 * 1024)).toFixed(2)} MB/s`;
  return `${(bytesPerSec / (1024 * 1024 * 1024)).toFixed(2)} GB/s`;
}

describe("Speed Formatter Utility", () => {
  it("formats B/s accurately", () => {
    expect(formatSpeed(500)).toBe("500 B/s");
  });

  it("formats KB/s accurately", () => {
    expect(formatSpeed(1536)).toBe("1.5 KB/s");
  });

  it("formats MB/s accurately", () => {
    expect(formatSpeed(5 * 1024 * 1024)).toBe("5.00 MB/s");
  });
});
