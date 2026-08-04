<script setup lang="ts">
/**
 * 代理节点视图 (重构：双栏行列表布局，统一数据源)
 * 作者: TanXiang
 */
import { onMounted, ref, computed } from "vue";
import { useRoute } from "vue-router";
import { useProxyStore } from "@/stores/proxy.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useToast } from "@/composables/useToast";
import { storeToRefs } from "pinia";
import NodeCard from "@/components/proxy/NodeCard.vue";
import SvgIcon from "@/components/common/SvgIcon.vue";
import type { ProxyNode, NodeSortConfig, CustomGroupRule } from "@/types";

const proxyStore = useProxyStore();
const speedtestStore = useSpeedtestStore();
const toast = useToast();
const route = useRoute();

const { groups, loading } = storeToRefs(proxyStore);
const selectedGroupTag = ref<string>("");
const showConfirmModal = ref(false);
const searchText = ref("");

// 排序状态
const sortConfig = ref<NodeSortConfig>({ key: "default", order: "asc" });

// 分组分类：内置系统分组 vs 地区分组
const systemGroupTags = ["proxy", "auto", "balance"];
const systemGroups = computed(() => {
  return groups.value.filter(g => systemGroupTags.includes(g.tag));
});

const regionGroups = computed(() => {
  return groups.value.filter(g => !systemGroupTags.includes(g.tag) && g.type === "urltest");
});

onMounted(async () => {
  proxyStore.loadCustomGroupRules();
  await proxyStore.fetchGroups();
  await speedtestStore.init();
  if (groups.value.length > 0) {
    const qGroup = route.query.group as string;
    const exists = groups.value.some((g) => g.tag === qGroup);
    selectedGroupTag.value = exists ? qGroup : groups.value[0].tag;
    await proxyStore.fetchGroupNodes(selectedGroupTag.value);
  }
});

// 当前分组的原始节点列表
const rawNodes = computed<ProxyNode[]>(() => {
  const nodes = proxyStore.nodeMap.get(selectedGroupTag.value);
  return nodes ?? [];
});

// 经搜索过滤 + 排序后的节点列表
const displayNodes = computed<ProxyNode[]>(() => {
  let nodes = rawNodes.value;

  // 搜索过滤
  if (searchText.value.trim()) {
    const kw = searchText.value.trim().toLowerCase();
    nodes = nodes.filter(n => n.tag.toLowerCase().includes(kw) || n.type.toLowerCase().includes(kw));
  }

  // 排序
  if (sortConfig.value.key === "default") return nodes;

  const sorted = [...nodes];
  const { key, order } = sortConfig.value;
  const multiplier = order === "asc" ? 1 : -1;

  sorted.sort((a, b) => {
    if (key === "name") {
      return a.tag.localeCompare(b.tag, "zh-CN") * multiplier;
    } else if (key === "protocol") {
      return a.type.localeCompare(b.type) * multiplier;
    } else if (key === "latency") {
      const aLat = speedtestStore.latencyMap[a.tag];
      const bLat = speedtestStore.latencyMap[b.tag];
      // 未测试的排最后
      if (aLat === undefined && bLat === undefined) return 0;
      if (aLat === undefined) return 1;
      if (bLat === undefined) return -1;
      return (aLat - bLat) * multiplier;
    }
    return 0;
  });

  return sorted;
});

// 当前分组对象
const currentGroup = computed(() => {
  return groups.value.find((x) => x.tag === selectedGroupTag.value);
});

const isSelectorGroup = computed(() => {
  return currentGroup.value?.type === "selector";
});

// 计算当前处于激活出口链路上的所有策略组 tag
const routingGroupTags = computed(() => {
  const tags = new Set<string>();
  const primary = groups.value.find((g) => g.type === "selector");
  if (!primary) return tags;

  tags.add(primary.tag);

  let currentTagName = primary.now;
  for (let i = 0; i < 10 && currentTagName; i++) {
    const nextGroup = groups.value.find((g) => g.tag === currentTagName);
    if (nextGroup) {
      tags.add(nextGroup.tag);
      currentTagName = nextGroup.now;
    } else {
      break;
    }
  }
  return tags;
});

async function handleGroupSelect(groupTag: string) {
  selectedGroupTag.value = groupTag;
  searchText.value = "";
  await proxyStore.fetchGroupNodes(groupTag);
  proxyStore.recordGroupUsage(groupTag);
}

async function handleNodeSelect(nodeTag: string) {
  if (!selectedGroupTag.value) return;
  if (!isSelectorGroup.value) {
    toast.warning("不支持切换", "该策略组为自动或非手动选择类型，无法手动指定节点。");
    return;
  }
  const res = await proxyStore.selectNode(selectedGroupTag.value, nodeTag);
  if (res && res.success) {
    toast.success("节点已切换", `当前出站: ${nodeTag}`);
  } else {
    toast.error("切换节点失败", res?.error || "未知错误");
  }
}

