<script setup lang="ts">
/**
 * 订阅完整配置深度查看器模态框 (多端适配)
 * 作者: TanXiang
 *
 * 功能：
 * - 多端响应式宽屏与紧凑自适应布局 (macOS/Windows/Linux 桌面及不同分辨率)
 * - 3 维视图切换：
 *   1. 清洗前原始文本 (Raw Content) - 原始未经处理的订阅文本
 *   2. 清洗后出站节点 (Parsed Nodes) - 协议类型、服务器端口、JSON 结构查看与快速过滤
 *   3. 最终 sing-box 运行时配置 (Final Config) - 该订阅生效后生成的完整 sing-box JSON
 * - 一键复制、节点检索、快捷键关闭 (Esc)
 *
 * 样式基于项目公共 modal 体系（modal-backdrop/modal-card）+ scoped token 化样式，
 * 修复历史版本使用未接入的 Tailwind 原子类导致整窗裸奔渲染的缺陷。
 */
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { inspectSubscription } from "@/api/ipc/subscription";
import { useToast } from "@/composables/useToast";
import type { Subscription, SubscriptionInspectData, ParsedOutboundNode } from "@/types";

const props = defineProps<{
  visible: boolean;
  subscription: Subscription | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", val: boolean): void;
}>();

const toast = useToast();

type InspectTab = "raw" | "nodes" | "config";
const activeTab = ref<InspectTab>("nodes");

const loading = ref(false);
const inspectData = ref<SubscriptionInspectData | null>(null);
const searchKeyword = ref("");
/** 当前展开详情的节点 tag（以 tag 而非索引为键，过滤后展开态不漂移） */
const expandedNodeTag = ref<string | null>(null);
const isFullscreen = ref(false);
/** 原始数据视图：默认显示解码后的明文（Base64 订阅），可切换查看原始缓存内容 */
const showOriginalRaw = ref(false);

/** 当前展示的原始文本：解码后明文 or 原始 Base64 缓存 */
const displayRawContent = computed(() => {
  if (!inspectData.value) return "";
  if (showOriginalRaw.value && inspectData.value.raw_content_original) {
    return inspectData.value.raw_content_original;
  }
  return inspectData.value.raw_content;
});

// 已拉取过详情的订阅 ID，避免同一订阅反复打开时重复请求
let lastFetchedSubId: string | null = null;

// 监听弹窗显示：仅由 visible 驱动重置 UI；
// 订阅对象引用可能每次打开都变化，故比较 sub.id 判断是否需要重新拉取
// （同时监听订阅 id：弹窗保持打开时切换订阅对象也能刷新数据）
watch(
  () => [props.visible, props.subscription?.id] as const,
  async ([visible]) => {
    const sub = props.subscription;
    if (visible && sub) {
      searchKeyword.value = "";
      expandedNodeTag.value = null;
      showOriginalRaw.value = false;
      activeTab.value = "nodes";
      if (sub.id !== lastFetchedSubId || !inspectData.value) {
        await fetchInspectData(sub.id);
      }
    } else {
      inspectData.value = null;
      lastFetchedSubId = null;
    }
  },
  { immediate: true }
);

// 键盘快捷键监听
function handleKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape" && props.visible) {
    closeModal();
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeyDown);
});

async function fetchInspectData(subId: string) {
  loading.value = true;
  try {
    const res = await inspectSubscription(subId);
    if (res.success && res.data) {
      inspectData.value = res.data;
    } else {
      toast.error(res.error || "获取配置详情失败");
    }
  } catch (e: any) {
    toast.error(`加载配置失败: ${e?.message || e}`);
  } finally {
    loading.value = false;
  }
}

function closeModal() {
  emit("update:visible", false);
}

function toggleFullscreen() {
  isFullscreen.value = !isFullscreen.value;
}

// 过滤后的节点列表
const filteredNodes = computed<ParsedOutboundNode[]>(() => {
  if (!inspectData.value) return [];
  const list = inspectData.value.parsed_nodes || [];
  const kw = searchKeyword.value.trim().toLowerCase();
  if (!kw) return list;
  return list.filter(
    (n) =>
      n.tag.toLowerCase().includes(kw) ||
      n.type.toLowerCase().includes(kw) ||
      (n.server && n.server.toLowerCase().includes(kw))
  );
});

// 复制文本到剪贴板
async function copyText(text: string, label: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(`${label}已成功复制到剪贴板`);
  } catch (err) {
    toast.error("复制到剪贴板失败，请手动选择复制");
  }
}

// 展开/收起单个节点 JSON 详情（以 tag 为键：过滤后索引变化不会导致展开态漂移到其他节点）
function toggleNodeExpand(tag: string) {
  expandedNodeTag.value = expandedNodeTag.value === tag ? null : tag;
}

