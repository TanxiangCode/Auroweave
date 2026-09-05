<template>
  <div class="panel-container">
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon">
          <BaseIcon name="Layers" :size="20" />
        </span>
        <div class="card-title-group">
          <h3>首页统计胶囊</h3>
          <p>控制主页右侧显示哪些实时信息卡片，关闭后立即生效无需重启</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>活动连接数</span>
            <span class="sub-label">当前活跃的代理连接数量，点击可跳转连接审计</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.dashboard_show_connections"
            class="switch"
            @change="save('dashboard_show_connections')"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>当前节点与延迟</span>
            <span class="sub-label">正在使用的出口节点名及最近一次测得的延迟（毫秒）</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.dashboard_show_current_node"
            class="switch"
            @change="save('dashboard_show_current_node')"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>出口 IP 与归属地</span>
            <span class="sub-label">当前流量真实出口的公网 IP、归属地与国家/地区旗帜；探测源 ip-api.com</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.dashboard_show_egress_ip"
            class="switch"
            @change="save('dashboard_show_egress_ip')"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>累计流量</span>
            <span class="sub-label">本次会话累计的下行/上行流量，点击可跳转流量统计</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.dashboard_show_total_traffic"
            class="switch"
            @change="save('dashboard_show_total_traffic')"
          />
        </div>
      </div>
    </div>

    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon">
          <BaseIcon name="Activity" :size="20" />
        </span>
        <div class="card-title-group">
          <h3>实时流量图</h3>
          <p>主页底部的双向实时速率折线图（跟随代理开关显示）</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item static-item">
          <div class="item-label">
            <span>实时双向流量监控图</span>
            <span class="sub-label">始终随代理状态显示，暂不提供单独开关——关闭代理时自动隐藏</span>
          </div>
          <span class="fixed-tag">跟随代理开关</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 首页显示设置面板
 * 作者: TanXiang
 *
 * 控制主页右翼统计胶囊的显隐开关（dashboard_show_* 字段），
 * 修改即时生效（DashboardView 经 settings store 响应式联动）。
 */
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useSettingsStore } from "@/stores/settings.store";

const settingsStore = useSettingsStore();

/** 保存单个开关字段（patch 式提交，仅传修改字段） */
async function save(field: string) {
  await settingsStore.updateSettings({
    [field]: (settingsStore.settings as Record<string, unknown>)[field],
  });
}
</script>

<style scoped>
/* switch 统一走 App.vue 全局胶囊开关；卡片三线视觉走全局 setting-card 体系 */
.static-item {
  opacity: 0.85;
}

.fixed-tag {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  padding: 2px 10px;
  white-space: nowrap;
}
</style>
