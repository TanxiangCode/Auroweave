# Auroweave 图标设计源文件

当前定稿：**深蓝紫夜空底 + 三股绳辫式缠绕环**——三股环带（青绿→天蓝→紫粉，三段极光色系）沿圆环骨架 360° 并行编织，径向位置按 `rc(θ) = R + A·sin(3θ + 2πi/3)` 连续换位，形成六个编织交叉点；交叉处颜色按间隔比例混色（40% 上限）；环筒实心粗壮；双层外光晕（近层 blur40/高亮 + 远层 blur110/低亮）。

## 文件
- `bg-only.svg` — 夜空背景矢量层（squircle rx=231）
- `weave5.py` — 三股绳辫环带生成（4096 超采样 + 交叉混色）
- `weave3.py` / `weave3-compose.py` — 旧方案「双粗环带穿插」（弃用，留档）
- `trefoil.py` — 旧方案「三叶结」（弃用，留档）
- `auroweave-icon.svg` — 旧方案「A 字标」（弃用，留档）

## 再生流程
```bash
cd design
# 1. 渲染背景 4096（输出到脚本引用路径）
/opt/homebrew/bin/rsvg-convert -w 4096 -h 4096 bg-only.svg -o /tmp/icon-design/bg-4096.png
# 2. 三股环带
python3 weave5.py            # 输出 /tmp 内 weave5-bands.png（路径见脚本）
# 3. 合成（双层光晕 + 缩到 1024）——代码在会话记录中，要点见下
# 4. 全套平台图标（仓库根目录）
node_modules/.bin/tauri icon <weave5-final.png>
# 5. icon.png / app-icon-squircle.png 手工覆盖为 1024 母版（tauri icon 输出 512）
# 6. 托盘：weave5-bands.png 的 alpha bbox 裁剪按短边缩到 32px 全幅
```

## 关键实现要点（踩坑记录）
- **绳辫模型**：三股全程 360° 并行（不是各占 120° 扇区），径向中心 `R + A·sin(3θ + φᵢ)`，`φᵢ = 2πi/3`。任意角度环体剖面三股都可见 → 环筒实心；径向换位 → 六个自然交叉。
- **交叉前后关系**：同一角度微段内按径向中心从小到大绘制（内先外后），外侧覆盖内侧 → 视觉自然交替；交叉微段按间隔混色避免突兀。
- **外发光**：librsvg 的 filter+mask 组合失效，用 PIL：blur 后 alpha 减原 alpha（负值归零）得「仅留字形外」光晕；双层（近亮远淡）更有空气感。
- **PIL `split()` 返回副本**：写 `split()[3].load()` 不影响原图，挖洞必须用洞 mask + `putalpha`。
- 环体（1024 坐标）半径带约 [235, 330]，占画幅 72%——与 Antigravity 实测的主体占比（60%）相当偏满，保留大留白。
