# 贡献指南

感谢你对 Auroweave 的兴趣！本文档说明参与开发所需了解的所有规范。

---

## 目录

- [行为准则](#行为准则)
- [开发环境搭建](#开发环境搭建)
- [分支与工作流](#分支与工作流)
- [提交规范](#提交规范)
- [代码规范](#代码规范)
- [测试要求](#测试要求)
- [Pull Request 流程](#pull-request-流程)

---

## 行为准则

- 保持尊重与建设性
- 优先考虑用户利益与产品一致性
- 涉及平台权限、系统 API 的代码，必须在真实设备上验证

---

## 开发环境搭建

### 必备工具

| 工具 | 版本要求 | 用途 |
|---|---|---|
| Node.js | 20+ | 前端构建 |
| Rust | 1.77+ (stable) | Tauri 后端 |
| rustup | 最新 | Rust 工具链管理 |
| Git | 2.x+ | 版本控制 |

### Windows 额外要求

```powershell
# 安装 Visual Studio C++ 构建工具（含 MSVC + Windows SDK）
winget install Microsoft.VisualStudio.2022.BuildTools
```

### macOS 额外要求

```bash
xcode-select --install
```

### 初始化项目

```bash
git clone https://github.com/TanxiangCode/gauzeweave.git
cd auroweave
npm install
```

### 开发命令速查

```bash
npm run tauri dev      # 启动完整 Tauri 开发环境
npm run dev            # 仅启动前端（无 Tauri，用于 UI 快速迭代）
npm run build:install  # 打包 release 并安装到当前系统（Win/macOS/Linux 自动分流）
npm run version:check  # 校验 8 处版本号是否一致（发版前必跑）
npm run lint           # ESLint 检查并自动修复
npm run test:unit      # 运行前端单元测试（vitest）
cargo test             # 运行 Rust 单元测试（在 src-tauri/ 目录下执行）
```

---

## 分支与工作流

```
main          ← 稳定版本，只接受来自 develop 的 PR
  └── develop ← 日常集成分支
        ├── feature/xxx   ← 新功能
        ├── fix/xxx       ← Bug 修复
        └── chore/xxx     ← 工具链、文档、配置变更
```

**规则：**
- 永远不要直接推送到 `main`
- 功能分支从 `develop` 创建，完成后 PR 回 `develop`
- 涉及核心逻辑的改动，PR 必须附带对应测试用例

---

## 提交规范

遵循 [Conventional Commits](https://www.conventionalcommits.org)，**提交信息使用英文**：

```
<类型>(<范围>): <简短描述>

[正文（可选，用中文描述技术细节）]

[页脚（可选，关联 issue）]
```

### 类型列表

| 类型 | 用途 |
|---|---|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `chore` | 构建工具、依赖、配置变更 |
| `docs` | 仅文档变更 |
| `style` | 代码格式（不影响逻辑） |
| `refactor` | 重构（非新功能、非 Bug 修复） |
| `test` | 添加或修改测试 |
| `perf` | 性能优化 |

### 示例

```
feat(speedtest): add single-node throughput test command

- 实现 speedtest_run_single IPC 命令
- 通过对应出站代理向测速服务器发起分块请求
- 限定测试时长 8 秒，计算平均速率
```

---

## 代码规范

### 通用原则

- **语言**：源码标识符（变量名、函数名、类型名）用**英文**；代码注释、文档用**中文**
- **单一职责**：一个函数/组件只做一件事；超过 300 行须拆分
- **禁止魔法值**：端口、超时、版本号等常量统一在 `src/constants.ts` / `constants.rs` 中定义

### 前端（Vue 3 + TypeScript）

- 统一使用 `<script setup lang="ts">`，禁止 Options API
- 开启 TypeScript `strict` 模式，**禁止使用 `any`**
- 样式只使用 Tailwind 原子类 + Design Tokens CSS 变量，**禁止在组件内硬编码颜色/圆角/阴影**
- 禁止在组件内直接调用 `invoke()`，必须通过 `src/api/ipc/` 封装层
- 所有装饰性动画必须通过 `useReducedMotion()` 判断后再决定是否播放

### 命名约定（前端）

| 对象 | 规范 |
|---|---|
| 组件文件 | `PascalCase.vue` |
| Composable | `use` 前缀 + camelCase，如 `useFluidWave.ts` |
| Pinia Store | `xxx.store.ts` |
| IPC 封装函数 | camelCase，如 `getProxyGroups` |

### 后端（Rust）

- 强制使用 `rustfmt` 格式化，`clippy` 检查为 CI 阻断项
- **禁止** `unwrap()` / `expect()` 用于可能失败的业务逻辑，统一使用 `Result<T, AppError>`
- I/O 密集型操作一律使用 `tokio` 异步
- 日志使用 `tracing` 库，按模块设置 target，**禁止** `println!`
- IPC 命令命名格式：`模块_动作`，如 `speedtest_run_single`

### IPC 边界约定

- 先在 `src/types/index.ts` 定义 TypeScript 接口，再在 Rust 端定义对应 `serde` struct
- 双方字段命名保持一致（均用 `snake_case`）
- 所有命令返回统一的 `ApiResponse<T>` 结构

---

## 测试要求

### 前端单元测试（vitest）

必须覆盖：
- 订阅解析器（多格式兼容性）
- 语义化规则翻译（含兜底逻辑）
- 工具函数（速度格式化等）

### Rust 单元测试

必须覆盖：
- 连接计数器 `connection_counter.rs`
- 测速调度器串行逻辑
- `ApiResponse` 序列化正确性

### AI 辅助开发约定

- 每次让 AI 生成涉及 IPC 边界的代码时，先明确双方数据结构再生成实现
- AI 生成的核心解析/测速/规则合并逻辑，要求同步生成对应单元测试
- 涉及权限、系统 API、窗口原生样式的代码，生成后必须在真实设备上验证

### 版本号规范

版本号曾散落在 8 处，现已收敛为**只需人工维护 2 个文件**：

| 位置 | 角色 |
| --- | --- |
| `package.json` | **npm 侧真源** |
| `Cargo.toml` 的 `[workspace.package]` | **Rust 侧真源** |

其余 7 处全部自动派生，**永远不要手改**：

| 派生位置 | 由谁同步 |
| --- | --- |
| `package-lock.json` ×2 | `npm install --package-lock-only` |
| `src-tauri/tauri.conf.json` | 写的是 `"version": "../package.json"`，Tauri 直接读 package.json |
| `src-tauri/Cargo.toml`、`crates/auroweave-svc/Cargo.toml` | 均为 `version.workspace = true`，继承 workspace 根 |
| `Cargo.lock` ×2 | `cargo metadata` |

一条命令完成升版（`--` 不可省，npm 需要它把参数转给脚本）：

```bash
npm run version:set -- 0.8.0    # 写 2 处真源
npm install --package-lock-only # 派生 npm 侧
cargo metadata --format-version 1 > /dev/null   # 派生 Rust 侧
npm run version:check           # 校验全部一致（漂移则 exit 1）
```

> 第一次改 `tauri.conf.json` 时，把 `version` 的值填成字符串路径
> `"../package.json"` 即可，官方 schema 明确支持（也支持省略后回退读
> `Cargo.toml` 的 version）。

**何时该升版本**（0.x 阶段语义）：

- `0.y.0`：新增功能、架构演进、里程碑交付（哪怕只是内部能力）
- `0.y.z`：仅缺陷修复、文案与样式调整
- **每次合并到 `main` 的功能/修复类改动都要按上表升版本**，不要累积多个版本再补

发版流程：

1. `npm run version:set -- <x.y.z>` + 上面两条派生命令
2. 在 `CHANGELOG.md` 把 `[Unreleased]` 内容归入新的 `## [x.y.z]` 条目并写日期
3. `npm run version:check` 确认全部一致
4. `git tag -a v<x.y.z> -m "..."` 并推送

> **注意**：本仓库在 2026-09-27 之前**从未打过 tag**，`0.1.1` 之后的 202 个提交
> 长期未记录在 CHANGELOG 中。当天已按 `TASK.md` 里程碑与提交聚类补全
> `0.1.0`~`0.7.0` 的条目并补打 tag，其中 `0.2.0`~`0.7.0` 的版本边界是
> **重建的"应有发布点"**，非历史真实发版。

---

## Pull Request 流程

1. Fork 仓库并创建功能分支
2. 完成功能开发与测试
3. 按上文「版本号规范」升版本并补 CHANGELOG 条目
4. 运行 `npm run lint` 确保无 lint 错误
5. 运行 `npm run test:unit` 确保测试全部通过
6. 运行 `npm run version:check` 确认版本号无漂移
7. 提交 PR 到 `develop` 分支
6. 在 PR 描述中说明：
   - 解决的问题
   - 技术实现简述
   - 测试方式
   - 平台验证情况（Windows / macOS）

### PR 代码评审清单

- [ ] 是否有硬编码常量（颜色、端口、版本号）
- [ ] 是否有裸露的 `unwrap()` / `any`
- [ ] 动效代码是否接入 `useReducedMotion()`
- [ ] IPC 边界是否有类型定义
- [ ] 新增功能是否有对应测试
