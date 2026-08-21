<template>
  <div class="topology-canvas-container" ref="canvasContainerRef">
    <!-- 顶部悬浮控制栏 -->
    <div class="canvas-floating-toolbar glass-effect">
      <div class="toolbar-left">
        <span class="pulse-dot"></span>
        <span class="canvas-title">全景分流拓扑网络 (Routing Flow Canvas)</span>
        <span class="active-badge" v-if="selectedNode">链路高亮中: {{ selectedNode }}</span>
      </div>

      <div class="toolbar-actions">
        <button class="btn-tool" @click="zoomIn" title="放大画布"></button>
        <button class="btn-tool" @click="zoomOut" title="缩小画布">-</button>
        <button class="btn-tool" @click="resetView" title="重置居中视角">居中</button>
        <button
          class="btn-tool"
          :class="{ active: enableParticles }"
          @click="enableParticles = !enableParticles"
          title="开启/关闭极光粒子流光动效"
        >
           流光
        </button>
      </div>
    </div>

    <!-- 交互式可平移可缩放视口 -->
    <div
      class="viewport"
      :class="{ dragging: isDragging }"
      @mousedown="startPan"
      @mousemove="onPan"
      @mouseup="endPan"
      @mouseleave="endPan"
      @wheel.prevent="onWheel"
    >
      <div
        class="canvas-world"
        :style="{
          transform: `translate(${panX}px, ${panY}px) scale(${scale})`,
          transformOrigin: '0 0',
        }"
      >
        <!-- SVG 流光连线层 -->
        <svg class="connections-svg">
          <defs>
            <linearGradient id="cyanGrad" x1="0%" y1="0%" x2="100%" y2="0%">
              <stop offset="0%" stop-color="#00f2fe" stop-opacity="0.8" />
              <stop offset="100%" stop-color="#4facfe" stop-opacity="0.9" />
            </linearGradient>
            <linearGradient id="greenGrad" x1="0%" y1="0%" x2="100%" y2="0%">
              <stop offset="0%" stop-color="#10b981" stop-opacity="0.8" />
              <stop offset="100%" stop-color="#059669" stop-opacity="0.9" />
            </linearGradient>
            <linearGradient id="redGrad" x1="0%" y1="0%" x2="100%" y2="0%">
              <stop offset="0%" stop-color="#f87171" stop-opacity="0.8" />
              <stop offset="100%" stop-color="#dc2626" stop-opacity="0.9" />
            </linearGradient>
            <linearGradient id="purpleGrad" x1="0%" y1="0%" x2="100%" y2="0%">
              <stop offset="0%" stop-color="#a78bfa" stop-opacity="0.8" />
              <stop offset="100%" stop-color="#8b5cf6" stop-opacity="0.9" />
            </linearGradient>
          </defs>

          <!-- 动态贝塞尔曲线 -->
          <path
            v-for="(link, idx) in computedLinks"
            :key="idx"
            :d="link.d"
            :stroke="link.color"
            :class="[
              'topo-link',
              { active: isLinkActive(link), dim: isLinkDim(link), particles: enableParticles }
            ]"
          />
        </svg>

        <!-- 4 层拓扑节点列 -->
        <div class="columns-layout">
          <!-- 第 1 列：入站流量 (Inbounds) -->
          <div class="column-group">
            <div class="col-header">
              <span class="col-icon"><BaseIcon name="Download" :size="16" /></span>
              <span class="col-name">流量入站层</span>
            </div>
            <div class="nodes-list">
              <div
                class="topo-node-card inbounds"
                :class="{ selected: selectedNode === 'inbound_mixed' }"
                @click="toggleSelectNode('inbound_mixed')"
              >
                <div class="node-icon"><BaseIcon name="Layers" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">Mixed 混合代理</span>
                  <span class="node-sub">127.0.0.1:8890</span>
                </div>
                <span class="status-indicator online"></span>
              </div>

              <div
                class="topo-node-card inbounds"
                :class="{ selected: selectedNode === 'inbound_tun' }"
                @click="toggleSelectNode('inbound_tun')"
              >
                <div class="node-icon"><BaseIcon name="Cpu" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">TUN 虚拟网卡</span>
                  <span class="node-sub">全局透明接管</span>
                </div>
                <span class="status-indicator online"></span>
              </div>
            </div>
          </div>

          <!-- 第 2 列：规则引擎 (Rules Engine) -->
          <div class="column-group">
            <div class="col-header">
              <span class="col-icon"><BaseIcon name="GitFork" :size="16" /></span>
              <span class="col-name">智能分流引擎</span>
            </div>
            <div class="nodes-list">
              <div
                class="topo-node-card rules"
                :class="{ selected: selectedNode === 'rule_app' }"
                @click="toggleSelectNode('rule_app')"
              >
                <div class="node-icon"><BaseIcon name="LayoutGrid" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">App-Matrix 进程规则</span>
                  <span class="node-sub">{{ appRulesCount }} 个进程已绑定</span>
                </div>
              </div>

              <div
                class="topo-node-card rules"
                :class="{ selected: selectedNode === 'rule_custom' }"
                @click="toggleSelectNode('rule_custom')"
              >
                <div class="node-icon"><BaseIcon name="Globe" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">自定义域名 / IP 规则</span>
                  <span class="node-sub">{{ customRulesCount }} 条规则生效</span>
                </div>
              </div>

              <div
                class="topo-node-card rules"
                :class="{ selected: selectedNode === 'rule_geosite' }"
                @click="toggleSelectNode('rule_geosite')"
              >
                <div class="node-icon"><BaseIcon name="MapPin" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">GeoSite 大陆白名单</span>
                  <span class="node-sub">geosite-cn + geoip-cn</span>
                </div>
              </div>

              <div
                class="topo-node-card rules"
                :class="{ selected: selectedNode === 'rule_final' }"
                @click="toggleSelectNode('rule_final')"
              >
                <div class="node-icon"><BaseIcon name="Compass" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">Final 默认兜底策略</span>
                  <span class="node-sub">全量境外流量代理</span>
                </div>
              </div>
            </div>
          </div>

          <!-- 第 3 列：出站节点与策略组 (Outbounds) -->
          <div class="column-group">
            <div class="col-header">
              <span class="col-icon"><BaseIcon name="Send" :size="16" /></span>
              <span class="col-name">出站节点策略</span>
            </div>
            <div class="nodes-list">
              <div
                class="topo-node-card outbounds proxy"
                :class="{ selected: selectedNode === 'outbound_proxy' }"
                @click="toggleSelectNode('outbound_proxy')"
              >
                <div class="node-icon"><BaseIcon name="Radio" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">PROXY 代理节点池</span>
                  <span class="node-sub text-cyan">{{ currentProxyNode }}</span>
                </div>
              </div>

              <div
                class="topo-node-card outbounds direct"
                :class="{ selected: selectedNode === 'outbound_direct' }"
                @click="toggleSelectNode('outbound_direct')"
              >
                <div class="node-icon"><BaseIcon name="Zap" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">DIRECT 大陆直连</span>
                  <span class="node-sub text-green">零延迟直通</span>
                </div>
              </div>

              <div
                class="topo-node-card outbounds block"
                :class="{ selected: selectedNode === 'outbound_block' }"
                @click="toggleSelectNode('outbound_block')"
              >
                <div class="node-icon"><BaseIcon name="Ban" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">BLOCK 安全阻断</span>
                  <span class="node-sub text-red">隐私探针拦截</span>
                </div>
              </div>
            </div>
          </div>

          <!-- 第 4 列：目标网络生态 (Target Ecosystem) -->
          <div class="column-group">
            <div class="col-header">
              <span class="col-icon"><BaseIcon name="Server" :size="16" /></span>
              <span class="col-name">目标网络生态</span>
            </div>
            <div class="nodes-list">
              <div
                class="topo-node-card targets global"
                :class="{ selected: selectedNode === 'target_global' }"
                @click="toggleSelectNode('target_global')"
              >
                <div class="node-icon"><BaseIcon name="Globe2" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">海外互联网 & AI 服务</span>
                  <span class="node-sub">OpenAI, GitHub, YouTube</span>
                </div>
              </div>

              <div
                class="topo-node-card targets domestic"
                :class="{ selected: selectedNode === 'target_cn' }"
                @click="toggleSelectNode('target_cn')"
              >
                <div class="node-icon"><BaseIcon name="MapPin" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">大陆国内网络生态</span>
                  <span class="node-sub">国内电商、微信、B站</span>
                </div>
              </div>

              <div
                class="topo-node-card targets blocked"
                :class="{ selected: selectedNode === 'target_block' }"
                @click="toggleSelectNode('target_block')"
              >
                <div class="node-icon"><BaseIcon name="ShieldAlert" :size="18" /></div>
                <div class="node-info">
                  <span class="node-title">恶意拦截与遥测威胁</span>
                  <span class="node-sub">广告追踪探针</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, computed, onMounted } from "vue";
