<script setup lang="ts">
/**
 * 测速与解锁设置面板（从「高级与内核」抽离 + 归并 DNS 面板重复项）
 *
 * 职责：延迟测试参数（URL/并发/超时）、内核自动优选周期、吞吐量测速源、
 *       订阅网络超时、AI 服务解锁检测判据
 * 注意：本面板字段与 DNS 面板无交集——DNS 面板此前重复的
 *       「出站 urltest 延迟测试地址 / Concurrency / Timeout / Interval」已归并至此
 */
import { computed, ref, watch } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";
import {
  DEFAULT_LATENCY_TEST_INTERVAL_SEC,
  DEFAULT_LATENCY_TEST_TOLERANCE_MS,
  DEFAULT_SPEED_TEST_URLS,
  SPEED_TEST_PRESETS,
} from "@/constants";

const settingsStore = useSettingsStore();
const toast = useToast();

const highlightField = ref<string>("");
const CUSTOM_SPEED_SOURCE = "__custom__";

/** 深链高亮目标（由 SettingsView 从 route.query 透传） */
const props = defineProps<{ highlightTarget?: string }>();

/** 预设与自定义输入双向同步：手工改成未知 URL 时自动切到“自定义地址”。 */
const selectedSpeedSource = computed({
  get: () =>
    SPEED_TEST_PRESETS.some((item) => item.url === settingsStore.settings.speed_test_url)
      ? settingsStore.settings.speed_test_url
      : CUSTOM_SPEED_SOURCE,
  set: (value: string) => {
    if (value !== CUSTOM_SPEED_SOURCE) settingsStore.settings.speed_test_url = value;
  },
});

// 深链高亮：接收父组件传入的 highlightTarget。
//
// 此前这里读的是 `new URLSearchParams(window.location.search)`，但本项目用
// vue-router **hash 模式**，query 写在 `#` 之后（#/settings?highlight=xxx），
// window.location.search 恒为空串 → 高亮永远不触发。改为与 AdvancedPanel
// 一致，统一由 SettingsView 从 route.query 透传。
watch(
  () => props.highlightTarget,
  (target) => {
    if (target) {
      highlightField.value = target;
      setTimeout(() => (highlightField.value = ""), 3000);
    }
  },
  { immediate: true }
);

// ------------------------------------------------------------
// 多源容错测速列表（settings.speed_test_urls）
// 后端按序尝试，失败自动回退；该字段此前无任何 UI 入口
// ------------------------------------------------------------

/** 当前数据源列表（store 缺省时回落到预设全量，避免空白面板） */
const speedSourceUrls = computed<string[]>(() => {
  const list = settingsStore.settings.speed_test_urls;
  return Array.isArray(list) && list.length > 0 ? list : [...DEFAULT_SPEED_TEST_URLS];
});

/** 待添加：预设下拉或自定义输入框 */
const pendingSource = ref("");
const customSource = ref("");

/** 仅接受 http/https 绝对地址；预设已选项与重复项都判为不可添加 */
const canAddSource = computed(() => {
  const candidate = (pendingSource.value || customSource.value).trim();
  if (!/^https?:\/\/\S+$/i.test(candidate)) return false;
  return !speedSourceUrls.value.includes(candidate);
});

/** 追加数据源（去首尾空白，忽略重复） */
function addSource() {
  if (!canAddSource.value) return;
  const candidate = (pendingSource.value || customSource.value).trim();
  settingsStore.settings.speed_test_urls = [...speedSourceUrls.value, candidate];
  pendingSource.value = "";
  customSource.value = "";
  save();
}

/** 移除指定下标的数据源；至少保留一个，避免测速无源可用 */
function removeSource(index: number) {
  if (speedSourceUrls.value.length <= 1) return;
  settingsStore.settings.speed_test_urls = speedSourceUrls.value.filter(
    (_, i) => i !== index
  );
  save();
}

/** 恢复为内置预设全量 */
function resetSources() {
  settingsStore.settings.speed_test_urls = [...DEFAULT_SPEED_TEST_URLS];
  pendingSource.value = "";
  customSource.value = "";
  save();
}

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("测速设置已保存");
}
</script>

