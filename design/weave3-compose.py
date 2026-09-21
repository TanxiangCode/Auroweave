# v3 合成：底 + 外发光 + 环带 + 极光核 + 高光环缘
from PIL import Image, ImageDraw, ImageFilter
import math

S4, S = 4096, 1024
R_CIRC = 1080
CX = CY = S4 // 2
bg = Image.open('bg-4096.png').convert('RGBA')
bands = Image.open('weave3-bands.png').convert('RGBA')

# 1) 外发光（同 v2 手法）
halo = bands.filter(ImageFilter.GaussianBlur(80))
w_a = bands.split()[3]; h_a = halo.split()[3]
wa, ha = w_a.load(), h_a.load()
new_a = Image.new('L', (S4,S4), 0); na = new_a.load()
for y in range(S4):
    for x in range(S4):
        v = ha[x,y] - wa[x,y]
        na[x,y] = v if v > 0 else 0
halo.putalpha(new_a)

# 2) 极光核：中心柔光球（青白核 → 蓝紫晕 → 透明），半径 ~环内径 70%
core = Image.new('RGBA', (S4,S4), (0,0,0,0))
cd = ImageDraw.Draw(core)
r_core = int(R_CIRC*0.42)
for rr in range(r_core, 0, -4):
    t = rr/r_core
    # 中心亮 → 边缘透明
    a = int(55 * (1-t)**2)
    cd.ellipse([CX-rr, CY-rr, CX+rr, CY+rr], fill=(80, 190, 255, a))
core = core.filter(ImageFilter.GaussianBlur(50))
# 再加一个更小更亮的芯
cd2 = ImageDraw.Draw(core)
for rr in range(int(r_core*0.35), 0, -3):
    t = rr/(r_core*0.35)
    a = int(150 * (1-t)**2)
    cd2.ellipse([CX-rr, CY-rr, CX+rr, CY+rr], fill=(200, 240, 255, a))
core = core.filter(ImageFilter.GaussianBlur(20))

# 3) 环带高光缘：环带 mask blur 1px 提边 → 上移 6px 的差集 = 上缘亮线
alpha_img = bands.split()[3].copy()
shifted = Image.new('L', (S4,S4), 0)
shifted.paste(alpha_img, (0, 8))   # 下移8px:差集=上边缘
edge = Image.new('L', (S4,S4), 0)
from PIL import ImageChops
edge = ImageChops.subtract(alpha_img, shifted)
edge = edge.filter(ImageFilter.GaussianBlur(3))
edge_rgba = Image.new('RGBA', (S4,S4), (255,255,255,0))
edge_rgba.putalpha(edge.point(lambda v: int(v*0.35)))

comp = bg.copy()
comp = Image.alpha_composite(comp, core)
comp = Image.alpha_composite(comp, halo)
comp = Image.alpha_composite(comp, bands)
comp = Image.alpha_composite(comp, edge_rgba)
comp = comp.resize((S,S), Image.LANCZOS)
comp.save('weave3-final.png')

px = comp.convert('RGB').load()
print('中心(512,512):', px[512,512], '(应有极光核微光)')
print('idx0(404,266) 蓝A:', px[404,266])
print('idx1(780,470) 紫粉B:', px[780,470])
print('idx2(620,758):', px[620,758], 'idx3(244,554):', px[244,554])
print('角alpha:', comp.load()[0,0][3])
comp.resize((512,512), Image.LANCZOS).save('weave3-512.png')
print('saved')
