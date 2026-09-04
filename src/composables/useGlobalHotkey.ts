/**
 * 全局快捷键注册 composable
 * 作者: TanXiang
 *
 * 把 settings.command_palette_hotkey 注册为系统级全局热键（tauri-plugin-global-shortcut）：
 * 任何应用前台时按下都能呼出主窗口 + Spotlight 命令框。
 * - 首次挂载时注册；hotkey 设置变化时先注销旧键再注册新键
 * - 浏览器预览模式下静默跳过
 */
import { watch } from "vue";
import { useSettingsStore } from "@/stores/settings.store";

/** 当前已注册的热键（注册成功后记录，用于变更时注销） */
let registeredHotkey: string | null = null;

/** 热键按下回调（App.vue 注入：显示窗口 + 呼出命令框） */
let hotkeyHandler: (() => void) | null = null;

/**
 * 注册 / 换绑全局热键
 * @param hotkey Tauri 快捷键格式（如 "CommandOrControl+Space"）
 */
async function rebindGlobalHotkey(hotkey: string) {
  try {
    const { register, unregister, isRegistered } = await import(
      "@tauri-apps/plugin-global-shortcut"
    );

    // 注销旧绑定（如存在）
    if (registeredHotkey && registeredHotkey !== hotkey) {
      try {
        await unregister(registeredHotkey);
      } catch {
        // 旧键可能已被外部注销，忽略
      }
      registeredHotkey = null;
    }

    // 同键已注册则跳过（幂等）
    if (await isRegistered(hotkey)) {
      registeredHotkey = hotkey;
      return;
    }

    await register(hotkey, (event) => {
      if (event.state === "Pressed" && hotkeyHandler) {
        hotkeyHandler();
      }
    });
    registeredHotkey = hotkey;
  } catch (e) {
    console.warn("[global-hotkey] 注册失败（浏览器预览或权限缺失）:", e);
  }
}

export function useGlobalHotkey(onTriggered: () => void) {
  const settingsStore = useSettingsStore();
  hotkeyHandler = onTriggered;

  // 监听设置中的热键字段：设置加载完成 / 用户修改后即时换绑
  watch(
    () => settingsStore.settings.command_palette_hotkey,
    (hotkey) => {
      if (hotkey) {
        rebindGlobalHotkey(hotkey);
      }
    },
    { immediate: true }
  );
}
