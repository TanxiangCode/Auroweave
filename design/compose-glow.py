# -*- coding: utf-8 -*-
"""
v7 合成：rsvg 渲染矢量底 + PIL 做外发光（绕开 librsvg filter+mask 的缺陷）
外发光用字形同色渐变：把渐变色 A blur 后，用未 blur 的 A mask 抠掉字形内部
"""
from PIL import Image, ImageFilter

S = 1024

# 1) 无 glow 的基础版（v6 去掉 glow 的结构 —— 重新渲染干净版）
base = Image.open('design-v6.png').convert('RGBA')

# 2) 白 A → 彩色渐变 A (rsvg 渲一个渐变 A)
# 直接用 v6 图抠: v6 的 A 主体像素就是渐变色, alpha 由 A-white 提供
aw = Image.open('A-white.png').convert('RGBA')
a_alpha = aw.split()[3]

# 渐变 A: 从 v6 取色（v6 A 主体像素 × A alpha）
v6_rgb = base.convert('RGB')
gradA = v6_rgb.copy()
gradA.putalpha(a_alpha)

# 3) 外发光: blur 渐变 A
halo = gradA.filter(ImageFilter.GaussianBlur(30))
# 抠掉字形内部: halo alpha -= A alpha
h_r, h_g, h_b, h_a = halo.split()
a_arr = h_a.load()
alpha_orig = a_alpha.load()
new_a = Image.new('L', (S,S), 0)
na = new_a.load()
for y in range(S):
    for x in range(S):
        v = a_arr[x,y] - alpha_orig[x,y]
        if v < 0: v = 0
        na[x,y] = v
halo.putalpha(new_a)

# 4) 合成: base + halo(50%) + A
out = base.copy()
out = Image.alpha_composite(out, halo)
out = Image.alpha_composite(out, gradA)
out.save('design-v7.png')

# 验证
px = out.convert('RGB').load()
print('上三角洞(500,330):', px[500,330])
print('右缘外(840,500):', px[840,500], 'vs 纯背景(100,900):', px[100,900])
print('紧邻右缘(826,500):', px[826,500])
