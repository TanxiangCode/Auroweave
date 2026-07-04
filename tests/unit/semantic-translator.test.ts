import { describe, it, expect } from "vitest";
import { translateConnection } from "@/utils/semantic-translator";

describe("Semantic Translator Utility", () => {
  it("translates google domains to proxy speedup", () => {
    const result = translateConnection("www.google.com", "proxy", "rule-1");
    expect(result.semanticText).toContain("谷歌服务");
    expect(result.icon).toBe("🚀");
    expect(result.type).toBe("proxied");
  });

  it("translates cn domains to direct bypass", () => {
    const result = translateConnection("baidu.com", "direct", "geoip-cn");
    expect(result.semanticText).toContain("直连");
    expect(result.icon).toBe("🎯");
    expect(result.type).toBe("direct");
  });

  it("translates blocked domains", () => {
    const result = translateConnection("adservice.google.com", "block", "ad-block");
    expect(result.semanticText).toContain("拦截");
    expect(result.icon).toBe("🚫");
    expect(result.type).toBe("blocked");
  });
});
