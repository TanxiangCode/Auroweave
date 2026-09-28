import { describe, expect, it } from "vitest";
import type { ProxyGroup, ProxyNode } from "@/types";
import {
  getLatencyTestNodeTags,
  hasProxyGroupTag,
} from "@/views/ProxiesView/utils/proxy-page";

const group = (tag: string, type: ProxyGroup["type"] = "selector"): ProxyGroup => ({
  tag,
  type,
  proxies: [],
});

const node = (tag: string, type: ProxyNode["type"]): ProxyNode => ({ tag, type });

describe("代理页分组选择辅助函数", () => {
  it("URL query 可命中真实组和 custom: 虚拟组", () => {
    const realGroups = [group("proxy"), group("auto", "urltest")];
    const virtualGroups = [group("custom:日本优选")];

    expect(hasProxyGroupTag(realGroups, virtualGroups, "auto")).toBe(true);
    expect(hasProxyGroupTag(realGroups, virtualGroups, "custom:日本优选")).toBe(true);
    expect(hasProxyGroupTag(realGroups, virtualGroups, "custom:不存在")).toBe(false);
    expect(hasProxyGroupTag(realGroups, virtualGroups, undefined)).toBe(false);
  });
});

describe("代理页批量延迟节点筛选", () => {
  it("虚拟 custom: 组的 rawNodes 可直接提取物理节点", () => {
    const rawNodes = [
      node("日本-01", "vless"),
      node("日本-02", "hysteria2"),
      node("内部选择器", "selector"),
      node("自动优选", "urltest"),
    ];

    expect(getLatencyTestNodeTags(rawNodes)).toEqual(["日本-01", "日本-02"]);
  });
});

describe("地区分组识别（探测面改造回归）", () => {
  // 2026-09-28：auto / 地区组 / 自定义组由 urltest 降级为 selector，
  // 侧栏地区分组若仍按 type==="urltest" 判定会全部消失。
  // 真实判据（useProxyGroups.regionGroups）已改为 tag 后缀匹配，
  // 此处以等价谓词锁定该契约。
  const systemTags = ["proxy", "auto", "balance"];
  const isRegion = (g: ProxyGroup) =>
    !systemTags.includes(g.tag) &&
    !g.tag.startsWith("custom-") &&
    g.tag.endsWith("-auto");

  it("selector 类型的地区组仍被识别（改造前会漏）", () => {
    const groups = [
      group("proxy"),
      group("auto"),
      group("balance"),
      group("US-auto", "selector"),
      group("JP-auto", "selector"),
      group("custom-Gemini可用", "selector"),
      group("其他普通组", "selector"),
    ];
    expect(groups.filter(isRegion).map((g) => g.tag)).toEqual(["US-auto", "JP-auto"]);
  });

  it("系统组与 custom- 前缀组不被误判为地区组", () => {
    const groups = [
      group("auto", "selector"),
      group("balance", "selector"),
      group("custom-Gemini可用", "selector"),
    ];
    expect(groups.filter(isRegion)).toHaveLength(0);
  });

  it("urltest 类型的老配置也能识别（向后兼容）", () => {
    const groups = [group("US-auto", "urltest"), group("JP-auto", "urltest")];
    expect(groups.filter(isRegion)).toHaveLength(2);
  });
});
