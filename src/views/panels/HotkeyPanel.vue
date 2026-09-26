<template>
  <div class="panel-container">
    <h2><BaseIcon name="Command" :size="20" class="panel-header-icon" /> 全局快捷键</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>当前生效热键</span>
          <span class="sub-label">{{ registered ? "系统级热键：任意应用前台按下即可呼出主窗口与命令框" : "全局注册不可用（可能被其他应用占用），仅应用内生效" }}</span>
        </div>
        <kbd class="hotkey-kbd">{{ displayHotkey }}</kbd>
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>修改热键组合 <span :class="registered ? 'live-tag' : 'fixed-tag'">{{ registered ? "已全局注册" : "应用内生效" }}</span></span>
          <span class="sub-label">
            使用 Tauri 加速键格式（+ 连接，如 CommandOrControl+Space）。应用后立即注销旧键并重新注册；
            组合被系统占用时会回退为仅应用内生效，不影响主流程。
          </span>
        </div>
        <div class="hotkey-editor">
          <input
            v-model="hotkeyInput"
            type="text"
            class="text-input hotkey-input"
            spellcheck="false"
            placeholder="CommandOrControl+Space"
            @keyup.enter="applyHotkey"
          />
          <button class="btn-apply" :disabled="!canApply" @click="applyHotkey">应用</button>
        </div>
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>常用组合</span>
          <span class="sub-label">点击填入上方的输入框，再点「应用」生效</span>
        </div>
        <div class="hotkey-presets">
          <button
            v-for="preset in HOTKEY_PRESETS"
            :key="preset"
            type="button"
            class="btn-preset"
            @click="hotkeyInput = preset"
          >
            {{ formatAccel(preset) }}
          </button>
        </div>
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
import { useToast } from "@/composables/useToast";

const toast = useToast();

const settingsStore = useSettingsStore();
const registered = ref(false);
const isMac = ref(false);

/** 常用组合（Tauri 加速键格式） */
const HOTKEY_PRESETS = [
  "CommandOrControl+Space",
  "CommandOrControl+Shift+Space",
  "Alt+Space",
  "CommandOrControl+K",
  "Alt+Shift+S",
] as const;

/** 输入框草稿值：与已保存值不同时才允许「应用」 */
const hotkeyInput = ref("");

/** 归一化：去空白 + 统一分隔符写法，降低手输格式错误的概率 */
function normalizeAccel(raw: string): string {
  return raw
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean)
    .join("+");
}

const normalizedDraft = computed(() => normalizeAccel(hotkeyInput.value));

/** 当前已保存的归一化热键 */
const savedAccel = computed(() =>
  normalizeAccel(settingsStore.settings.command_palette_hotkey || "CommandOrControl+Space")
);

const canApply = computed(
  () => normalizedDraft.value.length > 0 && normalizedDraft.value !== savedAccel.value
);

/** 展示层美化：CommandOrControl → Mac 显示 ⌘，其余显示 Ctrl */
function formatAccel(raw: string): string {
  const isMacPlatform = isMac.value;
  return normalizeAccel(raw)
    .replace(/\+/g, " + ")
    .replace(/CommandOrControl|Command/g, isMacPlatform ? "⌘" : "Ctrl")
    .replace(/Option|Alt/g, isMacPlatform ? "⌥" : "Alt")
    .replace(/Shift/g, isMacPlatform ? "⇧" : "Shift");
}

const displayHotkey = computed(() => formatAccel(savedAccel.value));

/**
 * 应用新热键
 *
 * 全局注册由 App.vue 的 useGlobalHotkey 监听 settings.command_palette_hotkey
 * 自动完成（先注销旧键再注册新键），此处只负责持久化 + 刷新注册态展示。
 */
async function applyHotkey() {
  if (!canApply.value) return;
  const accel = normalizedDraft.value;
  const res = await settingsStore.updateSettings({ command_palette_hotkey: accel });
  if (!res.success) return;
  hotkeyInput.value = accel;
  toast.success("热键已更新", formatAccel(accel));
  // 换绑在 watch 中异步进行，稍作延迟再读注册态
  setTimeout(refreshRegistered, 300);
}

async function refreshRegistered() {
  try {
    const { isRegistered } = await import("@tauri-apps/plugin-global-shortcut");
    registered.value = await isRegistered(
      settingsStore.settings.command_palette_hotkey || "CommandOrControl+Space"
    );
  } catch {
    registered.value = false;
  }
}

const platformHint = computed(() =>
  isMac.value
    ? "热键冲突时（如系统的 Spotlight 输入法切换），可在设置中改为其他组合。"
    : "若热键被其他全局软件占用，注册会自动回退为应用内快捷键。"
);

onMounted(async () => {
  isMac.value = navigator.userAgent.toLowerCase().includes("mac");
  hotkeyInput.value = savedAccel.value;
  await refreshRegistered();
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

/* 本面板：热键编辑区（输入框 + 应用按钮 / 常用组合） */
.hotkey-editor {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.hotkey-input {
  width: 240px;
}

.btn-apply {
  padding: 6px 14px;
  background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  border-radius: var(--radius-sm);
  color: var(--accent-cyan-vivid);
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  cursor: pointer;
  white-space: nowrap;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-apply:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 22%, transparent);
  color: var(--text-primary);
}

.btn-apply:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.hotkey-presets {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  justify-content: flex-end;
}

.btn-preset {
  padding: 3px 10px;
  background: var(--surface-hover);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-xs);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  cursor: pointer;
  white-space: nowrap;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-preset:hover {
  color: var(--accent-cyan-vivid);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
}

.info-card {
  padding: 10px 16px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}
</style>
