<template>
  <div class="panel-container">
    <h2><BaseIcon name="Globe" :size="20" class="panel-header-icon" /> DNS 安全与延迟测试配置</h2>
    <div class="setting-group">
      <!-- DNS 解析模式 (Fake-IP / 真实 IP) -->
      <div class="setting-item-column">
        <div class="item-label">
          <span class="mode-header-title">DNS 解析模式 (Resolution Mode)</span>
          <span class="sub-label">
            选择内核处理境外与代理域名的 DNS 解析方式。切换后下次重启内核生效。
          </span>
        </div>
        <div class="dns-mode-cards">
          <button
            type="button"
            class="dns-mode-card"
            :class="{ active: localDnsMode === 'fakeip' }"
            @click="setDnsMode('fakeip')"
          >
            <div class="card-title">
              <BaseIcon name="Zap" :size="15" class="mode-icon" />
              <span>Fake-IP 模式</span>
              <span class="recommend-badge">推荐 / 默认</span>
            </div>
            <div class="card-desc">
              内核即时返回 198.18.0.0/15 保留 IP，0ms 解析响应，彻底免疫本地污染与递归 DNS 限流，无 DNS 泄露。
            </div>
          </button>
          <button
            type="button"
            class="dns-mode-card"
            :class="{ active: localDnsMode === 'realip' }"
            @click="setDnsMode('realip')"
          >
            <div class="card-title">
              <BaseIcon name="Globe" :size="15" class="mode-icon" />
              <span>真实 IP 模式 (Real-IP)</span>
            </div>
            <div class="card-desc">
              由远端 DoH 先行解析真实 IP 后再通过代理发起连接，兼容要求端到端校验真实 IP 的特定网络环境。
            </div>
          </button>
        </div>
      </div>

      <!-- 远端加密 DNS -->
      <div class="setting-item">
        <div class="item-label">
          <span>远端加密 DNS (Remote DoH)</span>
          <span class="sub-label">代理侧加密解析，防 DNS 污染（如 8.8.8.8 / 1.1.1.1），保存后重启内核生效</span>
        </div>
        <input
          v-model="localRemoteDoh"
          type="text"
          class="text-input editable"
          placeholder="8.8.8.8"
          @blur="saveRemoteDoh"
          @keyup.enter="saveRemoteDoh"
        />
      </div>

      <!-- DNS 查询超时 -->
      <div class="setting-item">
        <div class="item-label">
          <span>DNS 查询超时 (Timeout)</span>
          <span class="sub-label">sing-box 1.14 新增：上游无响应快速失败，不再拖默认 10 秒（推荐 3~10 秒）</span>
        </div>
        <div class="input-with-unit">
          <input
            v-model.number="localDnsTimeout"
            type="number"
            min="1"
            max="60"
            class="text-input number-input editable"
            @blur="saveDnsTimeout"
            @keyup.enter="saveDnsTimeout"
          />
          <span class="unit-label">s</span>
        </div>
      </div>

      <!-- 乐观 DNS 缓存 -->
      <div class="setting-item">
        <div class="item-label">
          <span>乐观 DNS 缓存 (Optimistic Cache)</span>
          <span class="sub-label">sing-box 1.14 新增：过期缓存立即返回 + 后台刷新，重复查询零等待；配合 DNS 持久化冷启动秒解析</span>
        </div>
        <input
          type="checkbox"
          v-model="localOptimistic"
          class="switch"
          @change="toggleOptimistic"
        />
      </div>

      <!-- 智能分流 v2 -->
      <div class="setting-item">
        <div class="item-label">
          <span>智能分流 v2 (Smart Routing)</span>
          <span class="sub-label">按解析结果判定直连：先问本地 DNS，答案为国内 IP 则直接采用并直连——比域名名单（geosite）更实时，对新域名/CDN 误判免疫；境外域名会多一次本地查询（毫秒级）</span>
        </div>
        <input
          type="checkbox"
          v-model="localSmartV2"
          class="switch"
          @change="toggleSmartV2"
        />
      </div>

      <!-- 节点域名解析 DNS（bootstrap） -->
      <div class="setting-item">
        <div class="item-label">
          <span>节点域名解析 DNS (Bootstrap)</span>
          <span class="sub-label">专用于解析节点服务器域名：国内直连加密 DoH（如 223.5.5.5 / 223.6.6.6），不经代理无回环，也不受运营商 DNS 污染影响；填域名时自动用 Local 解析其地址；保存后重启内核生效</span>
        </div>
        <input
          v-model="localBootstrapDoh"
          type="text"
          class="text-input editable"
          placeholder="223.5.5.5"
          @blur="saveBootstrapDoh"
          @keyup.enter="saveBootstrapDoh"
        />
      </div>

      <!-- 节点域名解析备用 DNS（bootstrap-backup） -->
      <div class="setting-item">
        <div class="item-label">
          <span>节点域名解析备用 DNS (Backup)</span>
          <span class="sub-label">主解析器返回 NXDOMAIN/SERVFAIL 时自动切换至此——机场子域轮换的删除窗口会被单一递归 DNS 按负缓存放大成约 10 分钟死区，备用解析器须与主用异构运营商（如主 223.5.5.5 备 1.12.12.12）才有独立缓存对冲效果</span>
        </div>
        <input
          v-model="localBackupDoh"
          type="text"
          class="text-input editable"
          placeholder="1.12.12.12"
          @blur="saveBackupDoh"
          @keyup.enter="saveBackupDoh"
        />
      </div>

      <!-- 本地直连 DNS -->
      <div class="setting-item">
        <div class="item-label">
          <span>本地直连 DNS (Local)</span>
          <span class="sub-label">type: local —— 走 macOS 系统原生解析器，自动跟随系统 DNS 配置。必需：承担国内域名直连分流（geosite/evaluate）、Direct 模式解析与局域网 mDNS，不可删除</span>
        </div>
        <input type="text" value="system (type: local)" class="text-input readonly" readonly tabindex="-1" />
      </div>

      <div class="dns-panel-hint">
        延迟测试参数（目标 URL / 并发 / 超时 / 自动优选周期）已归并至「设置 → 测速与解锁」，
        避免与 DNS 配置混淆。
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, watch } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";