import { getAppRules, getCustomRules } from "@/api/ipc/routing";
import { getProxyGroups } from "@/api/ipc/proxy";

const canvasContainerRef = ref<HTMLDivElement | null>(null);

const appRulesCount = ref(0);
const customRulesCount = ref(0);
const currentProxyNodeName = ref("自动优选 (Auto)");

const scale = ref(0.95);
const panX = ref(30);
const panY = ref(20);
const isDragging = ref(false);
const startX = ref(0);
const startY = ref(0);

const enableParticles = ref(true);
const selectedNode = ref<string | null>(null);

const currentProxyNode = computed(() => {
  return currentProxyNodeName.value || "自动优选 (Auto)";
});


interface TopoLink {
  from: string;
  to: string;
  d: string;
  color: string;
}

// 静态拓扑连线拓扑关系表
const computedLinks = computed<TopoLink[]>(() => {
  // 列坐标固定参考（基于 4 列布局）
  const c1X = 220; // Inbounds 右端点
  const c2InX = 290; // Rules 左端点
  const c2OutX = 510; // Rules 右端点
  const c3InX = 580; // Outbounds 左端点
  const c3OutX = 800; // Outbounds 右端点
  const c4InX = 870; // Targets 左端点

  const links: TopoLink[] = [
    // Inbound -> Rules
    { from: "inbound_mixed", to: "rule_app", d: createBezier(c1X, 90, c2InX, 90), color: "url(#cyanGrad)" },
    { from: "inbound_mixed", to: "rule_custom", d: createBezier(c1X, 90, c2InX, 170), color: "url(#cyanGrad)" },
    { from: "inbound_mixed", to: "rule_geosite", d: createBezier(c1X, 90, c2InX, 250), color: "url(#greenGrad)" },
    { from: "inbound_mixed", to: "rule_final", d: createBezier(c1X, 90, c2InX, 330), color: "url(#cyanGrad)" },
    { from: "inbound_tun", to: "rule_app", d: createBezier(c1X, 170, c2InX, 90), color: "url(#purpleGrad)" },
    { from: "inbound_tun", to: "rule_custom", d: createBezier(c1X, 170, c2InX, 170), color: "url(#purpleGrad)" },
    { from: "inbound_tun", to: "rule_geosite", d: createBezier(c1X, 170, c2InX, 250), color: "url(#greenGrad)" },
    { from: "inbound_tun", to: "rule_final", d: createBezier(c1X, 170, c2InX, 330), color: "url(#purpleGrad)" },

    // Rules -> Outbounds
    { from: "rule_app", to: "outbound_proxy", d: createBezier(c2OutX, 90, c3InX, 90), color: "url(#cyanGrad)" },
    { from: "rule_custom", to: "outbound_proxy", d: createBezier(c2OutX, 170, c3InX, 90), color: "url(#cyanGrad)" },
    { from: "rule_custom", to: "outbound_direct", d: createBezier(c2OutX, 170, c3InX, 170), color: "url(#greenGrad)" },
    { from: "rule_geosite", to: "outbound_direct", d: createBezier(c2OutX, 250, c3InX, 170), color: "url(#greenGrad)" },
    { from: "rule_final", to: "outbound_proxy", d: createBezier(c2OutX, 330, c3InX, 90), color: "url(#cyanGrad)" },

    // Outbounds -> Targets
    { from: "outbound_proxy", to: "target_global", d: createBezier(c3OutX, 90, c4InX, 90), color: "url(#cyanGrad)" },
    { from: "outbound_direct", to: "target_cn", d: createBezier(c3OutX, 170, c4InX, 170), color: "url(#greenGrad)" },
    { from: "outbound_block", to: "target_block", d: createBezier(c3OutX, 250, c4InX, 250), color: "url(#redGrad)" },
  ];

  return links;
});

