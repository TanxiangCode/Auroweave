# -*- coding: utf-8 -*-
"""
缠绕环 v3 精修：
1. 中心加「极光核」：小径向柔光球（青白→透明），呼应 aurora，解决中心空
2. 环带渐变改为「沿环走向」（参数角）而非「画布 atan2 角」——圆环用画布角=参数角一致;
   椭圆环此前用画布角导致色不沿环走 → 改用参数角
3. 环带外缘加 1px 亮描边（高光环缘），强化小尺寸轮廓
4. 环带内侧轻微内阴影——增加立体
"""
from PIL import Image, ImageDraw, ImageFilter
import math

S4 = 4096
CX = CY = S4 // 2
RING_W = 340
R_CIRC = 1080
A_ELL, B_ELL = 1210, 1020
ROT = math.radians(-60)

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

A_STOPS = [(0.0, (158, 232, 255)), (0.5, (77, 165, 255)), (1.0, (150, 105, 250))]
B_STOPS = [(0.0, (160, 125, 255)), (0.5, (196, 92, 235)), (1.0, (249, 145, 200))]

def ring_band(r_mid, rot, squish, stops, ang_off, width=RING_W):
    out = Image.new('RGBA', (S4, S4), (0,0,0,0))
    d = ImageDraw.Draw(out)
    half = width//2
    steps = 2000
    for i in range(steps):
        th = 2*math.pi*i/steps
        ex = r_mid*math.cos(th)
        ey = r_mid*math.sin(th)*squish
        rx = CX + ex*math.cos(rot) - ey*math.sin(rot)
        ry = CY + ex*math.sin(rot) + ey*math.cos(rot)
        t = ((th + ang_off) % (2*math.pi)) / (2*math.pi)   # 参数角！颜色沿环走
        c = sample(stops, t)
        d.ellipse([rx-half, ry-half, rx+half, ry+half], fill=(c[0], c[1], c[2], 255))
    return out

ringA = ring_band(R_CIRC, 0, 1.0, A_STOPS, math.pi*0.75)
ringB = ring_band(A_ELL, ROT, B_ELL/A_ELL, B_STOPS, math.pi*1.55)
ringA.save('ringA3.png'); ringB.save('ringB3.png')

# 交点（与 weave2 相同几何）
pts = []
for i in range(7200):
    th = 2*math.pi*i/7200
    ex = A_ELL*math.cos(th); ey = B_ELL*math.sin(th)
    rx = CX + ex*math.cos(ROT) - ey*math.sin(ROT)
    ry = CY + ex*math.sin(ROT) + ey*math.cos(ROT)
    if abs(math.hypot(rx-CX, ry-CY) - R_CIRC) < 4:
        if not any(math.hypot(rx-p[0], ry-p[1]) < 100 for p in pts):
            pts.append((rx, ry))
pts.sort(key=lambda p: math.atan2(p[1]-CY, p[0]-CX))
assert len(pts) == 4

r_patch = int(RING_W*0.95)
def cut(ring, cx, cy, r):
    hole = Image.new('L', (S4,S4), 255)
    ImageDraw.Draw(hole).ellipse([cx-r, cy-r, cx+r, cy+r], fill=0)
    alpha = ring.split()[3]
    new_alpha = Image.composite(alpha, Image.new('L', (S4,S4), 0), hole)
    out = ring.copy()
    out.putalpha(new_alpha)
    return out

B_cut, A_cut = ringB, ringA
for i, p in enumerate(pts):
    if i % 2 == 0:
        B_cut = cut(B_cut, p[0], p[1], r_patch)
    else:
        A_cut = cut(A_cut, p[0], p[1], r_patch)

comp = Image.new('RGBA', (S4,S4), (0,0,0,0))
comp = Image.alpha_composite(comp, B_cut)
comp = Image.alpha_composite(comp, A_cut)
comp.save('weave3-bands.png')
print('bands done')