// 格式化展示协议 Badge 颜色（token 化语义类，scoped 样式表内定义）
function getProtocolBadgeClass(type: string) {
  const t = type.toLowerCase();
  if (t === "vmess") return "badge badge-vmess";
  if (t === "vless") return "badge badge-vless";
  if (t === "shadowsocks" || t === "ss") return "badge badge-ss";
  if (t === "trojan") return "badge badge-trojan";
  if (t.includes("hysteria") || t.includes("hy2")) return "badge badge-hy2";
  return "badge badge-other";
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="modal-backdrop"
      :class="{ fullscreen: isFullscreen }"
    >
      <div class="inspect-modal" :class="{ fullscreen: isFullscreen }">
        <!-- 头部 -->
        <div class="inspect-header">
          <div class="header-left">
            <div class="header-icon"><BaseIcon name="Code" :size="18" /></div>
            <div class="header-text">
              <div class="title-row">
                <h3 class="modal-title">{{ subscription?.name || "查看订阅配置" }}</h3>
                <span v-if="inspectData" class="count-chip">
                  {{ inspectData.format }} · {{ inspectData.node_count }} 节点
                </span>
              </div>
              <p class="header-sub">{{ subscription?.url || "本地或剪贴板导入" }}</p>
            </div>
          </div>

          <div class="header-actions">
            <!-- 视图切换 -->
            <div class="tab-switch">
              <button
                class="tab-btn"
                :class="{ active: activeTab === 'raw' }"
                @click="activeTab = 'raw'"
              >
                <BaseIcon name="FileText" :size="13" />
                <span>清洗前原始数据</span>
              </button>
              <button
                class="tab-btn"
                :class="{ active: activeTab === 'nodes' }"
                @click="activeTab = 'nodes'"
              >
                <BaseIcon name="Server" :size="13" />
                <span>解析后节点 ({{ inspectData?.node_count || 0 }})</span>
              </button>
              <button
                class="tab-btn"
                :class="{ active: activeTab === 'config' }"
                @click="activeTab = 'config'"
              >
                <BaseIcon name="FileCode" :size="13" />
                <span>最终运行时配置</span>
              </button>
            </div>

            <button class="icon-btn" :title="isFullscreen ? '还原窗口' : '最大化窗口'" @click="toggleFullscreen">
              <BaseIcon :name="isFullscreen ? 'Minimize2' : 'Maximize2'" :size="15" />
            </button>
            <button class="icon-btn danger" title="关闭 (Esc)" @click="closeModal">
              <BaseIcon name="X" :size="15" />
            </button>
          </div>
        </div>

        <!-- 主体 -->
        <div class="inspect-body">
          <!-- 加载中 -->
          <div v-if="loading" class="loading-mask">
            <span class="spin-ring"></span>
            <p>正在拉取并反编译订阅配置...</p>
          </div>

          <!-- TAB 1: 原始文本 -->
          <div v-else-if="activeTab === 'raw'" class="tab-pane">
            <div class="pane-toolbar">
              <span>原始文本字符数: {{ displayRawContent.length || 0 }} 字节</span>
              <div class="toolbar-right">
                <button
                  v-if="inspectData?.raw_content_original"
                  class="btn-copy"
                  :title="showOriginalRaw ? '切换为解码后的明文内容' : '切换为解码前的原始 Base64 缓存'"
                  @click="showOriginalRaw = !showOriginalRaw"
                >
                  <BaseIcon :name="showOriginalRaw ? 'FileText' : 'FileCode'" :size="13" />
                  {{ showOriginalRaw ? "查看解码明文" : "查看原始 Base64" }}
                </button>
                <button class="btn-copy" @click="copyText(displayRawContent, showOriginalRaw ? '原始 Base64 数据' : '解码后的订阅明文')">
                  <BaseIcon name="Copy" :size="13" /> 复制{{ showOriginalRaw ? "原始数据" : "解码明文" }}
                </button>
              </div>
            </div>
            <div class="raw-content">{{ displayRawContent || "# 暂无原始数据" }}</div>
          </div>

          <!-- TAB 2: 解析后节点 -->
          <div v-else-if="activeTab === 'nodes'" class="tab-pane">
            <div class="pane-toolbar with-search">
              <div class="search-wrap">
                <BaseIcon name="Search" :size="14" class="search-icon" />
                <input
                  v-model="searchKeyword"
                  type="text"
                  placeholder="搜索节点名称、协议或服务器地址..."
                  class="form-input search-input"
                />
              </div>
              <div class="toolbar-right">
                <span>显示 {{ filteredNodes.length }} / {{ inspectData?.node_count || 0 }} 个节点</span>
                <button
                  class="btn-copy"
                  @click="copyText(JSON.stringify(inspectData?.parsed_nodes || [], null, 2), '全部节点列表 JSON')"
                >
                  <BaseIcon name="Copy" :size="13" /> 复制全部节点 JSON
                </button>
              </div>
            </div>

            <div class="nodes-list">
              <div
                v-for="(node, idx) in filteredNodes"
                :key="`${idx}-${node.tag}`"
                class="node-row"
              >
                <div class="node-summary" @click="toggleNodeExpand(node.tag)">
                  <div class="summary-left">
                    <span class="node-idx">#{{ idx + 1 }}</span>
                    <span :class="getProtocolBadgeClass(node.type)">{{ node.type }}</span>
                    <span class="node-tag">{{ node.tag }}</span>
                  </div>
                  <div class="summary-right">
                    <span v-if="node.server" class="node-endpoint">
                      {{ node.server }}:{{ node.server_port || 0 }}
                    </span>
                    <BaseIcon
                      :name="expandedNodeTag === node.tag ? 'ChevronUp' : 'ChevronDown'"
                      :size="14"
                      class="chev"
                    />
                  </div>
                </div>

                <div v-if="expandedNodeTag === node.tag" class="node-detail">
                  <div class="detail-head">
                    <span>sing-box 出站配置规范:</span>
                    <button class="btn-copy small" @click.stop="copyText(JSON.stringify(node.raw_json, null, 2), node.tag)">
                      <BaseIcon name="Copy" :size="12" /> 复制此节点
                    </button>
                  </div>
                  <pre class="json-block">{{ JSON.stringify(node.raw_json, null, 2) }}</pre>
                </div>
              </div>

              <div v-if="filteredNodes.length === 0" class="empty-pane">没有找到匹配的节点</div>
            </div>
          </div>

          <!-- TAB 3: 最终运行时配置 -->
          <div v-else-if="activeTab === 'config'" class="tab-pane">
            <div class="pane-toolbar">
              <span>完整 sing-box config.json (包含 Inbounds, Outbounds, Route, DNS, ClashAPI)</span>
              <button class="btn-copy primary" @click="copyText(inspectData?.final_config_json || '', '完整 config.json')">
                <BaseIcon name="Copy" :size="13" /> 复制完整配置 JSON
              </button>
            </div>
            <div class="raw-content cyan">{{ inspectData?.final_config_json || "# 暂无配置 JSON" }}</div>
          </div>
        </div>

        <!-- 底部 -->
        <div class="inspect-footer">
          <span>提示: 该视图展示该订阅经解析清洗后的全部出站与最终由 sing-box 执行的配置详情。</span>
          <button class="btn-copy" @click="closeModal">关闭</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
/* 蒙层复用 modal-backdrop 语义，叠加全屏态 */
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-5);
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: var(--blur-panel);
}