<template>
  <div class="panel-container">
    <h2><BaseIcon name="Timer" :size="20" class="panel-header-icon" /> 测速与解锁</h2>

    <!-- 延迟测试与吞吐量测速 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"><BaseIcon name="FlaskConical" :size="20" /></span>
        <div class="card-title-group">
          <h3>延迟测试与吞吐量测速</h3>
          <p>配置全节点批量并发测速、内核健康检测参数与下载吞吐量测试源</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>延迟测试目标 URL</span>
            <span class="sub-label">节点延迟连通性测试与出站 urltest 健康检测的探测端点 (返回 HTTP 204)</span>
          </div>
          <input
            v-model="settingsStore.settings.latency_test_url"
            type="text"
            class="text-input"
            placeholder="http://www.gstatic.com/generate_204"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>延迟测试并发数 (Concurrency)</span>
            <span class="sub-label">一键全量测速时允许的最大并发请求通道数 (推荐: 20~50)</span>
          </div>
          <input
            v-model.number="settingsStore.settings.latency_test_concurrency"
            type="number"
            class="num-input"
            min="1"
            max="100"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>延迟测试超时 (Timeout)</span>
            <span class="sub-label">单节点完整连接与请求超时阈值 (默认: 5000 ms；高延迟订阅可适当增大)</span>
          </div>
          <input
            v-model.number="settingsStore.settings.latency_test_timeout_ms"
            type="number"
            class="num-input"
            step="500"
            min="1000"
            max="10000"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>统一延迟统计</span>
            <span class="sub-label">通过独立 test-core 预热持久连接，统计第二次请求 RTT；更接近 Mihomo unified-delay，不影响内核后台 URLTest</span>
          </div>
          <input
            v-model="settingsStore.settings.latency_unified_delay"
            type="checkbox"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>统一模式复用预热连接</span>
            <span class="sub-label">默认开启；关闭后第二次探测使用冷连接，仅用于对照连接复用带来的差异</span>
          </div>
          <input
            v-model="settingsStore.settings.latency_persistent_reuse"
            type="checkbox"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>自动优选探测周期 (Interval) <span class="readonly-tag">只读</span></span>
            <span class="sub-label">内核 URLTest 组后台周期性探测间隔 (默认: {{ DEFAULT_LATENCY_TEST_INTERVAL_SEC }} 秒 / 容差 {{ DEFAULT_LATENCY_TEST_TOLERANCE_MS }}ms)</span>
          </div>
          <input type="text" :value="`${DEFAULT_LATENCY_TEST_INTERVAL_SEC}s`" class="text-input readonly" readonly tabindex="-1" />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>吞吐量下载测速 URL</span>
            <span class="sub-label">节点速度测试时的流式下载数据源</span>
          </div>
          <div class="speed-source-controls">
            <select v-model="selectedSpeedSource" class="select-input" @change="save">
              <option v-for="item in SPEED_TEST_PRESETS" :key="item.url" :value="item.url">
                {{ item.label }}
              </option>
              <option :value="CUSTOM_SPEED_SOURCE">自定义地址</option>
            </select>
            <input
              v-model="settingsStore.settings.speed_test_url"
              type="text"
              class="text-input speed-url-input"
              placeholder="https://example.com/large-test-file.bin"
              @change="save"
            />
          </div>
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>单节点下载测速限时 (秒)</span>
            <span class="sub-label">吞吐量下载速度测试的最大持续时间 (默认: 5 秒)</span>
          </div>
          <input
            v-model.number="settingsStore.settings.speed_test_timeout_secs"
            type="number"
            class="num-input"
            min="3"
            max="30"
            @change="save"
          />
        </div>

        <!-- 多源容错：吞吐量测速按顺序回退，避免单一大文件源被墙/限速导致测速全盘失败 -->
        <div class="setting-item" :class="{ highlight: highlightField === 'speed_sources' }">
          <div class="item-label">
            <span>多源容错测速列表</span>
            <span class="sub-label">
              吞吐量测速的数据源候选队列，按顺序尝试，前一个失败（超时/限速/被墙）自动回退到下一个。
              当前配置 {{ settingsStore.settings.speed_test_urls?.length || 0 }} 个，建议保留 2 个以上。
            </span>
          </div>
          <div class="url-list-editor">
            <div class="url-chips">
              <span v-for="(url, idx) in speedSourceUrls" :key="`${url}-${idx}`" class="url-chip">
                <span class="chip-index">{{ idx + 1 }}</span>
                <span class="chip-url" :title="url">{{ url }}</span>
                <button
                  class="chip-remove"
                  type="button"
                  :disabled="speedSourceUrls.length <= 1"
                  :title="speedSourceUrls.length <= 1 ? '至少保留一个数据源' : `移除 ${url}`"
                  @click="removeSource(idx)"
                >
                  <BaseIcon name="X" :size="11" />
                </button>
              </span>
            </div>
            <div class="url-add-row">
              <select v-model="pendingSource" class="select-input">
                <option value="">从预设中添加…</option>
                <option
                  v-for="item in SPEED_TEST_PRESETS"
                  :key="item.url"
                  :value="item.url"
                  :disabled="speedSourceUrls.includes(item.url)"
                >
                  {{ item.label }}{{ speedSourceUrls.includes(item.url) ? "（已添加）" : "" }}
                </option>
              </select>
              <input
                v-model="customSource"
                type="text"
                class="text-input speed-url-input"
                placeholder="或直接粘贴自定义 URL"
              />
              <button class="btn-add-source" :disabled="!canAddSource" @click="addSource">添加</button>
              <button class="btn-add-source" :disabled="speedSourceUrls.length === 0" @click="resetSources">
                恢复默认
              </button>
            </div>
          </div>
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>上下行并行测速</span>
            <span class="sub-label">
              吞吐测试同时跑上传与下载，耗时约减半；但共享带宽会互相挤压导致单侧读数偏低。
              默认关闭（串行测量精度更高），追求整体速度估算时可开启。
            </span>
          </div>
          <input
            v-model="settingsStore.settings.speedtest_parallel_updown"
            type="checkbox"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>test-core 探测并发数</span>
            <span class="sub-label">
              启动独立 test-core 实例做延迟/解锁探测时的并发上限 (可设 2~16，默认 8)。
              调高可加快批量探测，但会占用更多端口与内存。
            </span>
          </div>
          <input
            v-model.number="settingsStore.settings.unlock_test_concurrency"
            type="number"
            class="num-input"
            min="2"
            max="16"
            @change="save"
          />
        </div>

      </div>
    </div>

    <!-- AI 服务解锁检测判据 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"><BaseIcon name="Sparkles" :size="20" /></span>
        <div class="card-title-group">
          <h3>AI 服务解锁检测判据</h3>
          <p>Gemini / Claude / ChatGPT 可用性判定的页面特征——服务方轮换特征后在此更新，无需发版</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>Gemini 可用性标记</span>
            <span class="sub-label">gemini.google.com 页面正文包含此片段即判定可用（Google 混淆 ID，随版本轮换）</span>
          </div>
          <input
            v-model="settingsStore.settings.unlock_gemini_marker"
            type="text"
            class="text-input"
            placeholder="45631641,null,true"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>Claude 地区封锁特征</span>
            <span class="sub-label">claude.ai 重定向落点包含此片段即判定地区封锁</span>
          </div>
          <input
            v-model="settingsStore.settings.unlock_claude_block_marker"
            type="text"
            class="text-input"
            placeholder="app-unavailable-in-region"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>ChatGPT 地区封锁特征</span>
            <span class="sub-label">api.openai.com 响应正文包含此片段即判定地区封锁</span>
          </div>
          <input
            v-model="settingsStore.settings.unlock_chatgpt_block_marker"
            type="text"
            class="text-input"
            placeholder="unsupported_country"
            @change="save"
          />
        </div>

        <div class="marker-hint">
          三项留空即使用内置默认判据。判定依据参考 lmc999/RegionRestrictionCheck 社区脚本；
          若某服务检测结果全量异常（如 Google 轮换了混淆 ID），可从社区脚本同步最新特征到此更新。
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 面板骨架（.panel-container / h2 / .panel-header-icon / .setting-card /
   .card-* / .setting-item / .item-label / .sub-label）统一走 panel.css 全局定义，
   此处只保留本面板独有的控件样式。 */