async function handleRunLatency() {
  if (!selectedGroupTag.value) return;
  toast.info("正在并发测试延迟...");
  const nodes = proxyStore.nodeMap.get(selectedGroupTag.value) ?? [];
  const tags = nodes
    .filter((n) => !["selector", "urltest", "fallback"].includes(n.type.toLowerCase()))
    .map((n) => n.tag);

  if (tags.length === 0) {
    toast.warning("该策略组内没有可供测试的真实节点");
    return;
  }

  await speedtestStore.testLatency(selectedGroupTag.value, tags);
  const success = Object.values(speedtestStore.latencyMap).filter(v => v > 0).length;
  toast.success("延迟测试完成", `成功 ${success} / 总计 ${tags.length}`);
}

async function handleSingleLatency(nodeTag: string) {
  if (!selectedGroupTag.value) return;
  await speedtestStore.testLatency(selectedGroupTag.value, [nodeTag]);
}

async function handleSingleSpeed(nodeTag: string) {
  toast.info("开始节点吞吐量测试", `正在测试: ${nodeTag}`);
  const res = await speedtestStore.testSingleThroughput(nodeTag);
  if (res.success && res.data) {
    const mbps = (res.data.download_bps / (1024 * 1024)).toFixed(1);
    toast.success("单节点测速完成", `${nodeTag}: ${mbps} MB/s`);
  } else {
    toast.error("测速失败", res.error);
  }
}

async function confirmBatchSpeedTest() {
  showConfirmModal.value = false;
  if (!selectedGroupTag.value) return;
  await speedtestStore.startBatchTest(selectedGroupTag.value, rawNodes.value.map(n => n.tag));
  toast.info("已启动批量串行测速任务");
}

const sortLabels: Record<string, string> = {
  default: "默认排序",
  name: "按名称",
  latency: "按延迟",
  protocol: "按协议",
};

function cycleSortKey() {
  const keys: Array<NodeSortConfig["key"]> = ["default", "name", "latency", "protocol"];
  const idx = keys.indexOf(sortConfig.value.key);
  const nextKey = keys[(idx + 1) % keys.length];
  sortConfig.value = { key: nextKey, order: "asc" };
}

function toggleSortOrder() {
  sortConfig.value = { ...sortConfig.value, order: sortConfig.value.order === "asc" ? "desc" : "asc" };
}

// ============ 分组配置编辑 ============
const showGroupEditModal = ref(false);
const editingGroupTag = ref("");
const editingGroupType = ref("");
const editingGroupConfig = ref<Record<string, any>>({});

function openGroupEdit(groupTag: string) {
  const group = groups.value.find(g => g.tag === groupTag);
  if (!group) return;
  editingGroupTag.value = groupTag;
  editingGroupType.value = group.type;
  // 初始化可编辑配置项
  editingGroupConfig.value = {
    interval: (group as any).interval || "15m",
    tolerance: (group as any).tolerance || 50,
    url: (group as any).url || "https://www.gstatic.com/generate_204",
  };
  showGroupEditModal.value = true;
}

async function saveGroupConfig() {
  // 目前仅通过刷新配置实现，后续可通过 ClashAPI 直接修改
  toast.info("配置已更新", "将在下次刷新订阅时生效");
  showGroupEditModal.value = false;
}

// ============ 自定义区域规则 ============
const showRegionModal = ref(false);
const editingRule = ref<CustomGroupRule | null>(null);
const isNewRule = ref(false);

