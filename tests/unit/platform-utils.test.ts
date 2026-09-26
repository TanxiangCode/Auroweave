import { describe, it, expect, afterEach } from "vitest";
import { isMacOS } from "@/utils/format";

/**
 * isMacOS 回归测试
 *
 * 曾经的实现是 `userAgent.includes("mac")`，而真实 macOS UA 里根本没有
 * "mac" 这个连续子串（是 Macintosh / darwin / Mac OS），
 * 部分精简 WebView UA 更是不含任何 mac 标识 —— 会把 macOS 误判成非 mac，
 * 导致 macOS 专属 UI（如 SUID/TUN 提示）静默消失。
 */
const originalUA = Object.getOwnPropertyDescriptor(globalThis, "navigator");

function withUA(ua: string, platform = "") {
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { userAgent: ua, platform },
  });
}

afterEach(() => {
  if (originalUA) {
    Object.defineProperty(globalThis, "navigator", originalUA);
  }
});

describe("isMacOS", () => {
  it("识别 macOS 桌面 Safari 的 Macintosh UA", () => {
    withUA(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15"
    );
    expect(isMacOS()).toBe(true);
  });

  it("识别含 darwin 的精简 WebView UA（无 mac 子串）", () => {
    withUA("Mozilla/5.0 (X11; Darwin arm64) AppleWebKit/537.36 HappyDOM/20.14.5");
    expect(isMacOS()).toBe(true);
  });

  it("识别旧式 Mac OS UA", () => {
    withUA("Mozilla/5.0 (Mac OS 10_15_7)");
    expect(isMacOS()).toBe(true);
  });

  it("UA 无 mac 标识时回退到 navigator.platform", () => {
    withUA("Mozilla/5.0 (X11; Darwin arm64) WebKit/537.36", "MacIntel");
    expect(isMacOS()).toBe(true);
  });

  it("Windows 与 Linux 不误判为 macOS", () => {
    withUA(
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36",
      "Win32"
    );
    expect(isMacOS()).toBe(false);

    withUA("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120.0 Safari/537.36", "Linux x86_64");
    expect(isMacOS()).toBe(false);
  });

  it("空 UA 且空 platform 时返回 false（不抛异常）", () => {
    withUA("", "");
    expect(isMacOS()).toBe(false);
  });
});