/* 输入框 / 下拉 / 数字框统一走 common.css 全局控件体系 */

/* 本面板的 URL / 说明类输入框需要固定宽度以对齐右侧控件列 */
.text-input {
  width: 260px;
}

.speed-source-controls {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 360px;
}

.speed-source-controls .select-input,
.speed-url-input {
  width: 100%;
}

/* 多源容错测速列表编辑器 */
.url-list-editor {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 420px;
  max-width: 100%;
}

.url-chips {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.url-chip {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 4px 8px;
  background: var(--surface-raised);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  font-size: var(--text-xs);
  min-width: 0;
}

.chip-index {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-full);
  background: color-mix(in srgb, var(--accent-cyan-vivid) 18%, transparent);
  color: var(--accent-cyan-vivid);
  font-size: 10px;
  font-weight: var(--weight-bold);
}

.chip-url {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono);
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chip-remove {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-full);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.chip-remove:hover:not(:disabled) {
  color: var(--accent-red);
  background: var(--accent-red-glow);
}

.chip-remove:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.url-add-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

/* 预设下拉与自定义输入框按 1:2 分配剩余宽度，按钮保持内容宽度 */
.url-add-row .select-input {
  flex: 1 1 0;
  min-width: 0;
}

.url-add-row .speed-url-input {
  flex: 2 1 0;
  min-width: 0;
  width: auto;
}

.btn-add-source {
  padding: 5px 12px;
  background: var(--surface-hover);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-xs);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  cursor: pointer;
  white-space: nowrap;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-add-source:hover:not(:disabled) {
  color: var(--accent-cyan-vivid);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
}

.btn-add-source:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.readonly-tag {
  font-size: 10px;
  padding: 1px 6px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  margin-left: 4px;
}

.marker-hint {
  font-size: 11px;
  line-height: 1.6;
  color: var(--text-secondary);
  padding: 8px 12px;
  background: var(--surface-inset);
  border-left: 2px solid color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
  border-radius: 0 8px 8px 0;
}
</style>