// 内置区域选项 — 按洲分类
const builtinRegions = [
  // ---- 亚洲 ----
  { name: "香港", keywords: ["HK", "Hong Kong", "香港", "🇭🇰"] },
  { name: "台湾", keywords: ["TW", "Taiwan", "台湾", "台灣", "🇹🇼"] },
  { name: "日本", keywords: ["JP", "Japan", "日本", "🇯🇵"] },
  { name: "韩国", keywords: ["KR", "Korea", "韩国", "韓國", "🇰🇷"] },
  { name: "新加坡", keywords: ["SG", "Singapore", "新加坡", "🇸🇬"] },
  { name: "马来西亚", keywords: ["MY", "Malaysia", "马来西亚", "馬來西亞", "🇲🇾"] },
  { name: "泰国", keywords: ["TH", "Thailand", "泰国", "泰國", "🇹🇭"] },
  { name: "越南", keywords: ["VN", "Vietnam", "越南", "🇻🇳"] },
  { name: "印度尼西亚", keywords: ["ID", "Indonesia", "印尼", "印度尼西亚", "🇮🇩"] },
  { name: "菲律宾", keywords: ["PH", "Philippines", "菲律宾", "菲律賓", "🇵🇭"] },
  { name: "印度", keywords: ["IN", "India", "印度", "🇮🇳"] },
  { name: "澳门", keywords: ["MO", "Macao", "Macau", "澳门", "澳門", "🇲🇴"] },
  { name: "柬埔寨", keywords: ["KH", "Cambodia", "柬埔寨", "🇰🇭"] },
  { name: "孟加拉国", keywords: ["BD", "Bangladesh", "孟加拉", "🇧🇩"] },
  { name: "巴基斯坦", keywords: ["PK", "Pakistan", "巴基斯坦", "🇵🇰"] },
  { name: "哈萨克斯坦", keywords: ["KZ", "Kazakhstan", "哈萨克斯坦", "🇰🇿"] },

  // ---- 大洋洲 ----
  { name: "澳大利亚", keywords: ["AU", "Australia", "澳大利亚", "澳洲", "🇦🇺"] },
  { name: "新西兰", keywords: ["NZ", "New Zealand", "新西兰", "紐西蘭", "🇳🇿"] },

  // ---- 北美洲 ----
  { name: "美国", keywords: ["US", "USA", "United States", "America", "美国", "🇺🇸"] },
  { name: "加拿大", keywords: ["CA", "Canada", "加拿大", "🇨🇦"] },
  { name: "墨西哥", keywords: ["MX", "Mexico", "墨西哥", "🇲🇽"] },

  // ---- 南美洲 ----
  { name: "巴西", keywords: ["BR", "Brazil", "巴西", "🇧🇷"] },
  { name: "阿根廷", keywords: ["AR", "Argentina", "阿根廷", "🇦🇷"] },
  { name: "智利", keywords: ["CL", "Chile", "智利", "🇨🇱"] },
  { name: "哥伦比亚", keywords: ["CO", "Colombia", "哥伦比亚", "🇨🇴"] },
  { name: "秘鲁", keywords: ["PE", "Peru", "秘鲁", "🇵🇪"] },

  // ---- 欧洲 ----
  { name: "英国", keywords: ["UK", "GB", "United Kingdom", "英国", "🇬🇧"] },
  { name: "德国", keywords: ["DE", "Germany", "德国", "德國", "🇩🇪"] },
  { name: "法国", keywords: ["FR", "France", "法国", "法國", "🇫🇷"] },
  { name: "荷兰", keywords: ["NL", "Netherlands", "Holland", "荷兰", "荷蘭", "🇳🇱"] },
  { name: "意大利", keywords: ["IT", "Italy", "意大利", "🇮🇹"] },
  { name: "西班牙", keywords: ["ES", "Spain", "西班牙", "🇪🇸"] },
  { name: "瑞士", keywords: ["CH", "Switzerland", "瑞士", "🇨🇭"] },
  { name: "瑞典", keywords: ["SE", "Sweden", "瑞典", "🇸🇪"] },
  { name: "挪威", keywords: ["NO", "Norway", "挪威", "🇳🇴"] },
  { name: "芬兰", keywords: ["FI", "Finland", "芬兰", "🇫🇮"] },
  { name: "丹麦", keywords: ["DK", "Denmark", "丹麦", "🇩🇰"] },
  { name: "波兰", keywords: ["PL", "Poland", "波兰", "🇵🇱"] },
  { name: "乌克兰", keywords: ["UA", "Ukraine", "乌克兰", "🇺🇦"] },
  { name: "捷克", keywords: ["CZ", "Czech", "捷克", "🇨🇿"] },
  { name: "奥地利", keywords: ["AT", "Austria", "奥地利", "🇦🇹"] },
  { name: "比利时", keywords: ["BE", "Belgium", "比利时", "🇧🇪"] },
  { name: "爱尔兰", keywords: ["IE", "Ireland", "爱尔兰", "🇮🇪"] },
  { name: "葡萄牙", keywords: ["PT", "Portugal", "葡萄牙", "🇵🇹"] },
  { name: "希腊", keywords: ["GR", "Greece", "希腊", "🇬🇷"] },
  { name: "罗马尼亚", keywords: ["RO", "Romania", "罗马尼亚", "🇷🇴"] },
  { name: "保加利亚", keywords: ["BG", "Bulgaria", "保加利亚", "🇧🇬"] },
  { name: "塞尔维亚", keywords: ["RS", "Serbia", "塞尔维亚", "🇷🇸"] },

  // ---- 俄罗斯/独联体 ----
  { name: "俄罗斯", keywords: ["RU", "Russia", "俄罗斯", "🇷🇺"] },
  { name: "白俄罗斯", keywords: ["BY", "Belarus", "白俄罗斯", "🇧🇾"] },

  // ---- 中东 ----
  { name: "土耳其", keywords: ["TR", "Turkey", "Türkiye", "土耳其", "🇹🇷"] },
  { name: "阿联酋", keywords: ["AE", "UAE", "United Arab Emirates", "阿联酋", "迪拜", "Dubai", "🇦🇪"] },
  { name: "以色列", keywords: ["IL", "Israel", "以色列", "🇮🇱"] },
  { name: "沙特阿拉伯", keywords: ["SA", "Saudi Arabia", "沙特", "🇸🇦"] },
  { name: "伊朗", keywords: ["IR", "Iran", "伊朗", "🇮🇷"] },

  // ---- 非洲 ----
  { name: "南非", keywords: ["ZA", "South Africa", "南非", "🇿🇦"] },
  { name: "埃及", keywords: ["EG", "Egypt", "埃及", "🇪🇬"] },
  { name: "尼日利亚", keywords: ["NG", "Nigeria", "尼日利亚", "🇳🇬"] },
  { name: "肯尼亚", keywords: ["KE", "Kenya", "肯尼亚", "🇰🇪"] },

  // ---- 按洲划分（宽匹配） ----
  { name: "亚洲", keywords: ["Asia", "亚洲", "亞洲", "🇭🇰", "🇯🇵", "🇰🇷", "🇸🇬", "🇹🇼", "🇹🇭", "🇻🇳", "🇮🇩", "🇵🇭", "🇮🇳", "🇲🇾", "🇲🇴"] },
  { name: "欧洲", keywords: ["Europe", "欧洲", "歐洲", "🇬🇧", "🇩🇪", "🇫🇷", "🇳🇱", "🇮🇹", "🇪🇸", "🇨🇭", "🇸🇪", "🇳🇴", "🇫🇮", "🇩🇰", "🇵🇱", "🇺🇦", "🇦🇹", "🇧🇪", "🇮🇪", "🇵🇹", "🇬🇷", "🇷🇴", "🇧🇬", "🇷🇸"] },
  { name: "北美洲", keywords: ["North America", "北美洲", "🇺🇸", "🇨🇦", "🇲🇽"] },
  { name: "南美洲", keywords: ["South America", "南美洲", "🇧🇷", "🇦🇷", "🇨🇱", "🇨🇴", "🇵🇪"] },
  { name: "大洋洲", keywords: ["Oceania", "大洋洲", "🇦🇺", "🇳🇿"] },
  { name: "中东", keywords: ["Middle East", "中东", "中東", "🇹🇷", "🇦🇪", "🇮🇱", "🇸🇦", "🇮🇷"] },
  { name: "非洲", keywords: ["Africa", "非洲", "🇿🇦", "🇪🇬", "🇳🇬", "🇰🇪"] },
];

