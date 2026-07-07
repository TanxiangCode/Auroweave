<template>
  <div class="panel-container">
    <h2>📡 DNS 安全与延迟测试</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>远端加密 DNS (Remote DoH)</span>
          <span class="sub-label">海外加密解析，防 DNS 污染 (默认: https://1.1.1.1/dns-query)</span>
        </div>
        <input type="text" value="https://1.1.1.1/dns-query" class="text-input" readonly />
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>本地直连 DNS (Local UDP)</span>
          <span class="sub-label">国内零延迟解析 (默认: udp://223.5.5.5)</span>
        </div>
        <input type="text" value="223.5.5.5" class="text-input" readonly />
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>出站 urltest 默认延迟测试地址</span>
          <span class="sub-label">Sing-box 规则测试健康监测 URL</span>
        </div>
        <input type="text" :value="DEFAULT_LATENCY_TEST_URL" class="text-input" readonly />
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>延迟测试触发间隔 (秒)</span>
          <span class="sub-label">自动心跳检测间隔 (默认: {{ DEFAULT_LATENCY_TEST_INTERVAL_SEC }} 秒 / 容差 {{ DEFAULT_LATENCY_TEST_TOLERANCE_MS }}ms)</span>
        </div>
        <input type="text" :value="`${DEFAULT_LATENCY_TEST_INTERVAL_SEC}s`" class="text-input" readonly />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  DEFAULT_LATENCY_TEST_URL,
  DEFAULT_LATENCY_TEST_INTERVAL_SEC,
  DEFAULT_LATENCY_TEST_TOLERANCE_MS,
} from "@/constants";
</script>

<style scoped>
.panel-container { display: flex; flex-direction: column; gap: 16px; }
h2 { font-size: 18px; font-weight: 700; }
.setting-group { display: flex; flex-direction: column; gap: 12px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-weight: 600; }
.sub-label { font-size: 11px; color: rgba(255,255,255,0.4); font-weight: normal; }
.text-input { padding: 6px 12px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: rgba(255,255,255,0.8); font-size: 12px; width: 220px; outline: none; }
</style>
