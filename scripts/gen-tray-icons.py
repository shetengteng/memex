#!/usr/bin/env python3
"""
生成 macOS menubar tray icons (template image: 纯黑 + alpha)。

输出:
  tauri-app/src-tauri/icons/tray-16x16.png   (1x)
  tauri-app/src-tauri/icons/tray-22x22.png   (1x default)
  tauri-app/src-tauri/icons/tray-32x32.png   (2x retina)
  tauri-app/src-tauri/icons/tray-64x64.png   (4x super-retina)
  tauri-app/src-tauri/icons/tray-icon.png    -> 22x22 alias

笔画做粗，字母 M 加大占满 padding，保证 22x22 也清晰可读。
"""
from PIL import Image, ImageDraw, ImageFont
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "tauri-app" / "src-tauri" / "icons"


def find_bold_font():
    candidates = [
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/SFCompactDisplay-Bold.otf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/Library/Fonts/Arial Bold.ttf",
    ]
    for c in candidates:
        if Path(c).exists():
            return c
    return None


FONT_PATH = find_bold_font()


def draw_m(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # font_size 取 ~0.78 * size 保证字母 M 撑满，padding 大约 11%。
    font_size = int(size * 0.82)
    if FONT_PATH:
        try:
            font = ImageFont.truetype(FONT_PATH, font_size, index=1)  # bold variant
        except OSError:
            font = ImageFont.truetype(FONT_PATH, font_size)
    else:
        font = ImageFont.load_default()

    # 文本测量+居中
    text = "M"
    bbox = draw.textbbox((0, 0), text, font=font, anchor="lt")
    text_w = bbox[2] - bbox[0]
    text_h = bbox[3] - bbox[1]
    # 字形的 baseline 偏移很微妙，用 bbox 居中
    x = (size - text_w) / 2 - bbox[0]
    y = (size - text_h) / 2 - bbox[1]

    draw.text((x, y), text, fill=(0, 0, 0, 255), font=font)
    return img


def main():
    sizes = [16, 22, 32, 64]
    for s in sizes:
        out = ICONS / f"tray-{s}x{s}.png"
        img = draw_m(s)
        img.save(out, "PNG")
        print(f"wrote {out} ({s}x{s})")

    # 默认 alias (Tauri 默认拿 tray-icon.png)
    alias = ICONS / "tray-icon.png"
    draw_m(22).save(alias, "PNG")
    print(f"wrote {alias} (alias of tray-22x22)")


if __name__ == "__main__":
    main()