function openRegionModal() {
  showRegionModal.value = true;
}

function addNewRule() {
  editingRule.value = {
    id: "",
    name: "",
    enabled: true,
    match_type: "keyword",
    keywords: [],
    pattern: "",
    protocols: [],
    order: proxyStore.customGroupRules.length,
  };
  isNewRule.value = true;
}

function editRule(rule: CustomGroupRule) {
  editingRule.value = { ...rule };
  isNewRule.value = false;
}

function applyBuiltinRegion(region: { name: string; keywords: string[] }) {
  if (!editingRule.value) return;
  editingRule.value.name = region.name;
  editingRule.value.match_type = "keyword";
  editingRule.value.keywords = [...region.keywords];
}

function saveRule() {
  if (!editingRule.value) return;
  if (!editingRule.value.name.trim()) {
    toast.warning("请填写区域名称");
    return;
  }
  if (isNewRule.value) {
    const { id, ...ruleData } = editingRule.value;
    proxyStore.addCustomGroupRule(ruleData);
  } else {
    proxyStore.updateCustomGroupRule(editingRule.value);
  }
  editingRule.value = null;
  toast.success("区域规则已保存");
}

function deleteRule(id: string) {
  proxyStore.deleteCustomGroupRule(id);
  toast.success("区域规则已删除");
}
</script>

