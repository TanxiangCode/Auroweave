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