.modal-backdrop.fullscreen {
  padding: 0;
}

.inspect-modal {
  display: flex;
  flex-direction: column;
  width: 100%;
  max-width: 960px;
  height: 86vh;
  max-height: 860px;
  overflow: hidden;
  background: var(--layer-1);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-glow, 0 24px 64px rgba(0, 0, 0, 0.6));
}

.inspect-modal.fullscreen {
  max-width: none;
  max-height: none;
  height: 100%;
  width: 100%;
  border-radius: 0;
  border: none;
}

/* ---- 头部 ---- */
.inspect-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-3);
  padding: 14px 20px;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--layer-2);
}

.header-left {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.header-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  color: var(--accent-cyan-vivid);
  background: color-mix(in srgb, var(--accent-cyan-vivid) 8%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 20%, transparent);
  border-radius: var(--radius-md);
}

.header-text {
  min-width: 0;
}

.title-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.modal-title {
  margin: 0;
  font-size: var(--text-base);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.count-chip {
  flex-shrink: 0;
  font-size: 10px;
  font-weight: var(--weight-semibold);
  text-transform: uppercase;
  color: var(--text-secondary);
  padding: 2px 8px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full);
}

.header-sub {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

/* 视图切换 tab（青色激活语义） */
.tab-switch {
  display: flex;
  gap: 4px;
  padding: 3px;
  background: var(--layer-3, rgba(0, 0, 0, 0.3));
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.tab-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  font-size: 11px;
  font-weight: var(--weight-semibold);
  color: var(--text-tertiary);
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.tab-btn:hover {
  color: var(--text-primary);
}

.tab-btn.active {
  color: var(--accent-cyan-vivid);
  background: var(--accent-cyan-glow, color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent));
}

.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  color: var(--text-tertiary);
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.icon-btn:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.08);
}

.icon-btn.danger:hover {
  color: var(--status-danger);
  background: rgba(248, 113, 113, 0.1);
}

/* ---- 主体 ---- */
.inspect-body {
  flex: 1;
  min-height: 0;
  position: relative;
  display: flex;
  flex-direction: column;
  background: var(--layer-0);
}

