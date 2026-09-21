<script setup lang="ts">
/**
 * 测速与解锁设置面板（从「高级与内核」抽离 + 归并 DNS 面板重复项）
 *
 * 职责：延迟测试参数（URL/并发/超时）、内核自动优选周期、吞吐量测速源、
 *       订阅网络超时、AI 服务解锁检测判据
 * 注意：本面板字段与 DNS 面板无交集——DNS 面板此前重复的
 *       「出站 urltest 延迟测试地址 / Concurrency / Timeout / Interval」已归并至此
 */
import { ref, onMounted } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";
import { DEFAULT_LATENCY_TEST_INTERVAL_SEC, DEFAULT_LATENCY_TEST_TOLERANCE_MS } from "@/constants";

const settingsStore = useSettingsStore();
const toast = useToast();

const highlightField = ref<string>("");

onMounted(() => {
  // 拓扑跳转高亮支持（与其他面板一致）
  const target = new URLSearchParams(window.location.search).get("highlight");
  if (target) {
    highlightField.value = target;
    setTimeout(() => (highlightField.value = ""), 3000);
  }
});

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
            type="text"
            v-model="settingsStore.settings.latency_test_url"
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
            type="number"
            v-model.number="settingsStore.settings.latency_test_concurrency"
            class="num-input"
            min="1"
            max="100"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>延迟测试超时 (Timeout)</span>
            <span class="sub-label">单节点 TCP/TLS 握手与连通性超时阈值 (默认: 3000 ms)</span>
          </div>
          <input
            type="number"
            v-model.number="settingsStore.settings.latency_test_timeout_ms"
            class="num-input"
            step="500"
            min="1000"
            max="10000"
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
          <input
            type="text"
            v-model="settingsStore.settings.speed_test_url"
            class="text-input"
            placeholder="https://speed.cloudflare.com/__down?bytes=25000000"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>单节点下载测速限时 (秒)</span>
            <span class="sub-label">吞吐量下载速度测试的最大持续时间 (默认: 5 秒)</span>
          </div>
          <input
            type="number"
            v-model.number="settingsStore.settings.speed_test_timeout_secs"
            class="num-input"
            min="3"
            max="30"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>订阅网络请求超时 (秒)</span>
            <span class="sub-label">远程拉取订阅节点与规则集的连接超时时长 (默认: 15 秒)</span>
          </div>
          <input
            type="number"
            v-model.number="settingsStore.settings.connection_timeout_secs"
            class="num-input"
            min="5"
            max="60"
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
            type="text"
            v-model="settingsStore.settings.unlock_gemini_marker"
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
            type="text"
            v-model="settingsStore.settings.unlock_claude_block_marker"
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
            type="text"
            v-model="settingsStore.settings.unlock_chatgpt_block_marker"
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
.panel-container {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

h2 {
  font-size: 18px;
  font-weight: 700;
  color: var(--text-primary);
}

.panel-header-icon {
  margin-right: 6px;
  vertical-align: -3px;
}

.setting-card {
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 12px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  padding-bottom: 10px;
}

.card-icon {
  font-size: 20px;
}

.card-title-group {
  flex: 1;
}

.card-title-group h3 {
  font-size: 13.5px;
  font-weight: 700;
  color: #fff;
  margin: 0;
}

.card-title-group p {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  margin: 2px 0 0;
}

.card-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.02);
  transition: all 0.15s;
}

.setting-item:hover {
  background: rgba(255, 255, 255, 0.04);
}

.item-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 12.5px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
}

.sub-label {
  font-size: 10.5px;
  color: rgba(255, 255, 255, 0.4);
  font-weight: normal;
}

.text-input {
  padding: 6px 10px;
  background: #141824;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #fff;
  font-size: 12px;
  width: 260px;
  outline: none;
  font-family: monospace;
}

.text-input.readonly {
  opacity: 0.65;
  cursor: not-allowed;
  border-style: dashed;
  filter: grayscale(0.3);
}

.text-input:focus,
.num-input:focus {
  border-color: var(--accent-cyan-vivid);
}

.num-input {
  padding: 6px 10px;
  background: #141824;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #fff;
  font-size: 12px;
  width: 90px;
  outline: none;
}

.readonly-tag {
  font-size: 10px;
  padding: 1px 6px;
  border: 1px dashed rgba(255, 255, 255, 0.2);
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.35);
  margin-left: 4px;
}

.marker-hint {
  font-size: 11px;
  line-height: 1.6;
  color: rgba(255, 255, 255, 0.4);
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.02);
  border-left: 2px solid color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
  border-radius: 0 8px 8px 0;
}
</style>
