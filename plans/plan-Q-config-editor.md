# 模块 Q — 配置编辑器（JSON Schema 校验 + 安全写回）

> 状态：⏳ 待开始 | 优先级：🟡 中 | 前置：plan-M M3-3
>
> 拟定日期：2026-09-08 | 预估工作量：Q1 半天 + Q2 2~3 天（可分期） | 文档依据：`docs/sing-box_docs/schema.md`
>
> **本文档为设计文档 + 开发说明书合一**：Part A 供评审决策，Part B 供直接动工。

---

## Part A — 设计文档

### A.1 背景与动机

高级用户手改 config.json 的现有路径：外部编辑器改文件 → 应用重启才发现错误 → 内核拒载 → 排查错误在哪。项目有 config.backup.json 兜底恢复，但**没有"写错能提前发现"的机制**。

两个既有能力（均实测确认）让校验成本极低：

1. **`sing-box schema` 命令**（1.14.0）：实测本仓库二进制生成 **444KB Draft 2020-12 schema**，含全部 14 个顶层配置段与完整 $defs，且"反映当前构建实际包含的功能"（比官方在线 schema 更贴合本地裁剪构建）
2. **`sing-box check` 的错误定位格式**：实测报错形态 `decode config at <path>: dns.rules[0].rules[0]: <原因>`、`initialize dns router: dns rule[1]: rule-set not found: <tag>`——**段名 + 条目序号 + 原因**，可直接解析为编辑器错误面板

### A.2 分期架构（两个独立可交付的子项）

**Q1（轻形态，半天）：schema 导出 + `$schema` 注入**——用户拿 schema 到自己熟悉的外部编辑器（VS Code + JSON Schema 插件即得字段级补全与校验）。

**Q2（主体，2~3 天）：内置编辑器**——textarea + 保存前双重校验（schema 层 + check 层）+ 安全写回链。

分期理由：Q1 是 Q2 的零成本前置（schema 文件生成逻辑共享），且 Q1 单独已有完整价值（外部编辑器本来就比内置 textarea 强）；Q2 的 textarea 形态是够用主义选择（项目零 CodeMirror 依赖，语法高亮编辑器属锦上添花，明确不做，见 A.5）。

### A.3 校验分层设计（Q2 核心）

```
用户点击「保存」
   │
   ▼
L1 JSON 语法校验（serde_json 解析）
   │ 失败 → 错误面板显示行号 + "JSON 语法错误: <原因>"，终止
   ▼
L2 schema 校验（sing-box schema 生成的 Draft 2020-12）
   │ 实现选型见 A.4——L2 是可降级层
   ▼
L3 语义校验（sing-box check -c <临时文件>）
   │ 失败 → 解析 FATAL 输出的 "段.条目: 原因" 格式，错误面板定位显示，终止
   ▼
安全写回链：
   1. 现有 config.json → config.backup.json（既有备份机制复用）
   2. 临时内容原子写 config.json（fs_utils::atomic_write）
   3. 重启内核（SidecarManager 既有链）
   4. 前端 toast 提示生效
```

**check 不过绝不写回**是硬边界：内核拒载的所有已知原因（tag 引用不存在、嵌套动作非法、字段类型错）都会被 L3 抓住，这是整个编辑器可靠性的基石。L2 只是锦上添花的"字段名拼写"精确定位。

### A.4 设计决策

1. **L2 schema 校验的选型 = 只用 check、砍掉 schema 库**（推荐）：
   - 引入 `jsonschema` crate = 新编译依赖（该 crate 依赖 jsonschema 转换器全家桶），为"字段拼写精确定位"这一个收益付出体积/编译成本，不划算
   - `sing-box check` 一层已覆盖 95% 的实际错误（所有结构性/引用性错误）；JSON 语法错误 L1 已拦
   - **降级后的 L2**：保留"导出 schema"能力（Q1），内置校验只走 L1+L3；错误定位靠 check 的 `段.条目` 格式反查。设计文档留此决策点，实施时如发现 check 漏拦某类错误（如多余字段静默忽略）再评估升格 L2
