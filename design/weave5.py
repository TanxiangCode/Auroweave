# -*- coding: utf-8 -*-
"""
绳编环（原版 DNA 的数学化）：
- 三股全程 360° 并行,径向位置按 sin 沿角度缓慢换位（绳辫）
- 股 i 的径向中心: rc_i(θ) = R + A·sin(θ·N + 2πi/3), N=1 → 绕一圈三股完成一次内外轮换
- 股宽 W,A·(2-覆盖) → 三股径向总有重叠 → 筒是实心,接缝自然交织
- depth 由径向速度决定? 更简单: 三股互不遮挡时按颜色直接画;重叠区按"谁在外(半径大)谁在前"
  —— 用 (rc 中心半径) 排序,外侧先画会被内侧盖? 不对,外侧应在前(视觉近)。
  实际绳编穿插: 每股交替处于"前/后"。用 depth = sin 的相位导数不可靠,
  改用经典绳辫规则: 股在前 ⇔ 径向中心速度朝外(正在向外翻)。
  简化且视觉正确的做法: 相邻股重叠带按像素画,重叠区取"径向中心更靠外"的股在前。
- 绘制: 沿 θ 步进 0.2°,每股画一条弧线段(宽 W),重叠区由"外前内后"自然覆盖 →
  视觉呈现绳辫的 X 交叉。交叉点的上下交替来自径向位置的连续交换。
"""
from PIL import Image, ImageDraw
import math

S4 = 4096
CX = CY = S4 // 2
R_TUBE = 1130          # 筒中心半径
AMP = 190              # 径向摆幅(三股换位)
STRAND_W = 420          # 股宽(径向) → 三股中心距 = 2·AMP·sin(60°)... 覆盖筒厚
N_STRANDS = 3

def sample(stops, t):
    t = min(1, max(0, t))
    if t <= 0: return stops[0][1]
    if t >= 1: return stops[-1][1]
    for i in range(len(stops)-1):
        (ta, ca), (tb, cb) = stops[i], stops[i+1]
        if ta <= t <= tb:
            u = (t-ta)/(tb-ta)
            return tuple(int(ca[j]+(cb[j]-ca[j])*u) for j in range(3))
    return stops[-1][1]

GRADS = [
    [(0, (104, 229, 247)), (0.5, (77, 193, 255)), (1, (72, 156, 255))],
    [(0, (72, 156, 255)), (0.5, (101, 122, 255)), (1, (134, 108, 252))],
    [(0, (134, 108, 252)), (0.5, (192, 101, 240)), (1, (238, 130, 196))],
]

# 每股径向中心: R + AMP·sin(θ + φi), φi = 2πi/3 → 三股在任意角度的径向位互错
# 但这给出三股"同相绕圈"——绳辫需要换位: 用 N=1.5? 标准三辫: 股位置循环置换,
# 用 θ + 2πi/3 相位差 + N=1 → 三股沿圈只完成"位置轮换"不产生交叉感
# 要"交叉": 让每股径向中心 rc_i(θ) = R + AMP·sin(θ·3 + φi) (N=3 波)
# → 每 120° 一次径向交换 → 视觉产生 6 个交叉点/圈,正是绳辫!
def rc(strand, theta):
    return R_TUBE + AMP*math.sin(3*theta + strand*2*math.pi/3)

im = Image.new('RGBA', (S4,S4), (0,0,0,0))

# 绘制策略: 逐 θ 微段,每股一条弧段;重叠区"半径大的后画(在前)"
# 收集 (θ, strand, r_center, color) → 分层画: 为真实穿插,同一 θ 处按 rc 排序,
# 先画 rc 小的(内,后),后画 rc 大的(外,前) → 每个微段自然形成前后来回交替
segs = []
STEP = 0.12
n_steps = int(360/STEP)
d = ImageDraw.Draw(im)
# 按 θ 推进,每 θ 内按 rc 从小到大画三股
for i in range(n_steps):
    th = math.radians(i*STEP)
    strand_at = []
    for s in range(N_STRANDS):
        r_c = rc(s, th)
        t = ((th/math.tau) + s/3) % 1
        c = sample(GRADS[s], (th/math.tau) % 1)  # 颜色沿股自身参数(全局角度)
        strand_at.append((r_c, s, c))
    strand_at.sort(key=lambda x: x[0])   # 内→外
    # 交叉检测: 径向中心距离 < 半股宽 → 该微段给"前股"颜色混入后股 40%
    for j in range(len(strand_at)):
        r_c, s, c = strand_at[j]
        if j > 0:
            r_prev, s_prev, c_prev = strand_at[j-1]
            gap = r_c - r_prev
            if gap < STRAND_W*0.55:
                u = 1 - gap/(STRAND_W*0.55)
                c = tuple(int(c[k]*(1-0.4*u) + c_prev[k]*0.4*u) for k in range(3))
        x1 = CX + r_c*math.cos(th); y1 = CY + r_c*math.sin(th)
        th2 = th + math.radians(STEP)
        x2 = CX + r_c*math.cos(th2); y2 = CY + r_c*math.sin(th2)
        d.line([x1,y1,x2,y2], fill=(c[0],c[1],c[2],255), width=STRAND_W)

im.save('weave5-bands.png')
print('weave5 done')