const settingsStore = useSettingsStore();
const toast = useToast();

// DNS 配置（sing-box 1.14.0）
const localDnsMode = ref<"fakeip" | "realip">(settingsStore.settings.dns_mode === "realip" ? "realip" : "fakeip");
const localRemoteDoh = ref(settingsStore.settings.dns_remote_doh || "8.8.8.8");
const localBootstrapDoh = ref(settingsStore.settings.dns_bootstrap_doh || "223.5.5.5");
const localBackupDoh = ref(settingsStore.settings.dns_bootstrap_backup_doh || "1.12.12.12");
const localDnsTimeout = ref(settingsStore.settings.dns_timeout_secs || 5);
const localOptimistic = ref(settingsStore.settings.dns_optimistic_cache !== false);
const localSmartV2 = ref(settingsStore.settings.dns_smart_routing_v2 === true);

// 精确监听相关字段（数组形式 getter），避免深度 watch 整个 settings
// 在无关字段变化时触发无意义的重置
watch(
  () => [
    settingsStore.settings.dns_mode,
    settingsStore.settings.dns_remote_doh,
    settingsStore.settings.dns_bootstrap_doh,
    settingsStore.settings.dns_bootstrap_backup_doh,
    settingsStore.settings.dns_timeout_secs,
    settingsStore.settings.dns_optimistic_cache,
    settingsStore.settings.dns_smart_routing_v2,
  ] as const,
  ([mode, doh, bootstrapDoh, backupDoh, dnsTimeout, optimistic, smartV2]) => {
    localDnsMode.value = mode === "realip" ? "realip" : "fakeip";
    localRemoteDoh.value = doh || "8.8.8.8";
    localBootstrapDoh.value = bootstrapDoh || "223.5.5.5";
    localBackupDoh.value = backupDoh || "1.12.12.12";
    localDnsTimeout.value = dnsTimeout || 5;
    localOptimistic.value = optimistic !== false;
    localSmartV2.value = smartV2 === true;
  }
);

async function setDnsMode(mode: "fakeip" | "realip") {
  if (localDnsMode.value === mode) return;
  localDnsMode.value = mode;
  const res = await settingsStore.updateSettings({ dns_mode: mode });
  if (res.success) {
    toast.success(
      mode === "fakeip" ? "已切换为 Fake-IP 模式" : "已切换为真实 IP 模式",
      "下次重启内核后生效"
    );
  }
}

async function saveRemoteDoh() {
  const val = localRemoteDoh.value.trim() || "8.8.8.8";
  localRemoteDoh.value = val;
  const res = await settingsStore.updateSettings({ dns_remote_doh: val });
  if (res.success) {
    toast.success("远端 DoH 已更新", "下次重启内核后生效");
  }
}

async function saveBootstrapDoh() {
  const val = localBootstrapDoh.value.trim() || "223.5.5.5";
  localBootstrapDoh.value = val;
  const res = await settingsStore.updateSettings({ dns_bootstrap_doh: val });
  if (res.success) {
    toast.success("节点域名解析 DNS 已更新", "下次重启内核后生效");
  }
}

