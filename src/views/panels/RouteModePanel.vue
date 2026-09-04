<template>
  <div class="panel-container">
    <h2><BaseIcon name="GitFork" :size="20" class="panel-header-icon" /> 代理模式与端口设置</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>代理分流模式</span>
          <span class="sub-label">切换当前全局路由分流规则</span>
        </div>
        <select v-model="settingsStore.settings.proxy_mode" class="select-input" @change="saveMode">
          <option value="rule">规则模式 (Rule)</option>
          <option value="global">全局代理 (Global)</option>
          <option value="direct">直连模式 (Direct)</option>
        </select>
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>本地混合代理端口 (Mixed Inbound Port)</span>
          <span class="sub-label">HTTP / SOCKS5 混合协议监听端口 (默认: 8890)</span>
        </div>
        <input
          type="number"
          v-model.number="settingsStore.settings.mixed_port"
          class="num-input"
          placeholder="8890"
          @change="save"
        />
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>ClashAPI 控制端口 (ClashAPI External Controller Port)</span>
          <span class="sub-label">Sing-box ClashAPI 控制通信端口 (默认: 9090)</span>
        </div>
        <input
          type="number"
          v-model.number="settingsStore.settings.clash_api_port"
          class="num-input"
          placeholder="9090"
          @change="save"
        />
      </div>
    </div>

    <h2><BaseIcon name="Database" :size="20" class="panel-header-icon" /> 分流规则集 (Rule-Set)</h2>
    <p class="panel-desc">geosite-cn / geoip-cn 决定国内域名与 IP 的直连判定，订阅激活时自动下载缓存。可手动强制更新到最新版本：</p>
    <div class="setting-group">
      <div class="setting-item ruleset-item">
        <div class="item-label">
          <span>geosite-cn.srs</span>
          <span class="sub-label">{{ geositeDesc }}</span>
        </div>
      </div>
      <div class="setting-item ruleset-item">
        <div class="item-label">
          <span>geoip-cn.srs</span>
          <span class="sub-label">{{ geoipDesc }}</span>
        </div>
      </div>
      <button
        class="btn-update-ruleset"
        :disabled="updatingRuleset"
        @click="handleUpdateRuleSets"
      >
        {{ updatingRuleset ? "正在更新…" : "强制更新规则集" }}
      </button>
    </div>
  </div>
  </template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { onMounted, ref, computed } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useToast } from "@/composables/useToast";
import {
  getRuleSetStatus,
  forceUpdateRuleSets,
  type RuleSetStatus,
} from "@/api/ipc/subscription";

const settingsStore = useSettingsStore();
const proxyStore = useProxyStore();
const toast = useToast();

const rulesetStatus = ref<RuleSetStatus | null>(null);
const updatingRuleset = ref(false);

function formatSize(bytes: number): string {
  if (bytes <= 0) return "0 KB";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatTime(ms: number | null): string {
  if (!ms) return "未知";
  const d = new Date(ms);
  return `${d.getMonth() + 1}/${d.getDate()} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

const geositeDesc = computed(() => {
  const s = rulesetStatus.value;
  if (!s?.geosite_exists) return "未缓存（订阅激活时自动下载）";
  return `${formatSize(s.geosite_size)} · 更新于 ${formatTime(s.geosite_modified)}`;
});

const geoipDesc = computed(() => {
  const s = rulesetStatus.value;
  if (!s?.geoip_exists) return "未缓存（订阅激活时自动下载）";
  return `${formatSize(s.geoip_size)} · 更新于 ${formatTime(s.geoip_modified)}`;
});

async function loadRuleSetStatus() {
  const res = await getRuleSetStatus();
  if (res.success && res.data) {
    rulesetStatus.value = res.data;
  }
}

async function handleUpdateRuleSets() {
  updatingRuleset.value = true;
  try {
    const res = await forceUpdateRuleSets();
    if (res.success && res.data) {
      const [geositeOk, geoipOk, geositeSize, geoipSize] = res.data;
      if (geositeOk && geoipOk) {
        toast.success(
          "规则集已更新",
          `geosite ${formatSize(geositeSize)} · geoip ${formatSize(geoipSize)}，配置已重建`
        );
      } else if (geositeOk || geoipOk) {
        toast.error(
          "部分更新成功",
          `geosite ${geositeOk ? "✓" : "✗"} · geoip ${geoipOk ? "✓" : "✗"}，失败项保留旧缓存`
        );
      } else {
        toast.error("更新失败", "网络不可达，旧缓存仍有效");
      }
      await loadRuleSetStatus();
    } else {
      toast.error("更新失败", res.error || "下载异常");
    }
  } finally {
    updatingRuleset.value = false;
  }
}

onMounted(loadRuleSetStatus);

async function save() {
  // 局部 patch：仅提交本面板涉及的端口字段，避免全量 settings 覆盖其他未保存修改
  await settingsStore.updateSettings({
    mixed_port: settingsStore.settings.mixed_port,
    clash_api_port: settingsStore.settings.clash_api_port,
  });
  toast.success("端口与代理模式设置已保存");
}

async function saveMode() {
  // proxyStore.changeProxyMode 内部已通过 proxy_set_mode 保存设置并重建配置
  // 这里不再调用 save()，避免重复保存导致不必要的内核重启
  await proxyStore.changeProxyMode(settingsStore.settings.proxy_mode);
  toast.success("代理模式已切换");
}
</script>

<style scoped>
.panel-container { display: flex; flex-direction: column; gap: var(--space-4); }
h2 { font-size: var(--text-lg); font-weight: var(--weight-bold); }
.panel-desc { font-size: var(--text-xs); color: var(--text-tertiary); margin: 0; line-height: 1.6; }
.setting-group { display: flex; flex-direction: column; gap: var(--space-3); }
/* setting-item / item-label / sub-label 统一走 panel.css 全局定义 */

.select-input { padding: 6px 12px; background: var(--layer-1); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); color: var(--text-primary); outline: none; }
.num-input { padding: 6px 12px; background: var(--layer-1); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); color: var(--text-primary); width: 100px; outline: none; }
/* switch 统一走 App.vue 全局胶囊开关定义 */

/* 规则集区块 */
.ruleset-item { font-family: var(--font-mono); font-size: var(--text-sm); }
.btn-update-ruleset {
  align-self: flex-start;
  padding: 8px 18px;
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  color: var(--accent-cyan-vivid);
  background: var(--accent-cyan-glow, rgba(0, 242, 254, 0.1));
  border: 1px solid rgba(0, 242, 254, 0.3);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}
.btn-update-ruleset:hover:not(:disabled) { background: rgba(0, 242, 254, 0.2); color: #fff; }
.btn-update-ruleset:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
