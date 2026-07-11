<template>
  <div class="panel-container">
    <h2>📦 订阅管理</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>自动整理地区分组</span>
          <span class="sub-label">导入订阅时自动识别 HK/JP/US 等地区并生成 urltest 延迟优选组</span>
        </div>
        <input type="checkbox" v-model="settingsStore.settings.auto_group_on_import" class="switch" @change="save" />
      </div>

      <!-- 快速导入模组 -->
      <div class="import-card glass-effect">
        <h3>🚀 导入新订阅</h3>
        <div class="input-form">
          <input v-model="subName" type="text" placeholder="订阅别名 (如: SKYLUMO加速器)" class="text-input" />
          <input v-model="subUrl" type="text" placeholder="订阅 URL (如: https://...)" class="text-input" />
          <button class="btn-import" :disabled="importing" @click="handleImport">
            {{ importing ? '正在导入解析中...' : '🚀 开始导入' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { importSubscription } from "@/api/ipc/subscription";
import { useToast } from "@/composables/useToast";

const settingsStore = useSettingsStore();
const toast = useToast();

const subName = ref("SKYLUMO加速器");
const subUrl = ref("https://skylumo.com/api/v1/client/subscribe?token=REDACTED");
const importing = ref(false);

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("订阅设置已保存");
}

async function handleImport() {
  if (!subName.value.trim() || !subUrl.value.trim()) {
    toast.warning("请输入订阅别名与链接");
    return;
  }
  importing.value = true;
  toast.info("正在网络拉取并解析订阅...", subName.value);

  const res = await importSubscription(
    subName.value.trim(),
    subUrl.value.trim(),
    settingsStore.settings.auto_group_on_import
  );

  importing.value = false;
  if (res.success) {
    toast.success("订阅导入成功！", `解析出 ${res.data?.node_count || 0} 个节点并拉起服务`);
    subName.value = "";
    subUrl.value = "";
  } else {
    toast.error("订阅导入失败", res.error);
  }
}
</script>

<style scoped>
.panel-container { display: flex; flex-direction: column; gap: 16px; }
h2 { font-size: 18px; font-weight: 700; }
.setting-group { display: flex; flex-direction: column; gap: 14px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-weight: 600; }
.sub-label { font-size: 11px; color: rgba(255,255,255,0.4); font-weight: normal; }
.switch { width: 18px; height: 18px; cursor: pointer; }
.import-card { padding: 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 14px; display: flex; flex-direction: column; gap: 12px; }
.import-card h3 { font-size: 15px; font-weight: 600; }
.input-form { display: flex; flex-direction: column; gap: 10px; }
.text-input { padding: 8px 12px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #fff; font-size: 13px; outline: none; }
.btn-import { padding: 10px; background: linear-gradient(135deg, #00f2fe, #4facfe); border: none; border-radius: 8px; color: #000; font-weight: 700; cursor: pointer; }
</style>