async function saveBackupDoh() {
  const val = localBackupDoh.value.trim() || "1.12.12.12";
  localBackupDoh.value = val;
  if (val === localBootstrapDoh.value.trim()) {
    toast.error("备用解析器与主用相同", "主备同地址时负缓存对冲无效，请填异构运营商 DNS");
  }
  const res = await settingsStore.updateSettings({ dns_bootstrap_backup_doh: val });
  if (res.success) {
    toast.success("节点域名备用解析 DNS 已更新", "下次重启内核后生效");
  }
}

async function saveDnsTimeout() {
  const val = Math.max(1, Math.min(60, Math.floor(localDnsTimeout.value || 5)));
  localDnsTimeout.value = val;
  const res = await settingsStore.updateSettings({ dns_timeout_secs: val });
  if (res.success) {
    toast.success("DNS 查询超时已更新", `当前超时: ${val}s`);
  }
}

async function toggleOptimistic() {
  // v-model 已翻转 localOptimistic，这里只负责持久化
  const val = localOptimistic.value;
  const res = await settingsStore.updateSettings({ dns_optimistic_cache: val });
  if (res.success) {
    toast.success("乐观 DNS 缓存已" + (val ? "开启" : "关闭"), "下次重启内核后生效");
  }
}

async function toggleSmartV2() {
  // v-model 已翻转 localSmartV2，这里只负责持久化
  const val = localSmartV2.value;
  const res = await settingsStore.updateSettings({ dns_smart_routing_v2: val });
  if (res.success) {
    toast.success("智能分流 v2 已" + (val ? "开启" : "关闭"), "下次重启内核后生效");
  }
}
</script>

<style scoped>
.panel-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

h2 {
  font-size: 18px;
  font-weight: 700;
  color: var(--text-primary);
}

.setting-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.setting-item-column {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
}

.mode-header-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.dns-mode-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
  margin-top: 4px;
}

.dns-mode-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 12px 14px;
  background: var(--surface-inset, var(--layer-2));
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  cursor: pointer;
  text-align: left;
  transition: all 0.2s ease;
  outline: none;
}

.dns-mode-card:hover {
  background: var(--layer-3);
  border-color: var(--border-normal);
  transform: translateY(-1px);
}

.dns-mode-card.active {
  background: var(--accent-cyan-glow, rgba(6, 182, 212, 0.08));
  border-color: var(--accent-cyan);
  color: var(--text-primary);
  box-shadow: 0 0 12px var(--accent-cyan-glow, rgba(6, 182, 212, 0.15));
}

.card-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: var(--weight-semibold);
}

.mode-icon {
  color: var(--accent-cyan);
  flex-shrink: 0;
}

.recommend-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--accent-cyan);
  color: #0b111e;
  font-weight: 700;
  letter-spacing: 0.2px;
}

.card-desc {
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-tertiary);
}

.dns-mode-card.active .card-desc {
  color: var(--text-secondary);
}

/* setting-item / item-label / sub-label 统一走 panel.css 全局定义 */

.text-input {
  padding: 6px 12px;
  background: var(--layer-1);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: 12px;
  width: 240px;
  outline: none;
  font-family: var(--font-mono);
}

.text-input.editable {
  color: var(--text-primary);
  border-color: var(--border-normal);
  background: var(--layer-3);
  transition: all var(--duration-fast);
}

.text-input.editable:focus {
  border-color: var(--accent-cyan);
  box-shadow: 0 0 6px var(--accent-cyan-glow);
}

/* 只读输入框：明显弱化的不可编辑样式，让用户明确知道该值当前不可配置 */
.text-input.readonly {
  opacity: 0.65;
  cursor: not-allowed;
  border-style: dashed;
  filter: grayscale(0.3);
}

/* "只读"标注徽标 */
.readonly-tag {
  font-size: 10px;
  font-weight: var(--weight-semibold);
  color: var(--text-tertiary);
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  padding: 0 4px;
  margin-left: 4px;
  vertical-align: 1px;
}

.input-with-unit {
  display: flex;
  align-items: center;
  gap: 8px;
}

.number-input {
  width: 100px;
  text-align: right;
}

.unit-label {
  font-size: 12px;
  color: var(--text-tertiary);
  min-width: 20px;
}

.dns-panel-hint {
  font-size: 11px;
  line-height: 1.6;
  color: var(--text-tertiary);
  padding: 8px 12px;
  background: var(--layer-1);
  border-left: 2px solid var(--accent-cyan);
  border-radius: 0 8px 8px 0;
}
</style>