2. **手改内容被 rebuild 覆盖的语义管理**——这是本项目特有的核心风险：订阅刷新/设置变更都会触发 ConfigBuilder 重新生成 config.json，**手改内容会被整体覆盖**。方案：
   - 编辑器打开时顶部固定提示："这是对当前生成配置的临时修改，任何订阅刷新或设置变更都会重新生成并覆盖此文件"
   - 首版不做"锁定配置"开关（改动大、与自动更新体系冲突），文档留档为 v2 决策点
3. **`$schema` 字段注入**：ConfigBuilder 生成配置时顶层注入 `"$schema": "https://sing-box.sagernet.org/schema.json"`——实测确认该字段不影响运行时（内核忽略），换来用户把 config.json 拿到任何兼容编辑器都自动获得校验。两处生成路径同步注入
4. **check 输出解析**：FATAL 行格式正则 `(?:decode config|initialize \w+).*?: ([^:]+): (.+)` 抽取定位与原因；**失败兜底**：解析不出定位就原样展示整行（不吞错）

### A.5 明确不做

| 不做项 | 原因 |
|---|---|
| CodeMirror/Monaco 富文本编辑器 | 语法高亮编辑器体积/依赖与收益不匹配；textarea + 校验已满足"安全改配置"核心诉求 |
| 内置 schema 校验库（jsonschema crate） | 见 A.4-1，check 一层够用 |
| 配置锁定开关 | 与自动更新体系冲突，v2 决策点 |
| 可视化配置表单 | 与"高级用户手改"定位重复，现有九面板已是表单面 |

---

## Part B — 开发说明书

### B.1 文件级改动清单

| 子项 | 文件 | 改动 |
|---|---|---|
| Q1 | `src-tauri/src/commands/singbox_update.rs` 或新 `commands/config_editor.rs` | `config_export_schema` 命令：调 `resolve_binary_path` + `schema -o <config_dir>/schema.json`，返回路径 |
| Q1 | `src-tauri/src/core/config_builder.rs` + `settings.rs` rebuild | 顶层 `"$schema"` 注入（两路径同步） |
| Q1 | `src/views/panels/AdvancedPanel.vue` | 「导出 JSON Schema」按钮（内核版本卡片内） |
| Q2 | `src-tauri/src/commands/config_editor.rs`（新） | `config_editor_load` / `config_editor_save` 两命令（见 B.2） |
| Q2 | `src/views/panels/ConfigEditorModal.vue`（新） | 编辑器弹窗（textarea + 错误面板 + 保存链） |
| Q2 | `src/views/panels/AdvancedPanel.vue` | 「编辑当前配置」入口（灾备卡片内，与恢复备份相邻） |
| Q2 | `src-tauri/src/lib.rs` | 命令注册 |

### B.2 后端命令设计

```rust
/// Q2：加载当前配置原文（含 $schema 注入后的）
#[tauri::command]
pub async fn config_editor_load() -> Result<ApiResponse<String>, AppError>;

/// Q2：校验 + 安全写回。
/// 返回校验结果：L1 失败 → EditCheckError::JsonSyntax{line, message}
///             L3 失败 → EditCheckError::CheckFailed{location, message}
///             全过 → 写回并返回 ok（调用方触发内核重启确认）
#[tauri::command]
pub async fn config_editor_save(
    app_handle: tauri::AppHandle,
    content: String,
) -> Result<ApiResponse<EditSaveResult>, AppError>;
```

`config_editor_save` 内部流程（严格按 A.3 顺序）：

