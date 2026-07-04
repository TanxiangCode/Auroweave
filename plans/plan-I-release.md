# 模块 I — 联调、测试与打包发布

> 状态：⏳ 待开始 | 优先级：🟠 高（最终里程碑）| 前置：模块 B~H 全部

---

## 目标

跨模块联调验证所有功能正确工作，补全测试用例，实现首次发布版本打包。

**验收标准**：
- 所有自动化测试通过
- 手动测试清单 100% 完成
- Windows `.msi` + macOS `.dmg` 安装包可正常安装运行

---

## 任务清单

### I-1 跨模块联调

**全链路 1：订阅导入 → 翻墙**
- [ ] 导入真实机场订阅（Clash YAML 格式）
- [ ] 验证 config.json 生成正确
- [ ] 验证 sing-box 成功拉起并监听 7890
- [ ] 验证访问 github.com 走代理出站
- [ ] 验证访问 baidu.com 走直连

**全链路 2：实时数据流**
- [ ] WebSocket `/traffic` → store → Dashboard 能量核动画 全链路
- [ ] WebSocket `/connections` → store → Audit 页连接列表 全链路

**全链路 3：App-Matrix**
- [ ] 绑定 Chrome 到香港节点
- [ ] 用 Cursor 访问 github.com，验证流量走指定节点
- [ ] 解绑后验证恢复默认行为

**全链路 4：测速**
- [ ] 单节点测速：结果与 Speedtest.net 误差 ≤ 20%
- [ ] 批量测速：串行执行，二次确认弹窗数字准确

**全链路 5：自动化**
- [ ] SSID 变化（模拟切换热点）→ 自动切换代理模式 触发验证

---

### I-2 前端单元测试补全

**文件**：`tests/unit/`

- [ ] `subscription-parser.test.ts` — Clash / V2ray / Sing-box 格式解析
- [ ] `semantic-translator.test.ts` — 规则翻译（含兜底逻辑覆盖）
- [ ] `speed-formatter.test.ts` — 速度格式化（bytes/s → MB/s 等）
- [ ] `ema-smoother.test.ts` — EMA 平滑算法边界值测试
- [ ] `region-parser.test.ts` — 节点地区标记解析（HK / 香港 / 🇭🇰 等）

---

### I-3 Rust 单元测试补全

**文件**：`tests/rust/` + 各模块内 `#[cfg(test)]`

- [ ] `connection_counter` — 计数逻辑、跨日重置、并发安全
- [ ] `config_builder` — 生成的 JSON 格式校验（反序列化验证）
- [ ] `speedtest::scheduler` — 串行调度顺序验证
- [ ] `subscription` 解析器 — 多格式兼容性

---

### I-4 E2E 测试

**文件**：`tests/e2e/`（Playwright）

- [ ] 安装 `@playwright/test`
- [ ] 基础流程：启动 → 导入订阅 → 连接 → Dashboard 显示「运行中」
- [ ] 命令框：呼出 → 输入节点名 → 选择 → 切换成功
- [ ] Audit 页：打开 → 看到语义化卡片 → 切换到原始日志

---

### I-5 手动测试清单

**平台**：Windows 11 + macOS（各一台）

- [ ] `prefers-reduced-motion` 开启时，能量核降为静态渐变环（无旋转）
- [ ] 性能模式开启后，磨砂玻璃取消（无模糊效果），动效全停
- [ ] 低配置 Windows 虚拟机（集显）帧率抽测（目标：日常 UI 60fps，动效降级后 30fps+）
- [ ] Windows 11 Snap Layout 悬浮菜单是否保留
- [ ] macOS 红绿灯区域是否可正常拖拽窗口（无误触）
- [ ] 系统休眠唤醒后，代理自动重连（≤ 5s）
- [ ] 批量测速二次确认弹窗耗时/流量数字准确
- [ ] 订阅流量信息展示正确（若订阅头包含 `Subscription-Userinfo`）

---

### I-6 Onboarding 引导流程

**文件**：`src/components/onboarding/OnboardingModal.vue`

首次启动时展示：
- [ ] Step 1：欢迎页 + 核心功能介绍
- [ ] Step 2：请求必要权限（TUN 模式可选）
- [ ] Step 3：导入第一个订阅
- [ ] Step 4：完成（进入 Dashboard）

---

### I-7 Windows 打包

**文件**：`src-tauri/tauri.conf.json` + `.github/workflows/release.yml`

- [ ] 配置 `bundle.windows` （NSIS 或 WiX `.msi`）
- [ ] 代码签名（若有 EV 证书；否则跳过签名，README 说明）
- [ ] 安装包测试（全新 Windows 11 虚拟机）

---

### I-8 macOS 打包

- [ ] 配置 `bundle.macos`（`.dmg`，adhoc 自签）
- [ ] 打包产物内置 `INSTALL.md`，说明 `xattr -cr` 绕过 Gatekeeper 步骤
- [ ] 安装包测试（全新 macOS 虚拟机或真机）

---

### I-9 GitHub Actions Release 流程

**文件**：`.github/workflows/release.yml`

触发条件：推送 `v*` tag（如 `v0.1.0`）

```yaml
jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - npm run tauri build
      - 上传 .msi artifact

  build-macos:
    runs-on: macos-latest
    steps:
      - npm run tauri build
      - 上传 .dmg artifact

  release:
    needs: [build-windows, build-macos]
    steps:
      - 创建 GitHub Release
      - 上传两个平台的安装包
      - 更新 CHANGELOG.md
```

---

### I-10 CHANGELOG 更新

- [ ] 整理所有模块的变更，更新 `CHANGELOG.md` v0.1.0 正式版条目
- [ ] 补全 README 中的版本徽章

---

## 提交节点

```
chore(release): prepare v0.1.0 production release (M5)
```
