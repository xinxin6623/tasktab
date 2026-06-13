#!/usr/bin/env python3
"""生成 TaskBoard 应用图标源图（1024×1024 透明 PNG）。

照参考图复刻：白色圆角方主体 + 彩虹斜条纹大对勾 + 左上角多层彩色纸张叠影。
依赖仅 Pillow。生成后交给 `tauri icon icon_source.png` 切全套尺寸。
"""
from PIL import Image, ImageDraw, ImageFilter
import math

S = 1024                      # 画布边长
SS = 4                        # 超采样倍率（先画 4x 再缩小，边缘更顺）
W = S * SS

img = Image.new("RGBA", (W, W), (0, 0, 0, 0))
d = ImageDraw.Draw(img)


def rounded(draw, box, radius, fill):
    draw.rounded_rectangle(box, radius=radius, fill=fill)


# ── 主体方块几何（macOS 图标留白：内容约占 80%）──────────────────────
margin = int(W * 0.10)
main_box = (margin, margin, W - margin, W - margin)
corner = int(W * 0.225)       # macOS squircle 风格大圆角

# ── 左上角多层"纸张"叠影：每层往左上偏移、依次露出一条彩色边 ──────
# 顺序从最底（最靠左上）到最顶，颜色：紫蓝 → 蓝 → 绿 → 黄 → 橙 → 红
layer_colors = [
    (124, 92, 220),   # 紫
    (66, 133, 244),   # 蓝
    (52, 168, 83),    # 绿
    (251, 188, 5),    # 黄
    (255, 145, 40),   # 橙
    (234, 67, 53),    # 红
]
step = int(W * 0.022)         # 每层错位量
for i, col in enumerate(layer_colors):
    off = (len(layer_colors) - i) * step
    box = (main_box[0] - off, main_box[1] - off,
           main_box[2] - off, main_box[3] - off)
    rounded(d, box, corner, col + (255,))

# ── 主体白卡（略带暖色，照参考图的米白）─────────────────────────────
rounded(d, main_box, corner, (247, 244, 240, 255))

# ── 彩虹对勾：把勾笔画当作一条粗折线，沿线铺彩虹斜条纹 ───────────────
# 勾的两个拐点（相对主体框的比例坐标）
mx0, my0, mx1, my1 = main_box
mw = mx1 - mx0
mh = my1 - my0


def pt(fx, fy):
    return (mx0 + fx * mw, my0 + fy * mh)


p_start = pt(0.30, 0.52)      # 短笔起点（左上）
p_corner = pt(0.44, 0.70)     # 勾底拐点
p_end = pt(0.74, 0.30)        # 长笔终点（右上）

stroke = int(W * 0.13)        # 勾的粗细

# 先把勾画成一条纯色路径做 mask
mask = Image.new("L", (W, W), 0)
md = ImageDraw.Draw(mask)
md.line([p_start, p_corner, p_end], fill=255, width=stroke, joint="curve")
for p in (p_start, p_corner, p_end):
    r = stroke // 2
    md.ellipse((p[0] - r, p[1] - r, p[0] + r, p[1] + r), fill=255)

# 彩虹渐变层：沿勾的"行进方向"铺色带（绿→黄→橙→红→紫→蓝）
rainbow = [
    (52, 168, 83),    # 绿
    (140, 198, 63),   # 黄绿
    (251, 188, 5),    # 黄
    (255, 145, 40),   # 橙
    (234, 67, 53),    # 红
    (200, 60, 130),   # 品红
    (124, 92, 220),   # 紫
    (66, 133, 244),   # 蓝
]
grad = Image.new("RGBA", (W, W), (0, 0, 0, 0))
gd = ImageDraw.Draw(grad)
# 用沿对勾包围盒的对角方向投影做渐变参数
gx0, gy0 = p_start
gx1, gy1 = p_end
dx, dy = (gx1 - gx0), (gy1 - gy0)
length2 = dx * dx + dy * dy
# 逐像素太慢，改为铺一组垂直于行进方向的彩色条带
n = 400
for i in range(n):
    t = i / (n - 1)
    # 颜色插值
    fpos = t * (len(rainbow) - 1)
    lo = int(math.floor(fpos))
    hi = min(lo + 1, len(rainbow) - 1)
    f = fpos - lo
    c = tuple(int(rainbow[lo][k] * (1 - f) + rainbow[hi][k] * f) for k in range(3))
    # 该 t 对应的中心点，沿行进方向
    cx = gx0 + dx * t
    cy = gy0 + dy * t
    # 垂直方向画一条很宽的条带
    px, py = -dy, dx
    plen = math.hypot(px, py)
    px, py = px / plen, py / plen
    half = W  # 足够长盖满
    seg_w = (math.hypot(dx, dy) / n) * 1.8 + 2
    band = [
        (cx + px * half, cy + py * half),
        (cx - px * half, cy - py * half),
    ]
    gd.line(band, fill=c + (255,), width=int(seg_w) + 1)

# 把彩虹按勾的 mask 贴上去
img.paste(grad, (0, 0), mask)

# 勾内侧轻微阴影，增加立体感（在勾下方偏移一份暗 mask 模糊后压低）
shadow = Image.new("RGBA", (W, W), (0, 0, 0, 0))
sd = ImageDraw.Draw(shadow)
sd.line([p_start, p_corner, p_end], fill=(0, 0, 0, 70), width=stroke, joint="curve")
shadow = shadow.filter(ImageFilter.GaussianBlur(W * 0.012))
# 只保留落在白卡内、勾外侧的阴影
card_mask = Image.new("L", (W, W), 0)
ImageDraw.Draw(card_mask).rounded_rectangle(main_box, radius=corner, fill=255)
shadow.putalpha(Image.composite(shadow.getchannel("A"),
                                Image.new("L", (W, W), 0), card_mask))
base = Image.new("RGBA", (W, W), (0, 0, 0, 0))
base = Image.alpha_composite(base, img)

# 缩小回 1024（高质量抗锯齿）
out = base.resize((S, S), Image.LANCZOS)
out.save("icon_source.png")
print("wrote icon_source.png", out.size)