<template>
  <div class="proxies-view">
    <!-- 批量测速进度条 (全局浮层) -->
    <div v-if="speedtestStore.isBatchTesting && speedtestStore.batchProgress" class="batch-progress-card glass-effect">
      <div class="progress-info">
        <span>正在测速: <strong>{{ speedtestStore.batchProgress.current_node }}</strong></span>
        <span>进度: {{ speedtestStore.batchProgress.current_index }} / {{ speedtestStore.batchProgress.total }}</span>
      </div>
      <div class="progress-bar-bg">
        <div
          class="progress-bar-fill"
          :style="{ width: `${(speedtestStore.batchProgress.current_index / speedtestStore.batchProgress.total) * 100}%` }"
        ></div>
      </div>
      <button class="btn-cancel" @click="speedtestStore.cancelBatch">取消测速</button>
    </div>

    <!-- 双栏布局区域 -->
    <div v-if="groups.length > 0" class="proxies-layout">
      <!-- 左栏: 分组选择器 -->
      <aside class="sidebar-groups glass-effect">
        <!-- 最近常用快捷标签 -->
        <div v-if="proxyStore.recentGroups.length > 1" class="recent-section">
          <span class="section-title">
            <SvgIcon name="clock" :size="12" style="margin-right: 4px;" />
            最近常用
          </span>
          <div class="recent-list">
            <button
              v-for="tag in proxyStore.recentGroups"
              :key="tag"
              class="recent-item"
              :class="{ active: tag === selectedGroupTag }"
              @click="handleGroupSelect(tag)"
            >
              {{ tag }}
            </button>
          </div>
        </div>

        <!-- 系统分组 -->
        <span class="section-title">
          <SvgIcon name="folder" :size="12" style="margin-right: 4px;" />
          主策略组
        </span>
        <div class="groups-list">
          <div
            v-for="group in systemGroups"
            :key="group.tag"
            class="group-item-wrapper"
          >
            <button
              class="group-item"
              :class="{ 
                active: group.tag === selectedGroupTag,
                'in-route': routingGroupTags.has(group.tag)
              }"
              @click="handleGroupSelect(group.tag)"
            >
              <div class="group-header-info">
                <div class="group-name-wrapper">
                  <span v-if="routingGroupTags.has(group.tag)" class="route-dot" title="当前活跃出口链路成员"></span>
                  <span class="group-name">{{ group.tag }}</span>
                </div>
                <span class="group-badge">{{ group.type }}</span>
              </div>
              <span 
                v-if="group.now" 
                class="group-current-node"
                :class="{ 'highlight-now': routingGroupTags.has(group.tag) }"
              >
                {{ group.now }}
              </span>
            </button>
            <button
              v-if="group.tag !== 'proxy'"
              class="btn-group-edit"
              @click.stop="openGroupEdit(group.tag)"
              title="编辑分组配置"
            >
              <SvgIcon name="edit" :size="10" />
            </button>
          </div>
        </div>

        <!-- 地区分组 -->
        <template v-if="regionGroups.length > 0 || true">
          <div class="section-title-row">
            <span class="section-title">
              <SvgIcon name="globe" :size="12" style="margin-right: 4px;" />
              地区分组
            </span>
            <button class="btn-add-region" @click="openRegionModal" title="管理自定义区域">
              <SvgIcon name="plus" :size="12" />
            </button>
          </div>
          <div class="groups-list">
            <button
              v-for="group in regionGroups"
              :key="group.tag"
              class="group-item"
              :class="{ 
                active: group.tag === selectedGroupTag,
                'in-route': routingGroupTags.has(group.tag)
              }"
              @click="handleGroupSelect(group.tag)"
            >
              <div class="group-header-info">
                <div class="group-name-wrapper">
                  <span v-if="routingGroupTags.has(group.tag)" class="route-dot" title="当前活跃出口链路成员"></span>
                  <span class="group-name">{{ group.tag }}</span>
                </div>
                <span class="group-badge">{{ group.type }}</span>
              </div>
              <span 
                v-if="group.now" 
                class="group-current-node"
                :class="{ 'highlight-now': routingGroupTags.has(group.tag) }"
              >
                {{ group.now }}
              </span>
            </button>
          </div>
        </template>
      </aside>

      <!-- 右栏: 节点行列表 -->
      <main class="nodes-content glass-effect">
        <!-- 工具栏：搜索 + 排序 + 操作 -->
        <div class="nodes-toolbar">
          <div class="toolbar-left">
            <h2>{{ selectedGroupTag }}</h2>
            <span class="nodes-count" v-if="rawNodes.length > 0">
              共 {{ rawNodes.length }} 个节点
              <span v-if="searchText.trim()" class="filter-count">（已筛选 {{ displayNodes.length }}）</span>
            </span>
          </div>
          <div class="toolbar-right">
            <!-- 搜索框 -->
            <div class="search-box">
              <SvgIcon name="search" :size="12" class="search-icon" />
              <input
                v-model="searchText"
                type="text"
                placeholder="搜索节点..."
                class="search-input"
              />
              <button v-if="searchText" class="search-clear" @click="searchText = ''">×</button>
            </div>
            <!-- 排序按钮 -->
            <button class="btn-sort" @click="cycleSortKey" :title="'排序: ' + sortLabels[sortConfig.key]">
              <SvgIcon name="sort" :size="12" style="margin-right: 4px;" />
              {{ sortLabels[sortConfig.key] }}
              <span class="sort-order" @click.stop="toggleSortOrder">{{ sortConfig.order === 'asc' ? '↑' : '↓' }}</span>
            </button>
            <!-- 操作按钮 -->
            <button class="btn-action" @click="handleRunLatency" title="延迟测试">
              <SvgIcon name="bolt" :size="12" style="margin-right: 4px;" />
              测延迟
            </button>
            <button class="btn-action" @click="showConfirmModal = true" title="批量测速">
              <SvgIcon name="wifi" :size="12" style="margin-right: 4px;" />
              批量测速
            </button>
            <button class="btn-action" @click="proxyStore.fetchGroups" title="刷新">
              <SvgIcon name="refresh" :size="12" style="margin-right: 4px;" />
            </button>
          </div>
        </div>

        <div v-if="loading" class="state-tip">
          ⏳ 正在加载节点列表...
        </div>
        <div v-else-if="rawNodes.length === 0" class="state-tip">
          📭 暂无节点数据，请点击刷新
        </div>
        <div v-else-if="displayNodes.length === 0" class="state-tip">
          🔍 没有匹配「{{ searchText }}」的节点
        </div>
        <div v-else class="nodes-scroll">
          <div class="nodes-list">
            <NodeCard
              v-for="node in displayNodes"
              :key="node.tag"
              :node-tag="node.tag"
              :node-type="node.type"
              :is-active="node.is_active"
              :latency="speedtestStore.latencyMap[node.tag]"
              :speed-bps="speedtestStore.throughputMap[node.tag]?.download_bps"
              :is-selectable="isSelectorGroup"
              @select="handleNodeSelect(node.tag)"
              @test-latency="handleSingleLatency(node.tag)"
              @test-speed="handleSingleSpeed(node.tag)"
            />
          </div>
        </div>
      </main>
    </div>

    <!-- 异常或空状态 -->
    <div v-else class="state-tip-full">
      <SvgIcon name="proxies" :size="48" style="color: var(--text-tertiary);" />
      <p>未检测到运行中的代理节点组。</p>
      <p class="sub-tip">请确保 Sing-box 后台核心已成功启动且配置导入正确。</p>
      <button class="btn-retry" @click="proxyStore.fetchGroups">
        <SvgIcon name="refresh" :size="12" style="margin-right: 4px;" />
        重试刷新
      </button>
    </div>

    <!-- 批量测速确认 Modal -->
    <Teleport to="body">
      <div v-if="showConfirmModal" class="modal-backdrop" @click.self="showConfirmModal = false">
        <div class="modal-card glass-effect">
          <h3>⚠️ 批量吞吐量测速确认</h3>
          <p>将对分组 <strong>「{{ selectedGroupTag }}」</strong> 的所有节点依次进行带宽测试。</p>
          <div class="estimate-box">
            <div>⏱️ 预计总耗时: 约 {{ Math.ceil(rawNodes.length * speedtestStore.THROUGHPUT_TEST_DURATION_SEC / 60) }} 分钟</div>
            <div>📉 预计流量消耗: 约 {{ Math.ceil(rawNodes.length * (speedtestStore.THROUGHPUT_TEST_CHUNK_BYTES / (1024 * 1024))) }} MB</div>
          </div>
          <p class="warning-tip">测速将以串行队列形式进行，以获得最准确的无干扰结果。</p>
          <div class="modal-actions">
            <button class="btn text" @click="showConfirmModal = false">取消</button>
            <button class="btn primary" @click="confirmBatchSpeedTest">开始测速</button>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- 分组配置编辑 Modal -->
    <Teleport to="body">
      <div v-if="showGroupEditModal" class="modal-backdrop" @click.self="showGroupEditModal = false">
        <div class="modal-card glass-effect">
          <h3>⚙️ 编辑分组配置 — {{ editingGroupTag }}</h3>
          <div class="edit-form">
            <div class="form-row">
              <label>测速间隔</label>
              <select v-model="editingGroupConfig.interval" class="form-input">
                <option value="1m">1 分钟</option>
                <option value="3m">3 分钟</option>
                <option value="5m">5 分钟</option>
                <option value="15m">15 分钟</option>
                <option value="30m">30 分钟</option>
              </select>
            </div>
            <div class="form-row" v-if="editingGroupTag === 'balance'">
              <label>容差 (ms)</label>
              <input v-model.number="editingGroupConfig.tolerance" type="number" class="form-input" min="0" max="500" />
              <span class="form-hint">延迟差在此范围内的节点会被轮询</span>
            </div>
            <div class="form-row">
              <label>测速 URL</label>
              <input v-model="editingGroupConfig.url" type="text" class="form-input" />
            </div>
          </div>
          <div class="modal-actions">
            <button class="btn text" @click="showGroupEditModal = false">取消</button>
            <button class="btn primary" @click="saveGroupConfig">保存</button>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- 自定义区域管理 Modal -->
    <Teleport to="body">
      <div v-if="showRegionModal" class="modal-backdrop" @click.self="showRegionModal = false">
        <div class="modal-card glass-effect region-modal">
          <h3>🌍 自定义区域管理</h3>

          <!-- 编辑/新增表单 -->
          <div v-if="editingRule" class="rule-edit-form">
            <div class="form-row">
              <label>区域名称</label>
              <input v-model="editingRule.name" type="text" class="form-input" placeholder="如: 欧洲" />
            </div>
            <div class="form-row">
              <label>匹配方式</label>
              <select v-model="editingRule.match_type" class="form-input">
                <option value="keyword">关键词匹配</option>
                <option value="regex">正则表达式</option>
                <option value="protocol">协议类型</option>
              </select>
            </div>
            <div v-if="editingRule.match_type === 'keyword'" class="form-row">
              <label>关键词列表</label>
              <input
                :value="editingRule.keywords.join(', ')"
                @input="editingRule.keywords = ($event.target as HTMLInputElement).value.split(',').map(s => s.trim()).filter(Boolean)"
                type="text"
                class="form-input"
                placeholder="用逗号分隔，如: EU, Europe, 欧洲"
              />
            </div>
            <div v-if="editingRule.match_type === 'regex'" class="form-row">
              <label>正则表达式</label>
              <input v-model="editingRule.pattern" type="text" class="form-input" placeholder="如: ^(EU|Europe)" />
            </div>
            <div v-if="editingRule.match_type === 'protocol'" class="form-row">
              <label>协议列表</label>
              <input
                :value="editingRule.protocols.join(', ')"
                @input="editingRule.protocols = ($event.target as HTMLInputElement).value.split(',').map(s => s.trim()).filter(Boolean)"
                type="text"
                class="form-input"
                placeholder="用逗号分隔，如: vmess, trojan"
              />
            </div>

            <!-- 内置区域快捷填充 -->
            <div v-if="isNewRule" class="builtin-regions">
              <span class="form-hint">快捷填充内置区域：</span>
              <div class="builtin-tags">
                <button
                  v-for="region in builtinRegions"
                  :key="region.name"
                  class="builtin-tag"
                  @click="applyBuiltinRegion(region)"
                >
                  {{ region.name }}
                </button>
              </div>
            </div>

            <div class="modal-actions">
              <button class="btn text" @click="editingRule = null">取消</button>
              <button class="btn primary" @click="saveRule">保存规则</button>
            </div>
          </div>

          <!-- 规则列表 -->
          <div v-else>
            <div v-if="proxyStore.customGroupRules.length === 0" class="empty-rules">
              暂无自定义区域规则，点击下方按钮添加
            </div>
            <div v-else class="rules-list">
              <div v-for="rule in proxyStore.customGroupRules" :key="rule.id" class="rule-item">
                <div class="rule-info">
                  <span class="rule-name">{{ rule.name }}</span>
                  <span class="rule-type">{{ rule.match_type }}</span>
                  <span class="rule-enabled" :class="{ disabled: !rule.enabled }">
                    {{ rule.enabled ? '启用' : '禁用' }}
                  </span>
                </div>
                <div class="rule-actions">
                  <button class="btn-icon" @click="editRule(rule)" title="编辑">
                    <SvgIcon name="edit" :size="12" />
                  </button>
                  <button class="btn-icon btn-delete-icon" @click="deleteRule(rule.id)" title="删除">
                    <SvgIcon name="trash" :size="12" />
                  </button>
                </div>
              </div>
            </div>
            <div class="modal-actions">
              <button class="btn text" @click="showRegionModal = false">关闭</button>
              <button class="btn primary" @click="addNewRule">
                <SvgIcon name="plus" :size="12" style="margin-right: 4px;" />
                新增区域
              </button>
            </div>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.proxies-view {
  padding: 20px;
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.proxies-layout {
  display: flex;
  gap: 16px;
  height: 100%;
  overflow: hidden;
}

.glass-effect {
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
}

/* ---- 左栏: 分组列表 ---- */
.sidebar-groups {
  width: 220px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  overflow-y: auto;
  flex-shrink: 0;
}

.section-title {
  display: flex;
  align-items: center;
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  color: var(--text-tertiary);
  text-transform: uppercase;
  margin-top: 8px;
}

.recent-list, .groups-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.recent-item, .group-item {
  display: flex;
  flex-direction: column;
  padding: 8px 12px;
  border-radius: var(--radius-md);
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  text-align: left;
  transition: all var(--duration-fast) var(--ease-out);
}

.recent-item:hover, .group-item:hover {
  background: var(--border-subtle);
  color: var(--text-primary);
}

.recent-item.active, .group-item.active {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
  font-weight: var(--weight-semibold);
}

.group-header-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

.group-name-wrapper {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex: 1;
}

.route-dot {
  width: 6px;
  height: 6px;
  background-color: var(--accent-green);
  border-radius: 50%;
  box-shadow: 0 0 8px var(--accent-green);
  flex-shrink: 0;
  animation: pulse-green 2s infinite;
}

@keyframes pulse-green {
  0% { box-shadow: 0 0 0 0 rgba(52, 211, 153, 0.7); }
  70% { box-shadow: 0 0 0 6px rgba(52, 211, 153, 0); }
  100% { box-shadow: 0 0 0 0 rgba(52, 211, 153, 0); }
}

.group-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-badge {
  font-size: var(--text-xs);
  padding: 1px 4px;
  background: var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.group-current-node {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-top: 2px;
  transition: color var(--duration-fast);
}

.group-current-node.highlight-now {
  color: var(--accent-cyan);
  font-weight: var(--weight-medium);
}

/* ---- 右栏: 节点内容 ---- */
.nodes-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px;
  overflow: hidden;
}

.nodes-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
  border-bottom: 1px solid var(--border-subtle);
  padding-bottom: 12px;
  gap: 12px;
}