.loading-mask {
  position: absolute;
  inset: 0;
  z-index: 20;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  background: rgba(10, 12, 18, 0.8);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.spin-ring {
  width: 26px;
  height: 26px;
  border: 2px solid color-mix(in srgb, var(--accent-cyan-vivid) 25%, transparent);
  border-top-color: var(--accent-cyan-vivid);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.tab-pane {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.pane-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-3);
  padding: 8px 20px;
  font-size: 11px;
  color: var(--text-tertiary);
  background: var(--layer-2);
  border-bottom: 1px solid var(--border-subtle);
}

.pane-toolbar.with-search {
  padding: 10px 20px;
}

.search-wrap {
  position: relative;
  flex: 1;
  max-width: 380px;
}

.search-icon {
  position: absolute;
  left: 10px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--text-tertiary);
}

.search-input {
  width: 100%;
  padding-left: 32px;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-shrink: 0;
}

.btn-copy {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  font-size: 11px;
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-copy:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.1);
}

.btn-copy.primary {
  color: var(--accent-cyan-vivid);
  background: var(--accent-cyan-glow, color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent));
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
}

.btn-copy.primary:hover {
  color: #fff;
  background: color-mix(in srgb, var(--accent-cyan-vivid) 20%, transparent);
}

.btn-copy.small {
  padding: 3px 8px;
  font-size: 10px;
}

/* 原始内容/配置 JSON 展示区 */
.raw-content {
  flex: 1;
  overflow: auto;
  padding: var(--space-4);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.7;
  color: var(--status-success);
  background: rgba(10, 12, 16, 0.6);
  white-space: pre-wrap;
  word-break: break-all;
  user-select: text;
}

.raw-content.cyan {
  color: var(--accent-cyan-vivid);
}

/* ---- 节点列表 ---- */
.nodes-list {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: 8px;
  user-select: text;
}

.node-row {
  /* 关键：禁止 flex 压缩。nodes-list 是固定高度纵向 flex 容器，
     大订阅（数百节点）时子项默认 flex-shrink:1 会被压缩成 2px 横线，
     必须保持自然行高让容器滚动 */
  flex-shrink: 0;
  overflow: hidden;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  transition: border-color var(--duration-fast) var(--ease-out);
}

.node-row:hover {
  border-color: var(--border-normal);
}

.node-summary {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-3);
  padding: 9px 14px;
  cursor: pointer;
}

.node-summary:hover {
  background: rgba(255, 255, 255, 0.02);
}

.summary-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1;
}

.node-idx {
  flex-shrink: 0;
  width: 26px;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-tertiary);
}

/* 协议徽章（token 化色板） */
.badge {
  flex-shrink: 0;
  padding: 1px 7px;
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: var(--weight-bold);
  text-transform: uppercase;
  border-radius: var(--radius-xs);
  border: 1px solid;
}

.badge-vmess { color: #60a5fa; background: rgba(96, 165, 250, 0.1); border-color: rgba(96, 165, 250, 0.25); }
.badge-vless { color: #a78bfa; background: rgba(167, 139, 250, 0.1); border-color: rgba(167, 139, 250, 0.25); }
.badge-ss { color: #34d399; background: rgba(52, 211, 153, 0.1); border-color: rgba(52, 211, 153, 0.25); }
.badge-trojan { color: #fbbf24; background: rgba(251, 191, 36, 0.1); border-color: rgba(251, 191, 36, 0.25); }
.badge-hy2 { color: #fb7185; background: rgba(251, 113, 133, 0.1); border-color: rgba(251, 113, 133, 0.25); }
.badge-other { color: var(--text-tertiary); background: rgba(255, 255, 255, 0.06); border-color: var(--border-subtle); }

.node-tag {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.summary-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-shrink: 0;
  font-size: 11px;
  color: var(--text-tertiary);
}

.node-endpoint {
  font-family: var(--font-mono);
}

.chev {
  color: var(--text-tertiary);
}

.node-detail {
  padding: 10px 14px;
  background: rgba(10, 12, 16, 0.7);
  border-top: 1px solid var(--border-subtle);
  font-family: var(--font-mono);
  font-size: 11px;
}

.detail-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  color: var(--text-tertiary);
}

.json-block {
  margin: 0;
  padding: 8px;
  overflow-x: auto;
  font-family: var(--font-mono);
  font-size: 10.5px;
  line-height: 1.6;
  color: var(--status-success);
  background: rgba(0, 0, 0, 0.35);
  border-radius: var(--radius-sm);
  white-space: pre-wrap;
  word-break: break-all;
}

.empty-pane {
  padding: 48px 0;
  text-align: center;
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

/* ---- 底部 ---- */
.inspect-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 20px;
  font-size: 11px;
  color: var(--text-tertiary);
  background: var(--layer-2);
  border-top: 1px solid var(--border-subtle);
}
</style>
