/**
 * 首页出口信息 Hook
 * 作者: TanXiang
 *
 * 拉取代理出口 IP / 归属地 / 国家代码（复用审计页连通检测后端命令）。
 * 策略：视图激活时探测一次 + 节点切换时自动重探（延迟防抖）+ 手动刷新。
 * 不做常驻轮询——ip-api.com 免费接口限频 45 req/min，且出口 IP 低频变化。
 */
import { ref, watch, type Ref } from "vue";
import { invokeWithTimeout } from "@/api/ipc/client";
import type { ApiResponse } from "@/types";

/** 出口信息（取 proxied 路径——即当前流量的真实出口） */
export interface EgressInfo {
  ip: string;
  location: string;
  countryCode: string;
  /** 是否取得成功（失败时展示 fallback 文案） */
  ok: boolean;
}

/** 连通检测报告（后端 proxy_connectivity_check 的 data 结构） */
interface ConnectivityReport {
  proxied: {
    ok: boolean;
    egress_ip: string;
    location: string;
    country_code: string;
  };
  kernel_running: boolean;
}

export function useEgressInfo(currentNode: Ref<string | undefined>) {
  const egress = ref<EgressInfo | null>(null);
  const loading = ref(false);

  /** 探测出口 IP（8s 后端超时 + 前端 15s 兜底） */
  async function refresh() {
    if (loading.value) return;
    loading.value = true;
    try {
      const res = await invokeWithTimeout<ApiResponse<ConnectivityReport>>(
        "proxy_connectivity_check",
        {},
        20000
      );
      if (res.success && res.data && res.data.proxied) {
        const p = res.data.proxied;
        egress.value = {
          ip: p.egress_ip,
          location: p.location,
          countryCode: p.country_code,
          ok: p.ok,
        };
      } else {
        egress.value = { ip: "", location: "", countryCode: "", ok: false };
      }
    } catch {
      egress.value = { ip: "", location: "", countryCode: "", ok: false };
    } finally {
      loading.value = false;
    }
  }

  // 节点切换 → 出口可能变化，2s 防抖后重探（切换瞬间链路未稳，立刻探会失败）
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  watch(currentNode, (val, old) => {
    if (!val || val === old) return;
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => refresh(), 2000);
  });

  return { egress, egressLoading: loading, refresh };
}
