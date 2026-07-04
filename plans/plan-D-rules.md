# 模块 D — 规则合并与内核管理

> 状态：⏳ 待开始 | 优先级：🟠 高 | 前置：模块 B

---

## 目标

实现本地离线的多订阅规则合并，并确保 sing-box 在系统睡眠唤醒后能自动重连，同时管理内核版本更新。

**验收标准**：
- 多订阅规则合并后生成正确的 `config.json`
- 系统休眠唤醒后代理自动重连，无需用户手动操作

---

## 任务清单

### D-1 规则合并数据流

**文件**：`src-tauri/src/core/config_builder.rs`（扩展）

**数据流**：
```
用户订阅 A（节点列表）
用户订阅 B（节点列表）
        ↓ 合并去重
    统一节点池
        ↓ 按地区自动分组
  urltest 出站组（香港/日本/美国等）
        ↓
规则集（GeoIP / GeoSite / 用户自定义）
        ↓ sing-box CLI 编译
  .srs 二进制规则文件
        ↓
    config.json
        ↓ ClashAPI PATCH /configs reload=true
    sing-box 热重载
```

**任务细节**：
- [ ] 多订阅节点合并（相同 `server:port` 去重）
- [ ] 地区标记解析（支持：香港/HK/🇭🇰/Hongkong/hong_kong 等多种写法）
- [ ] 自动生成 `urltest` 出站（每个地区一组）
- [ ] 调用 `sing-box rule-set compile` 生成 `.srs` 文件
- [ ] `config.json` 完整结构（outbounds / route / dns / inbounds）
- [ ] 热重载：`PATCH http://127.0.0.1:9090/configs?force=true`

**隐私保证**：全过程无网络请求，仅本地文件 I/O。

---

### D-2 睡眠/唤醒自动重连

**文件**：`src-tauri/src/core/sidecar.rs`（扩展）

- [ ] 监听系统 `suspend` / `resume` 事件
  - Windows：通过 `WM_WTSSESSION_CHANGE` 或 PowerBroadcast 消息
  - macOS：通过 IOKit `kIOMessageSystemWillSleep` / `kIOMessageSystemHasPoweredOn`
- [ ] 唤醒后延迟 1 秒（等待网络适配器就绪），再重启 sing-box 子进程
- [ ] 重启后验证 ClashAPI 可用（`GET /version`），超时则上报错误

---

### D-3 sing-box 内核版本管理

**文件**：`src-tauri/src/update/`

- [ ] 从 sing-box GitHub Releases API 检查最新版本
- [ ] 与当前锁定版本（1.13.14）比较，提示用户是否升级
- [ ] 下载新版本二进制（SHA256 校验）
- [ ] 替换 `sidecar-bin/` 中的旧版本，重启 sidecar
- [ ] 新版本验证：启动后检查 `sing-box version` 输出

> ⚠️ 注意：用户升级内核版本时，需提醒可能存在 config 格式兼容性风险，建议备份当前配置。

---

### D-4 配置备份与回滚

**文件**：`src-tauri/src/core/config_builder.rs`（扩展）

- [ ] 每次生成新 `config.json` 前，备份上一版本到 `config.backup.json`
- [ ] sing-box 启动失败时，自动恢复备份并重试
- [ ] 前端提供「恢复上一次配置」入口（Settings → AdvancedPanel）

---

## 关键数据结构

### config.json 骨架（Sing-box 1.13.14 格式）

```json
{
  "log": { "level": "info", "output": "box.log" },
  "dns": {
    "servers": [
      { "tag": "remote", "address": "tls://1.1.1.1", "detour": "proxy" },
      { "tag": "local", "address": "223.5.5.5", "detour": "direct" }
    ],
    "rules": []
  },
  "inbounds": [
    {
      "type": "mixed",
      "tag": "mixed-in",
      "listen": "127.0.0.1",
      "listen_port": 7890
    }
  ],
  "outbounds": [
    { "type": "selector", "tag": "proxy", "outbounds": ["auto", "HK-01", "JP-01"] },
    { "type": "urltest", "tag": "auto", "outbounds": ["HK-01", "HK-02"],
      "url": "https://www.gstatic.com/generate_204", "interval": "5m" }
  ],
  "route": {
    "rule_set": [],
    "rules": [],
    "final": "proxy"
  },
  "experimental": {
    "clash_api": {
      "external_controller": "127.0.0.1:9090",
      "secret": ""
    }
  }
}
```

---

## 提交节点

```
feat(core): rule merging and auto-reconnect on wake (模块D)
```
