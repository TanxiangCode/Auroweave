<template>
  <div class="panel-container">
    <h2><BaseIcon name="Command" :size="20" class="panel-header-icon" /> 全局快捷键</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>Spotlight 命令框热键 <span :class="registered ? 'live-tag' : 'fixed-tag'">{{ registered ? "已全局注册" : "应用内生效" }}</span></span>
          <span class="sub-label">{{ registered ? "系统级热键：任意应用前台按下即可呼出主窗口与命令框" : "全局注册不可用（可能被其他应用占用），仅应用内生效" }}</span>
        </div>
        <kbd class="hotkey-kbd">{{ displayHotkey }}</kbd>
      </div>
      <div class="info-card">
        <p class="sub-label">提示：{{ platformHint }}</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 全局快捷键面板
 * 作者: TanXiang
 *
 * 展示当前注册的系统级热键状态（真实注册态，非静态文案）。
 * 热键值来自 settings.command_palette_hotkey。
 */
import { computed, onMounted, ref } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useSettingsStore } from "@/stores/settings.store";

const settingsStore = useSettingsStore();
const registered = ref(false);
const isMac = ref(false);

const displayHotkey = computed(() => {
  const raw = settingsStore.settings.command_palette_hotkey || "CommandOrControl+Space";
  // 展示层美化：CommandOrControl → Mac 显示 ⌘，其余显示 Ctrl
  return raw
    .replace(/\+/g, " + ")
    .replace(/CommandOrControl|Command/g, isMac.value ? "⌘" : "Ctrl")
    .replace(/Option|Alt/g, isMac.value ? "⌥" : "Alt")
    .replace(/Shift/g, isMac.value ? "⇧" : "Shift");
});

const platformHint = computed(() =>
  isMac.value
    ? "热键冲突时（如系统的 Spotlight 输入法切换），可在设置中改为其他组合。"
    : "若热键被其他全局软件占用，注册会自动回退为应用内快捷键。"
);

onMounted(async () => {
  isMac.value = navigator.userAgent.toLowerCase().includes("mac");
  try {
    const { isRegistered } = await import("@tauri-apps/plugin-global-shortcut");
    registered.value = await isRegistered(
      settingsStore.settings.command_palette_hotkey || "CommandOrControl+Space"
    );
  } catch {
    registered.value = false;
  }
});
</script>

<style scoped>
/* 面板骨架（.panel-container / h2 / .panel-header-icon / .setting-group）统一走 panel.css 全局定义，
   此处只保留本面板独有的控件样式。 */

.hotkey-kbd {
  padding: var(--space-1) var(--space-3);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--accent-cyan-vivid);
}

/* "已全局注册"标注徽标（真实注册态） */
.live-tag {
  font-size: 10px;
  font-weight: 600;
  color: var(--status-success, #22c55e);
  background: rgba(34, 197, 94, 0.1);
  border: 1px solid rgba(34, 197, 94, 0.3);
  border-radius: var(--radius-xs);
  padding: 0 5px;
  margin-left: 6px;
  vertical-align: 1px;
}

/* 回退态标注徽标 */
.fixed-tag {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-tertiary);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  padding: 0 5px;
  margin-left: 6px;
  vertical-align: 1px;
}

.info-card {
  padding: 10px 16px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}
</style>
