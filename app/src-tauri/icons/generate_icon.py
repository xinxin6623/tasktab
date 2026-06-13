#!/usr/bin/env python3
"""生成 TaskTab 应用图标源图（1024×1024 透明 PNG）。

以项目根 `TaskTab应用macOS风格Logo设计.png` 为设计参考，用代码重画一个干净版：
青绿渐变磨砂玻璃圆角方（squircle）+ 任务清单（4 个复选框 + 横线）+ 白色对勾。
矢量绘制无压缩伪影，比直接裁 AI 原图更锐利。

依赖仅 Pillow。生成后交给 `tauri icon icon_source.png` 切全套尺寸。
"""
import math
from PIL import Image, ImageDraw, ImageFilter

S = 1024
SS = 2                       # 超采样
W = S * SS

img = Image.new("RGBA", (W, W), (0, 0, 0, 0))


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


# ── 玻璃方几何（macOS squircle，内容约占 84%）────────────────────────
margin = int(W * 0.085)
box = (margin, margin, W - margin, W - margin)
bw = box[2] - box[0]
corner = int(bw * 0.235)


def rounded_mask(size, rad):
    m = Image.new("L", size, 0)
    ImageDraw.Draw(m).rounded_rectangle((0, 0, size[0] - 1, size[1] - 1), radius=rad, fill=255)
    return m


# ── 1) 玻璃主体：青绿对角渐变（左上浅青 → 右下青绿）────────────────
grad = Image.new("RGBA", (bw, bw), (0, 0, 0, 0))
top_l = (214, 240, 236)      # 左上：近白浅青
mid = (150, 222, 214)        # 中：薄荷
bot_r = (90, 200, 196)       # 右下：青绿
px = grad.load()
for y in range(bw):
    for x in range(bw):
        t = (x + y) / (2 * bw)          # 对角参数 0..1
        if t < 0.5:
            c = lerp(top_l, mid, t / 0.5)
        else:
            c = lerp(mid, bot_r, (t - 0.5) / 0.5)
        px[x, y] = c + (255,)
gmask = rounded_mask((bw, bw), corner)
grad.putalpha(gmask)
img.paste(grad, (box[0], box[1]), grad)

# ── 2) 顶部高光弧 + 整体内发光，做出磨砂玻璃质感 ────────────────────
gloss = Image.new("RGBA", (W, W), (0, 0, 0, 0))
gd = ImageDraw.Draw(gloss)
# 顶部高光：偏上的大椭圆白雾
gd.ellipse((box[0] + bw * 0.08, box[1] - bw * 0.30,
            box[2] - bw * 0.08, box[1] + bw * 0.42),
           fill=(255, 255, 255, 90))
gloss = gloss.filter(ImageFilter.GaussianBlur(W * 0.02))
# 裁进玻璃方圆角内
clip = Image.new("L", (W, W), 0)
ImageDraw.Draw(clip).rounded_rectangle(box, radius=corner, fill=255)
gloss.putalpha(Image.composite(gloss.getchannel("A"), Image.new("L", (W, W), 0), clip))
img = Image.alpha_composite(img, gloss)

# ── 3) 任务清单内容（白色半透明）─────────────────────────────────
d = ImageDraw.Draw(img)
WHITE = (255, 255, 255, 235)

# 内容区域（玻璃方内再留白）
cx0 = box[0] + bw * 0.20
cx1 = box[2] - bw * 0.16
inner_top = box[1] + bw * 0.26
row_gap = bw * 0.165
rows = 4

box_sz = bw * 0.085            # 复选框边长
box_rad = int(box_sz * 0.32)
line_h = bw * 0.052            # 横线粗细
line_x0 = cx0 + box_sz + bw * 0.075

for i in range(rows):
    cy = inner_top + i * row_gap
    # 复选框
    bx0, by0 = cx0, cy - box_sz / 2
    d.rounded_rectangle((bx0, by0, bx0 + box_sz, by0 + box_sz),
                        radius=box_rad, fill=WHITE)
    # 横线（最后一行短一点，给对勾让位）
    lx1 = cx1 if i < rows - 1 else cx0 + bw * 0.30
    d.rounded_rectangle((line_x0, cy - line_h / 2, lx1, cy + line_h / 2),
                        radius=int(line_h / 2), fill=WHITE)

# ── 4) 右下角大对勾（白色，略带阴影立体感）──────────────────────────
ck_w = int(bw * 0.052)         # 勾粗细
p_a = (cx1 - bw * 0.30, inner_top + 3 * row_gap - bw * 0.02)
p_b = (cx1 - bw * 0.205, inner_top + 3 * row_gap + bw * 0.075)
p_c = (cx1 - bw * 0.02, inner_top + 3 * row_gap - bw * 0.105)

# 阴影
sh = Image.new("RGBA", (W, W), (0, 0, 0, 0))
sd = ImageDraw.Draw(sh)
sd.line([p_a, p_b, p_c], fill=(20, 90, 90, 110), width=ck_w, joint="curve")
sh = sh.filter(ImageFilter.GaussianBlur(W * 0.006))
sh.putalpha(Image.composite(sh.getchannel("A"), Image.new("L", (W, W), 0), clip))
img = Image.alpha_composite(img, sh)

d = ImageDraw.Draw(img)
d.line([p_a, p_b, p_c], fill=(255, 255, 255, 255), width=ck_w, joint="curve")
for p in (p_a, p_b, p_c):
    r = ck_w // 2
    d.ellipse((p[0] - r, p[1] - r, p[0] + r, p[1] + r), fill=(255, 255, 255, 255))

# ── 5) 玻璃方外发光柔影（贴合 macOS 立体感）──────────────────────────
out = Image.new("RGBA", (W, W), (0, 0, 0, 0))
shadow = Image.new("RGBA", (W, W), (0, 0, 0, 0))
ImageDraw.Draw(shadow).rounded_rectangle(
    (box[0], box[1] + bw * 0.02, box[2], box[2] + bw * 0.02),
    radius=corner, fill=(60, 130, 130, 70))
shadow = shadow.filter(ImageFilter.GaussianBlur(W * 0.018))
out = Image.alpha_composite(out, shadow)
out = Image.alpha_composite(out, img)

out.resize((S, S), Image.LANCZOS).save("icon_source.png")
print("wrote icon_source.png", (S, S))