.toolbar-left {
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-shrink: 0;
}

.toolbar-left h2 {
  font-size: var(--text-md);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.nodes-count {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.filter-count {
  color: var(--accent-cyan);
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

/* 搜索框 */
.search-box {
  display: flex;
  align-items: center;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  padding: 0 8px;
  height: 30px;
  gap: 4px;
  transition: border-color var(--duration-fast);
}

.search-box:focus-within {
  border-color: var(--accent-blue);
}

.search-icon {
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.search-input {
  background: transparent;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: var(--text-xs);
  width: 120px;
}

.search-input::placeholder {
  color: var(--text-tertiary);
}

.search-clear {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 16px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}

.search-clear:hover {
  color: var(--text-primary);
}

/* 排序按钮 */
.btn-sort {
  display: flex;
  align-items: center;
  padding: 6px 10px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
  gap: 4px;
}

.btn-sort:hover {
  background: var(--border-strong);
  border-color: var(--border-accent);
}

.sort-order {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  background: var(--border-subtle);
  border-radius: var(--radius-xs);
  font-size: 11px;
  cursor: pointer;
}

.sort-order:hover {
  background: var(--accent-blue-glow);
  color: var(--accent-blue);
}

.btn-action {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-action:hover {
  background: var(--border-strong);
  border-color: var(--border-accent);
}

.btn-action.primary {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
  font-weight: var(--weight-semibold);
}

.btn-action.primary:hover {
  background: var(--accent-blue);
  color: var(--text-on-accent);
}

.nodes-scroll {
  flex: 1;
  overflow-y: auto;
  margin-top: 12px;
  padding-right: 4px;
}

.nodes-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* ---- 批量测速进度条 ---- */
.batch-progress-card {
  padding: 12px 16px;
  margin-bottom: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-xs);
}

.progress-bar-bg {
  height: 6px;
  background: var(--layer-2);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: var(--accent-cyan);
  box-shadow: var(--shadow-glow-cyan);
  transition: width var(--duration-normal) var(--ease-out);
}

.btn-cancel {
  align-self: flex-end;
  background: transparent;
  border: none;
  color: var(--accent-red);
  font-size: var(--text-xs);
  cursor: pointer;
}

/* ---- 异常与空状态 ---- */
.state-tip-full {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
  gap: 12px;
}

.sub-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.btn-retry {
  display: flex;
  align-items: center;
  padding: 8px 16px;
  background: var(--accent-blue-glow);
  border: 1px solid var(--accent-blue);
  border-radius: var(--radius-sm);
  color: var(--accent-blue);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-retry:hover {
  background: var(--accent-blue);
  color: var(--text-on-accent);
}

.state-tip {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

/* ---- 测速确认 Modal ---- */
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 99999;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: var(--blur-panel);
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-card {
  width: 400px;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.estimate-box {
  background: var(--layer-2);
  padding: 12px;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.warning-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

.btn {
  padding: 6px 16px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-normal);
  background: transparent;
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn.primary {
  background: var(--accent-blue);
  color: var(--text-on-accent);
  border-color: var(--accent-blue);
}

.btn.primary:hover {
  opacity: 0.9;
}

.btn.text {
  border-color: transparent;
  color: var(--text-secondary);
}

.btn.text:hover {
  background: var(--border-subtle);
  color: var(--text-primary);
}

/* ---- 分组项包装器（带编辑按钮） ---- */
.group-item-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.group-item-wrapper .group-item {
  flex: 1;
}

.btn-group-edit {
  position: absolute;
  right: 4px;
  top: 50%;
  transform: translateY(-50%);
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--layer-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  opacity: 0;
  transition: all var(--duration-fast) var(--ease-out);
}

.group-item-wrapper:hover .btn-group-edit {
  opacity: 1;
}

.btn-group-edit:hover {
  color: var(--accent-blue);
  border-color: var(--accent-blue);
}

/* ---- 分区标题行（带操作按钮） ---- */
.section-title-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.btn-add-region {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-add-region:hover {
  color: var(--accent-blue);
  border-color: var(--accent-blue);
}

/* ---- 编辑表单 ---- */
.edit-form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.form-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.form-row label {
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.form-input {
  padding: 6px 10px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  outline: none;
}

.form-input:focus {
  border-color: var(--accent-blue);
}

.form-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

/* ---- 区域管理 Modal ---- */
.region-modal {
  width: 460px;
  max-height: 80vh;
  overflow-y: auto;
}

.empty-rules {
  text-align: center;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  padding: 24px 0;
}

.rules-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.rule-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.rule-info {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.rule-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rule-type {
  font-size: var(--text-xs);
  padding: 1px 6px;
  background: var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.rule-enabled {
  font-size: var(--text-xs);
  color: var(--accent-green);
}

.rule-enabled.disabled {
  color: var(--text-tertiary);
}

.rule-actions {
  display: flex;
  gap: 4px;
}

.btn-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-icon:hover {
  color: var(--text-primary);
  border-color: var(--border-normal);
}

.btn-delete-icon:hover {
  color: var(--accent-red);
  border-color: var(--accent-red);
}

.builtin-regions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.builtin-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.builtin-tag {
  padding: 4px 10px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.builtin-tag:hover {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
}
</style>