function createBezier(x1: number, y1: number, x2: number, y2: number): string {
  const mx = (x1 + x2) / 2;
  return `M ${x1} ${y1} C ${mx} ${y1}, ${mx} ${y2}, ${x2} ${y2}`;
}

function isLinkActive(link: TopoLink): boolean {
  if (!selectedNode.value) return true;
  return link.from === selectedNode.value || link.to === selectedNode.value;
}

function isLinkDim(link: TopoLink): boolean {
  if (!selectedNode.value) return false;
  return link.from !== selectedNode.value && link.to !== selectedNode.value;
}

function toggleSelectNode(nodeId: string) {
  if (selectedNode.value === nodeId) {
    selectedNode.value = null;
  } else {
    selectedNode.value = nodeId;
  }
}

function zoomIn() {
  scale.value = Math.min(1.8, scale.value + 0.15);
}

function zoomOut() {
  scale.value = Math.max(0.5, scale.value - 0.15);
}

function resetView() {
  scale.value = 0.95;
  panX.value = 30;
  panY.value = 20;
  selectedNode.value = null;
}

function onWheel(e: WheelEvent) {
  if (e.deltaY < 0) {
    zoomIn();
  } else {
    zoomOut();
  }
}

function startPan(e: MouseEvent) {
  isDragging.value = true;
  startX.value = e.clientX - panX.value;
  startY.value = e.clientY - panY.value;
}

