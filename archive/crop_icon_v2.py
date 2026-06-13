#!/usr/bin/env python3
"""从 James 提供的 macOS 风格 Logo 裁出干净的应用图标源图。

输入：项目根 `TaskTab应用macOS风格Logo设计.png`（2048×2048，豆包 AI 生成）。
原图中间是青绿磨砂玻璃圆角方（任务清单 + 打勾），底部有 "TaskTab" 文字、
右下角有"豆包AI生成"水印——都不能进图标。

处理：裁出玻璃方包围盒 → 套 macOS squircle 圆角 alpha mask（去掉圆角外背景、
排除底部文字带）→ 居中到透明正方画布留白 → 缩到 1024 存 icon_source.png。
之后交给 `tauri icon icon_source.png` 切全套尺寸 + .icns/.ico。

依赖仅 Pillow。如需重切，按需微调 L/T/R/B 与 rad。
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[3]
SRC = ROOT / "TaskTab应用macOS风格Logo设计.png"
OUT = Path(__file__).resolve().parent / "icon_source.png"

# 玻璃方在 2048 画布里的包围盒（实测；B 收到文字带之前以排除 "TaskTab" 文字）
L, T, R, B = 358, 318, 1698, 1800

im = Image.open(SRC).convert("RGBA")
glass = im.crop((L, T, R, B)).convert("RGBA")
gw, gh = glass.size

# macOS squircle 圆角（按短边 ~24.5%），轻模糊让边缘顺滑
rad = int(min(gw, gh) * 0.245)
mask = Image.new("L", (gw, gh), 0)
ImageDraw.Draw(mask).rounded_rectangle((0, 0, gw - 1, gh - 1), radius=rad, fill=255)
mask = mask.filter(ImageFilter.GaussianBlur(3))
glass.putalpha(mask)

# 居中到透明正方画布，四周留白（macOS 图标内容约占 87%）
side = int(max(gw, gh) * 1.15)
canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
canvas.paste(glass, ((side - gw) // 2, (side - gh) // 2), glass)
canvas.resize((1024, 1024), Image.LANCZOS).save(OUT)
print("wrote", OUT, "(1024x1024) from glass", glass.size)
