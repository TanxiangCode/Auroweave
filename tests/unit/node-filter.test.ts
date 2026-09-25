import { describe, expect, it } from "vitest";
import { isLatencyTimeout } from "@/views/ProxiesView/hooks/useNodeFilter";

describe("节点超时筛选", () => {
  it("只把已测试的 0/-1 视为超时，未测试节点保留", () => {
    expect(isLatencyTimeout(0)).toBe(true);
    expect(isLatencyTimeout(-1)).toBe(true);
    expect(isLatencyTimeout(1)).toBe(false);
    expect(isLatencyTimeout(999)).toBe(false);
    expect(isLatencyTimeout(undefined)).toBe(false);
  });
});