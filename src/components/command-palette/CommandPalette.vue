<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="visible" class="command-palette-backdrop">
        <div class="command-palette-modal glass-effect">
          <!-- 搜索框 -->
          <div class="search-bar">
            <BaseIcon name="Search" :size="16" class="search-icon" />
            <input
              ref="inputRef"
              v-model="query"
              type="text"
              placeholder="搜索节点、分组或快捷命令 (按 Esc 退出)..."
              @keydown.down.prevent="moveHover(1)"
              @keydown.up.prevent="moveHover(-1)"
              @keydown.enter.prevent="selectActive"
              @keydown.esc="close"
            />
            <span class="hotkey-hint">ESC 退出</span>
            <button class="close-x" title="关闭 (Esc)" @click="close">×</button>
          </div>

          <!-- 搜索结果列表 -->
          <div class="result-list" ref="listRef">
            <div v-if="filteredCommands.length === 0" class="empty-state">
              无匹配操作或节点
            </div>
            <div
              v-for="(cmd, index) in filteredCommands"
              :key="cmd.id"
              class="command-item"
              :class="{ active: index === activeIndex }"
              @mousemove="activeIndex = index"
              @click="executeCommand(cmd)"
            >
              <div class="cmd-icon"><BaseIcon :name="cmd.icon" :size="16" /></div>
              <div class="cmd-info">
                <div class="cmd-title">{{ cmd.title }}</div>
                <div class="cmd-subtitle">{{ cmd.subtitle }}</div>
              </div>
              <div class="cmd-category">{{ cmd.category }}</div>
            </div>
          </div>

          <!-- 页脚快捷键提示 -->
          <div class="palette-footer">
            <span><kbd>↑</kbd> <kbd>↓</kbd> 导航</span>
            <span><kbd>↵</kbd> 执行</span>
            <span><kbd>Ctrl+Space</kbd> 快捷呼出</span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useToast } from "@/composables/useToast";
import { useRouter } from "vue-router";

export interface CommandItem {
  id: string;
  category: "代理节点" | "代理模式" | "系统操作";
  icon: string;
  title: string;
  subtitle: string;
  action: () => void;
}

const visible = ref(false);
const query = ref("");
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLDivElement | null>(null);

const proxyStore = useProxyStore();
const toast = useToast();
const router = useRouter();

const open = () => {
  visible.value = true;
  query.value = "";
  activeIndex.value = 0;
  nextTick(() => {
    inputRef.value?.focus();
  });
};

const close = () => {
  visible.value = false;
};

// 预制全量可搜索命令
const allCommands = computed<CommandItem[]>(() => {
  const list: CommandItem[] = [
    // 代理模式
    {
      id: "mode-rule",
      category: "代理模式",
      icon: "GitFork",
      title: "切换到 规则模式 (Rule)",
      subtitle: "智能分流，常用国内流量直连",
      action: async () => {
        const res = await proxyStore.changeProxyMode("rule");
        if (res.success) {
          toast.success("代理模式已切换", "规则分流模式 (Rule)");
        }
      },
    },
    {
      id: "mode-global",
      category: "代理模式",
      icon: "Globe",
      title: "切换到 全局模式 (Global)",
      subtitle: "未命中显式规则与私网直连的流量走代理",
      action: async () => {
        const res = await proxyStore.changeProxyMode("global");
        if (res.success) {
          toast.success("代理模式已切换", "全局模式 (Global)");
        }
      },
    },
    {
      id: "mode-direct",
      category: "代理模式",
      icon: "Zap",
      title: "切换到 直连模式 (Direct)",
      subtitle: "所有网络流量直连，不经过代理",
      action: async () => {
        const res = await proxyStore.changeProxyMode("direct");
        if (res.success) {
          toast.success("代理模式已切换", "直连模式 (Direct)");
        }
      },
    },

    // 页面跳转与系统功能
    {
      id: "nav-proxies",
      category: "系统操作",
      icon: "Radio",
      title: "打开 代理节点大厅",
      subtitle: "查看与选择具体节点分组",
      action: () => router.push("/proxies"),
    },
    {
      id: "nav-subscriptions",
      category: "系统操作",
      icon: "Rss",
      title: "打开 订阅中心",
      subtitle: "管理机场订阅、流量监控与节点更新",
      action: () => router.push("/subscriptions"),
    },
    {
      id: "nav-routing",
      category: "系统操作",
      icon: "GitFork",
      title: "打开 分流规则与拓扑",
      subtitle: "配置应用分流、域名规则与出站策略",
      action: () => router.push("/routing"),
    },
    {
      id: "nav-audit",
      category: "系统操作",
      icon: "ShieldCheck",
      title: "打开 安全审计控制台",
      subtitle: "实时抓包监控与 DNS 连接诊断",
      action: () => router.push("/audit"),
    },
    {
      id: "nav-settings",
      category: "系统操作",
      icon: "Settings",
      title: "打开 偏好设置",
      subtitle: "修改系统代理、开机启动与极客配置",
      action: () => router.push("/settings"),
    },
    {
      id: "nav-speedtest",
      category: "系统操作",
      icon: "Gauge",
      title: "打开 智能测速与质量监控",
      subtitle: "一键并发测试全量节点延迟",
      action: () => router.push("/speedtest"),
    },
  ];


  // 动态加入已有代理分组和节点
  proxyStore.groups.forEach((group) => {
    group.proxies.slice(0, 30).forEach((proxyName) => {
      list.push({
        id: `node-${group.tag}-${proxyName}`,
        category: "代理节点",
        icon: "MapPin",
        title: proxyName,
        subtitle: `属于分组: ${group.tag}`,
        action: async () => {
          await proxyStore.selectNode(group.tag, proxyName);
          toast.success("节点选择成功", `[${group.tag}] 切换至: ${proxyName}`);
        },
      });
    });
  });

  return list;
});