```rust
// L1: serde_json 解析（行号：serde error 的 line/col 字段直接可用）
let parsed: serde_json::Value = match serde_json::from_str(&content) {
    Ok(v) => v,
    Err(e) => return Ok(ApiResponse::ok(EditSaveResult::blocked_syntax(e.line(), e.column(), &e))),
};

// L3: 写临时文件 → sing-box check
let tmp = config_dir.join("config_edit_check.json");
fs_utils::atomic_write(&tmp, content.as_bytes())?;
let output = Command::new(binary).arg("check").arg("-c").arg(&tmp).output().await;
fs::remove_file(&tmp);
if !output.status.success() {
    // 解析 FATAL 行 → 定位 + 原因（正则见 A.4-4；解析失败原样返回 stderr 全文）
    return Ok(ApiResponse::ok(EditSaveResult::blocked_check(parse_fatal(&output.stderr))));
}

// 安全写回：备份现有 → 原子写 → true
let backup = config_dir.join("config.backup.json");
let _ = fs::copy(config_path, &backup);
fs_utils::atomic_write(&config_path, content.as_bytes())?;
Ok(ApiResponse::ok(EditSaveResult::saved))
```

**注意**：写回后**不自动重启内核**——弹窗内给「立即重启内核生效」确认按钮（用户可能还想连改几处），重启走既有 SidecarManager restart 链。

### B.3 前端实现要点

`ConfigEditorModal.vue` 骨架：

```
┌─ 编辑当前内核配置 ────────────────────┐
│ ⚠ 临时修改提示条（A.4-2 文案，固定顶部） │
│ ┌────────────────────────────┐ │
│ │ <textarea> config.json 原文     │ │  行高对齐、等宽字体（--font-mono）
│ └────────────────────────────┘ │
│ 错误面板（校验失败时显示：定位 + 原因，红字）│
│              [取消]  [校验并保存]  [重启内核]│  重启钮仅 saved 后可用
└──────────────────────────────────┘
```

交互细节：
- textarea 默认等宽字体 + tab 支持（keydown 拦截 Tab 插入两个空格——纯体验项，可选）
- 保存被拦时错误面板显示定位，textarea **不滚动定位**（serde 行号到 textarea 定位需行计算，首版只显示行号让用户自己找——够用）
- 巨型 config（300 节点 ~MB 级）：textarea 渲染无压力（纯文本），但**加载时给出大小提示**；不设截断（截断会静默破坏配置完整性，不可接受）

### B.4 验证计划

1. **cargo 单测**：`parse_fatal` 正则——构造 4 类 FATAL 输出（decode config / initialize router / initialize dns router / 无定位格式）断言解析
2. **check 实测**：故意注入 3 类错误（坏 JSON、rule-set 引用不存在、嵌套动作非法——第三类用 M3-2 实测的 `dns.rules[0].rules[0]` 样例）走完整保存链，确认拦截且定位正确
3. **写回链实测**：合法修改保存 → config.backup.json 内容 = 修改前 → config.json = 修改后 → 重启内核 ClashAPI 就绪
4. **回归**：正常订阅刷新链路不受编辑器影响（编辑器只读 config.json，不碰生成器状态）
5. **vue-tsc / vite build / vitest** 全套

### B.5 验收标准

- [ ] Q1：schema 导出按钮产出 444KB 级合法 Draft 2020-12 文件；VS Code 打开 config.json 有字段补全（`$schema` 注入生效）
- [ ] Q2：坏 JSON/坏引用/坏结构三类修改全部保存被拦，错误面板给出定位
- [ ] 合法修改保存后内核重启生效，改动前内容自动入 backup
- [ ] 编辑器打开期间订阅刷新 → 提示条预警覆盖风险（提示已在，验证文案可见性）

### B.6 排期建议

Q1（半天）可与 plan-P 同批顺手交付（`$schema` 注入两路径改动与 P 的 dns 段改动同文件不同段，无冲突）；Q2 独立排期，等 P 验收后启动——两批改动都动 config 生成面，串行降低复盘成本。
