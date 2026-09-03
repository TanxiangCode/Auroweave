import { describe, it, expect } from "vitest";
import { translateConnection } from "@/utils/semantic-translator";

describe("Semantic Translator Utility", () => {
  it("translates google domains to proxy speedup", () => {
    const result = translateConnection({
      id: "conn-1",
      destination: "www.google.com",
      port: 443,
      outbound: "proxy",
      rule: "rule-1",
    });
    expect(result.type).toBe("proxied");
    expect(result.outbound).toBe("proxy");
    expect(result.domain).toBe("www.google.com");
    // 谷歌域名应命中生态分类规则
    expect(result.category).toBeTruthy();
    expect(result.semanticTitle.length).toBeGreaterThan(0);
  });

  it("translates cn domains to direct bypass", () => {
    const result = translateConnection({
      id: "conn-2",
      destination: "baidu.com",
      port: 443,
      outbound: "direct",
      rule: "geoip-cn",
    });
    expect(result.type).toBe("direct");
    expect(result.outbound).toBe("direct");
    expect(result.ruleMatched).toBe("geoip-cn");
    expect(result.riskLevel).toBe("safe");
  });

  it("translates blocked domains", () => {
    const result = translateConnection({
      id: "conn-3",
      destination: "adservice.google.com",
      port: 443,
      outbound: "block",
      rule: "ad-block",
    });
    expect(result.type).toBe("blocked");
    expect(result.outbound).toBe("block");
  });
});