// 过滤搜索
const filteredCommands = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return allCommands.value;
  return allCommands.value.filter(
    (c) =>
      c.title.toLowerCase().includes(q) ||
      c.subtitle.toLowerCase().includes(q) ||
      c.category.toLowerCase().includes(q)
  );
});

watch(query, () => {
  activeIndex.value = 0;
});

const moveHover = (delta: number) => {
  const total = filteredCommands.value.length;
  if (total === 0) return;
  activeIndex.value = (activeIndex.value + delta + total) % total;
};

const selectActive = () => {
  const item = filteredCommands.value[activeIndex.value];
  if (item) {
    executeCommand(item);
  }
};

const executeCommand = (item: CommandItem) => {
  item.action();
  close();
};

// 监听快捷键 Ctrl+Space 或 Cmd+Shift+P 呼出；Esc 全局兜底关闭
// （输入框失焦时 @keydown.esc 收不到事件，须在 window 层拦截）
const handleGlobalKeyDown = (e: KeyboardEvent) => {
  if ((e.ctrlKey || e.metaKey) && e.code === "Space") {
    e.preventDefault();
    if (visible.value) close();
    else open();
  } else if (e.key === "Escape" && visible.value) {
    e.preventDefault();
    close();
  }
};

onMounted(() => {
  window.addEventListener("keydown", handleGlobalKeyDown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleGlobalKeyDown);
});

defineExpose({ open, close });
</script>

<style scoped>
.command-palette-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10000;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(12px);
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 10vh;
}

.command-palette-modal {
  width: 600px;
  max-width: 90vw;
  /* 浮层底色走令牌：浅色主题下自动变白底深字 */
  background: var(--surface-deep);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-xl, 18px);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.search-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-normal);
}

.search-icon {
  font-size: 18px;
}

.search-bar input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  font-size: 16px;
  color: var(--text-primary);
}

.hotkey-hint {
  font-size: 11px;
  padding: 4px 8px;
  background: var(--surface-hover);
  border-radius: 6px;
  color: var(--text-tertiary);
}

.close-x {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  padding: 2px 4px;
  transition: color 0.15s;
}

.close-x:hover {
  color: var(--text-primary);
}

.result-list {
  max-height: 380px;
  overflow-y: auto;
  padding: 8px;
}

.empty-state {
  padding: 32px;
  text-align: center;
  font-size: 14px;
  color: var(--text-tertiary);
}

.command-item {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 10px 14px;
  border-radius: var(--radius-md, 10px);
  cursor: pointer;
  transition: background 0.15s ease;
}

.command-item.active {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 25%, transparent);
}

.cmd-icon {
  font-size: 20px;
  width: 24px;
  text-align: center;
}

.cmd-info {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.cmd-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.cmd-subtitle {
  font-size: 12px;
  color: var(--text-tertiary);
  margin-top: 2px;
}

.cmd-category {
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 4px;
  background: var(--surface-raised);
  color: var(--text-secondary);
}

.palette-footer {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 10px 20px;
  background: var(--surface-inset);
  border-top: 1px solid var(--border-subtle);
  font-size: 11px;
  color: var(--text-tertiary);
}

kbd {
  background: var(--surface-hover);
  padding: 2px 5px;
  border-radius: 4px;
  font-family: monospace;
}

.fade-enter-from, .fade-leave-to {
  opacity: 0;
}
.fade-enter-active, .fade-leave-active {
  transition: opacity 0.2s ease;
}
</style>
