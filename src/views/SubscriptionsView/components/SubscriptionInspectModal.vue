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
const expandedNodeIndex = ref<number | null>(null);
const isFullscreen = ref(false);

// 监听弹窗显示与订阅变更
watch(
  () => [props.visible, props.subscription] as const,
  async ([visible, sub]) => {
    if (visible && sub) {
      searchKeyword.value = "";
      expandedNodeIndex.value = null;
      activeTab.value = "nodes";
      await fetchInspectData(sub.id);
    } else {
      inspectData.value = null;
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

// 展开/收起单个节点 JSON 详情
function toggleNodeExpand(idx: number) {
  expandedNodeIndex.value = expandedNodeIndex.value === idx ? null : idx;
}

// 格式化展示协议 Badge 颜色
function getProtocolBadgeClass(type: string) {
  const t = type.toLowerCase();
  if (t === "vmess") return "bg-blue-500/10 text-blue-400 border-blue-500/20";
  if (t === "vless") return "bg-purple-500/10 text-purple-400 border-purple-500/20";
  if (t === "shadowsocks" || t === "ss") return "bg-emerald-500/10 text-emerald-400 border-emerald-500/20";
  if (t === "trojan") return "bg-amber-500/10 text-amber-400 border-amber-500/20";
  if (t.includes("hysteria") || t.includes("hy2")) return "bg-rose-500/10 text-rose-400 border-rose-500/20";
  return "bg-gray-500/10 text-gray-400 border-gray-500/20";
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-md animate-fade-in"
      @click.self="closeModal"
    >
      <div
        class="bg-[#12141a] border border-white/10 rounded-2xl shadow-2xl flex flex-col transition-all duration-200 overflow-hidden"
        :class="[
          isFullscreen
            ? 'w-full h-full max-w-none max-h-none rounded-none'
            : 'w-full max-w-5xl h-[88vh] max-h-[860px]'
        ]"
      >
        <!-- 头部 Header -->
        <div class="flex items-center justify-between px-6 py-4 border-b border-white/5 bg-[#161922]">
          <div class="flex items-center gap-3 min-w-0">
            <div class="w-9 h-9 rounded-xl bg-primary/10 border border-primary/20 flex items-center justify-center text-primary shrink-0">
              <BaseIcon name="Code" class="w-5 h-5" />
            </div>
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <h3 class="text-base font-semibold text-white truncate">
                  {{ subscription?.name || "查看订阅配置" }}
                </h3>
                <span
                  v-if="inspectData"
                  class="px-2 py-0.5 text-xs font-medium rounded-full bg-white/5 border border-white/10 text-white/70 uppercase shrink-0"
                >
                  {{ inspectData.format }} · {{ inspectData.node_count }} 节点
                </span>
              </div>
              <p class="text-xs text-white/40 truncate mt-0.5">
                {{ subscription?.url || "本地或剪贴板导入" }}
              </p>
            </div>
          </div>

          <div class="flex items-center gap-2 shrink-0">
            <!-- 视图切换 Tabs -->
            <div class="flex p-1 bg-black/30 rounded-xl border border-white/5 text-xs">
              <button
                class="px-3 py-1.5 rounded-lg font-medium transition-colors flex items-center gap-1.5"
                :class="activeTab === 'raw' ? 'bg-primary text-white shadow-sm' : 'text-white/60 hover:text-white'"
                @click="activeTab = 'raw'"
              >
                <BaseIcon name="FileText" class="w-3.5 h-3.5" />
                <span>清洗前原始数据</span>
              </button>
              <button
                class="px-3 py-1.5 rounded-lg font-medium transition-colors flex items-center gap-1.5"
                :class="activeTab === 'nodes' ? 'bg-primary text-white shadow-sm' : 'text-white/60 hover:text-white'"
                @click="activeTab = 'nodes'"
              >
                <BaseIcon name="Server" class="w-3.5 h-3.5" />
                <span>解析后节点 ({{ inspectData?.node_count || 0 }})</span>
              </button>
              <button
                class="px-3 py-1.5 rounded-lg font-medium transition-colors flex items-center gap-1.5"
                :class="activeTab === 'config' ? 'bg-primary text-white shadow-sm' : 'text-white/60 hover:text-white'"
                @click="activeTab = 'config'"
              >
                <BaseIcon name="FileCode" class="w-3.5 h-3.5" />
                <span>最终运行时配置</span>
              </button>
            </div>

            <!-- 最大化/还原 -->
            <button
              class="w-8 h-8 rounded-lg bg-white/5 hover:bg-white/10 text-white/60 hover:text-white flex items-center justify-center transition-colors"
              :title="isFullscreen ? '还原窗口' : '最大化窗口'"
              @click="toggleFullscreen"
            >
              <BaseIcon :name="isFullscreen ? 'Minimize2' : 'Maximize2'" class="w-4 h-4" />
            </button>

            <!-- 关闭按钮 -->
            <button
              class="w-8 h-8 rounded-lg bg-white/5 hover:bg-rose-500/20 text-white/60 hover:text-rose-400 flex items-center justify-center transition-colors"
              title="关闭 (Esc)"
              @click="closeModal"
            >
              <BaseIcon name="X" class="w-4 h-4" />
            </button>
          </div>
        </div>

        <!-- 主体区域 Body -->
        <div class="flex-1 min-h-0 bg-[#0d0f14] overflow-hidden flex flex-col relative">
          <!-- 加载中骨架屏 -->
          <div
            v-if="loading"
            class="absolute inset-0 z-20 flex flex-col items-center justify-center bg-[#0d0f14]/80 backdrop-blur-sm"
          >
            <div class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
            <p class="text-xs text-white/50 mt-3">正在拉取并反编译订阅配置...</p>
          </div>

          <!-- TAB 1: 清洗前原始文本 (Raw Content) -->
          <div v-else-if="activeTab === 'raw'" class="flex-1 flex flex-col min-h-0">
            <div class="flex items-center justify-between px-6 py-2.5 bg-[#141720] border-b border-white/5 text-xs text-white/60">
              <div class="flex items-center gap-2">
                <span>原始文本字符数: {{ inspectData?.raw_content.length || 0 }} 字节</span>
                <span class="text-white/20">|</span>
                <span>包含多行协议 URI 或 Base64 编码数据</span>
              </div>
              <button
                class="px-3 py-1 bg-white/10 hover:bg-white/20 text-white rounded-lg transition-colors flex items-center gap-1.5 font-medium"
                @click="copyText(inspectData?.raw_content || '', '原始订阅文本')"
              >
                <BaseIcon name="Copy" class="w-3.5 h-3.5" />
                <span>复制原始数据</span>
              </button>
            </div>
            <div class="flex-1 overflow-auto p-4 font-mono text-xs text-emerald-400/90 leading-relaxed bg-[#0a0c10] select-text break-all whitespace-pre-wrap">
              {{ inspectData?.raw_content || "# 暂无原始数据" }}
            </div>
          </div>

          <!-- TAB 2: 解析后节点列表 (Parsed Nodes) -->
          <div v-else-if="activeTab === 'nodes'" class="flex-1 flex flex-col min-h-0">
            <!-- 搜索与操作栏 -->
            <div class="flex items-center justify-between px-6 py-3 bg-[#141720] border-b border-white/5 gap-4">
              <div class="relative flex-1 max-w-md">
                <BaseIcon name="Search" class="w-4 h-4 text-white/40 absolute left-3 top-1/2 -translate-y-1/2" />
                <input
                  v-model="searchKeyword"
                  type="text"
                  placeholder="搜索节点名称、协议或服务器地址..."
                  class="w-full bg-[#1c202b] border border-white/10 rounded-xl pl-9 pr-3 py-1.5 text-xs text-white placeholder-white/30 focus:outline-none focus:border-primary transition-colors"
                />
              </div>

              <div class="flex items-center gap-3 text-xs text-white/60">
                <span>显示 {{ filteredNodes.length }} / {{ inspectData?.node_count || 0 }} 个节点</span>
                <button
                  class="px-3 py-1.5 bg-white/10 hover:bg-white/20 text-white rounded-xl transition-colors flex items-center gap-1.5 font-medium"
                  @click="copyText(JSON.stringify(inspectData?.parsed_nodes || [], null, 2), '全部节点列表 JSON')"
                >
                  <BaseIcon name="Copy" class="w-3.5 h-3.5" />
                  <span>复制全部节点 JSON</span>
                </button>
              </div>
            </div>

            <!-- 节点表格/列表 -->
            <div class="flex-1 overflow-y-auto p-4 space-y-2 select-text">
              <div
                v-for="(node, idx) in filteredNodes"
                :key="idx"
                class="bg-[#151821] border border-white/5 rounded-xl overflow-hidden transition-all hover:border-white/15"
              >
                <!-- 节点摘要行 -->
                <div
                  class="flex items-center justify-between px-4 py-2.5 cursor-pointer hover:bg-white/[0.02]"
                  @click="toggleNodeExpand(idx)"
                >
                  <div class="flex items-center gap-3 min-w-0 flex-1">
                    <span class="text-xs text-white/30 w-7 font-mono shrink-0">#{{ idx + 1 }}</span>
                    <span
                      class="px-2 py-0.5 text-[11px] font-mono rounded border shrink-0 uppercase font-semibold"
                      :class="getProtocolBadgeClass(node.type)"
                    >
                      {{ node.type }}
                    </span>
                    <span class="text-sm font-medium text-white/90 truncate">{{ node.tag }}</span>
                  </div>

                  <div class="flex items-center gap-4 shrink-0 text-xs text-white/50">
                    <span v-if="node.server" class="font-mono text-white/40">
                      {{ node.server }}:{{ node.server_port || 0 }}
                    </span>
                    <BaseIcon
                      :name="expandedNodeIndex === idx ? 'ChevronUp' : 'ChevronDown'"
                      class="w-4 h-4 text-white/30"
                    />
                  </div>
                </div>

                <!-- 节点展开后的 JSON 配置详情 -->
                <div
                  v-if="expandedNodeIndex === idx"
                  class="px-4 py-3 bg-[#0a0c10] border-t border-white/5 text-xs font-mono"
                >
                  <div class="flex items-center justify-between mb-2 text-white/40">
                    <span>sing-box 出站配置规范:</span>
                    <button
                      class="text-primary hover:underline flex items-center gap-1"
                      @click.stop="copyText(JSON.stringify(node.raw_json, null, 2), node.tag)"
                    >
                      <BaseIcon name="Copy" class="w-3 h-3" />
                      <span>复制此节点</span>
                    </button>
                  </div>
                  <pre class="text-emerald-400 overflow-x-auto p-2 bg-black/40 rounded-lg whitespace-pre-wrap">{{ JSON.stringify(node.raw_json, null, 2) }}</pre>
                </div>
              </div>

              <!-- 空状态 -->
              <div v-if="filteredNodes.length === 0" class="py-12 text-center text-white/40 text-xs">
                没有找到匹配的节点
              </div>
            </div>
          </div>

          <!-- TAB 3: 最终 sing-box 运行时配置 (Final Config) -->
          <div v-else-if="activeTab === 'config'" class="flex-1 flex flex-col min-h-0">
            <div class="flex items-center justify-between px-6 py-2.5 bg-[#141720] border-b border-white/5 text-xs text-white/60">
              <div class="flex items-center gap-2">
                <span>完整 sing-box config.json (包含 Inbounds, Outbounds, Route, DNS, ClashAPI)</span>
              </div>
              <button
                class="px-3 py-1 bg-primary hover:bg-primary/90 text-white rounded-lg transition-colors flex items-center gap-1.5 font-medium shadow-sm"
                @click="copyText(inspectData?.final_config_json || '', '完整 config.json')"
              >
                <BaseIcon name="Copy" class="w-3.5 h-3.5" />
                <span>复制完整配置 JSON</span>
              </button>
            </div>
            <div class="flex-1 overflow-auto p-4 font-mono text-xs text-cyan-300 leading-relaxed bg-[#0a0c10] select-text break-all whitespace-pre-wrap">
              {{ inspectData?.final_config_json || "# 暂无配置 JSON" }}
            </div>
          </div>
        </div>

        <!-- 底部 Footer -->
        <div class="flex items-center justify-between px-6 py-3.5 border-t border-white/5 bg-[#161922] text-xs text-white/40">
          <div>
            <span>提示: 该视图展示该订阅经解析清洗后的全部出站与最终由 sing-box 执行的配置详情。</span>
          </div>
          <button
            class="px-4 py-1.5 bg-white/10 hover:bg-white/20 text-white rounded-xl font-medium transition-colors"
            @click="closeModal"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