function onPan(e: MouseEvent) {
  if (!isDragging.value) return;
  panX.value = e.clientX - startX.value;
  panY.value = e.clientY - startY.value;
}

function endPan() {
  isDragging.value = false;
}

onMounted(async () => {
  const [appRes, customRes, groupsRes] = await Promise.all([
    getAppRules(),
    getCustomRules(),
    getProxyGroups(),
  ]);
  if (appRes.success && appRes.data) {
    appRulesCount.value = Object.keys(appRes.data).length;
  }
  if (customRes.success && customRes.data) {
    customRulesCount.value = customRes.data.filter((r) => r.enabled).length;
  }
  if (groupsRes.success && groupsRes.data) {
    const proxyGroup = groupsRes.data.find((g) => g.tag === "proxy" || g.tag === "GLOBAL");
    if (proxyGroup && proxyGroup.proxies && proxyGroup.proxies.length > 0) {
      currentProxyNodeName.value = proxyGroup.proxies[0] || "自动优选 (Auto)";
    }
  }
});

</script>

<style scoped>
.topology-canvas-container {
  position: relative;
  width: 100%;
  height: 100%;
  background: radial-gradient(circle at 50% 50%, rgba(18, 24, 38, 0.9) 0%, rgba(10, 12, 18, 0.98) 100%);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 14px;
  overflow: hidden;
  user-select: none;
}

.canvas-floating-toolbar {
  position: absolute;
  top: 14px;
  left: 14px;
  right: 14px;
  padding: 8px 14px;
  background: rgba(18, 20, 28, 0.85);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  z-index: 10;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.pulse-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #00f2fe;
  box-shadow: 0 0 8px #00f2fe;
}

.canvas-title {
  font-size: 12px;
  font-weight: 600;
  color: #fff;
}

.active-badge {
  font-size: 10.5px;
  color: #00f2fe;
  background: rgba(0, 242, 254, 0.12);
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid rgba(0, 242, 254, 0.3);
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-tool {
  padding: 4px 8px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-tool:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
}

.btn-tool.active {
  background: rgba(0, 242, 254, 0.18);
  border-color: #00f2fe;
  color: #00f2fe;
}

.viewport {
  width: 100%;
  height: 100%;
  cursor: grab;
  overflow: hidden;
}

.viewport.dragging {
  cursor: grabbing;
}

.canvas-world {
  position: absolute;
  width: 1100px;
  height: 480px;
}

.connections-svg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 1;
}

.topo-link {
  fill: none;
  stroke-width: 2.2px;
  transition: opacity 0.3s ease, stroke-width 0.2s;
}

.topo-link.particles {
  stroke-dasharray: 6 6;
  animation: flowParticle 1.4s linear infinite;
}

@keyframes flowParticle {
  from { stroke-dashoffset: 24; }
  to { stroke-dashoffset: 0; }
}

.topo-link.dim {
  opacity: 0.12;
  stroke-width: 1.2px;
}

.topo-link.active {
  opacity: 1;
  stroke-width: 3px;
  filter: drop-shadow(0 0 5px currentColor);
}

.columns-layout {
  display: flex;
  gap: 70px;
  padding: 65px 20px 20px;
  position: relative;
  z-index: 2;
}

.column-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: 220px;
}

.col-header {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.6);
  padding: 0 4px;
}

.nodes-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.topo-node-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  background: rgba(255, 255, 255, 0.035);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  box-shadow: 0 4px 15px rgba(0, 0, 0, 0.3);
  position: relative;
}

.topo-node-card:hover {
  background: rgba(255, 255, 255, 0.07);
  border-color: rgba(255, 255, 255, 0.2);
  transform: translateY(-2px);
}

.topo-node-card.selected {
  border-color: #00f2fe;
  background: rgba(0, 242, 254, 0.1);
  box-shadow: 0 0 20px rgba(0, 242, 254, 0.3);
}

.node-icon {
  font-size: 20px;
  flex-shrink: 0;
}

.node-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 0;
}

.node-title {
  font-size: 12px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.95);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-sub {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-indicator {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.status-indicator.online {
  background: #10b981;
  box-shadow: 0 0 6px #10b981;
}

.text-cyan { color: #00f2fe !important; }
.text-green { color: #10b981 !important; }
.text-red { color: #f87171 !important; }
</style>
