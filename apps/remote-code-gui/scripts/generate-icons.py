#!/usr/bin/env python3
"""Generate the Remote Code icon system.

The source of truth is the vector geometry in this script. It emits app,
browser, PWA, Android, iOS-ready, and brand assets from the same mark so the
product never drifts across platforms.
"""

from __future__ import annotations

import io
import json
import math
import struct
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont


ROOT = Path(__file__).resolve().parents[1]
PUBLIC = ROOT / "public"
TAURI_ICONS = ROOT / "src-tauri" / "icons"
ANDROID_RES = ROOT / "src-tauri" / "android" / "app" / "src" / "main" / "res"
BRAND = ROOT / "assets" / "brand"

INK = (13, 19, 35)
INK_2 = (17, 24, 39)
BLUE = (37, 99, 235)
CYAN = (8, 145, 178)
MINT = (45, 212, 191)
WHITE = (248, 250, 252)
AMBER = (250, 204, 21)


def ensure_dirs() -> None:
    for path in [PUBLIC, TAURI_ICONS, BRAND, ANDROID_RES / "drawable", ANDROID_RES / "mipmap-anydpi-v26"]:
        path.mkdir(parents=True, exist_ok=True)
    for density in ["mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"]:
        (ANDROID_RES / f"mipmap-{density}").mkdir(parents=True, exist_ok=True)
    (TAURI_ICONS / "ios" / "AppIcon.appiconset").mkdir(parents=True, exist_ok=True)


def lerp(a: int, b: int, t: float) -> int:
    return round(a + (b - a) * t)


def mix(c1: tuple[int, int, int], c2: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    return tuple(lerp(c1[i], c2[i], t) for i in range(3))


def linear_gradient(size: int, start: tuple[int, int, int], end: tuple[int, int, int]) -> Image.Image:
    image = Image.new("RGB", (size, size), start)
    pixels = image.load()
    denom = max(1, (size - 1) * 2)
    for y in range(size):
        for x in range(size):
            t = (x + y) / denom
            pixels[x, y] = mix(start, end, t)
    return image.convert("RGBA")


def add_glow(base: Image.Image, center: tuple[float, float], color: tuple[int, int, int], radius: float, alpha: int) -> None:
    width, height = base.size
    glow = Image.new("RGBA", base.size, (0, 0, 0, 0))
    pixels = glow.load()
    cx, cy = center
    for y in range(height):
        for x in range(width):
            d = math.hypot(x - cx, y - cy) / radius
            if d < 1:
                a = round(alpha * (1 - d) ** 2)
                pixels[x, y] = (*color, a)
    base.alpha_composite(glow)


def draw_soft_grid(base: Image.Image) -> None:
    size = base.size[0]
    draw = ImageDraw.Draw(base, "RGBA")
    step = size // 8
    for i in range(1, 8):
        p = i * step
        draw.line((p, 0, p, size), fill=(255, 255, 255, 3), width=max(1, size // 512))
        draw.line((0, p, size, p), fill=(255, 255, 255, 2), width=max(1, size // 512))


def draw_round_line(
    draw: ImageDraw.ImageDraw,
    points: list[tuple[float, float]],
    fill: tuple[int, int, int, int],
    width: int,
) -> None:
    if len(points) < 2:
        return
    draw.line(points, fill=fill, width=width, joint="curve")
    r = width / 2
    for x, y in points:
        draw.ellipse((x - r, y - r, x + r, y + r), fill=fill)


def arc_points(size: int, scale: float) -> list[tuple[float, float]]:
    cx = cy = size / 2
    radius = size * 0.318 * scale
    start = 218
    end = 510
    steps = 104
    points: list[tuple[float, float]] = []
    for i in range(steps + 1):
        angle = math.radians(start + (end - start) * i / steps)
        points.append((cx + radius * math.cos(angle), cy + radius * math.sin(angle)))
    return points


def draw_arc_gradient(
    target: Image.Image,
    size: int,
    scale: float,
    alpha: int = 255,
    monochrome: bool = False,
) -> None:
    width = max(4, round(size * 0.088 * scale))
    points = arc_points(size, scale)
    mask = Image.new("L", target.size, 0)
    mask_draw = ImageDraw.Draw(mask)
    cx = cy = size / 2
    radius = size * 0.318 * scale
    mask_draw.arc(
        (cx - radius, cy - radius, cx + radius, cy + radius),
        start=218,
        end=510,
        fill=alpha,
        width=width,
    )
    cap_r = width / 2
    for x, y in (points[0], points[-1]):
        mask_draw.ellipse((x - cap_r, y - cap_r, x + cap_r, y + cap_r), fill=alpha)

    layer = Image.new("RGBA", target.size, (255, 255, 255, 0))
    pixels = layer.load()
    mask_pixels = mask.load()
    for y in range(size):
        for x in range(size):
            a = mask_pixels[x, y]
            if not a:
                continue
            if monochrome:
                color = WHITE
            else:
                t = min(1.0, max(0.0, (x * 0.55 + y * 0.45) / max(1, size)))
                color = mix(MINT, BLUE, t)
            pixels[x, y] = (*color, a)
    target.alpha_composite(layer)

    if not monochrome:
        draw = ImageDraw.Draw(target, "RGBA")
        for point, color in [(points[0], MINT), (points[-1], BLUE)]:
            x, y = point
            draw.ellipse((x - cap_r, y - cap_r, x + cap_r, y + cap_r), fill=(*color, alpha))


def draw_mark(
    image: Image.Image,
    scale: float = 1.0,
    shadow: bool = True,
    monochrome: bool = False,
) -> None:
    size = image.size[0]
    layer = Image.new("RGBA", image.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer, "RGBA")

    if shadow:
        shadow_layer = Image.new("RGBA", image.size, (0, 0, 0, 0))
        draw_arc_gradient(shadow_layer, size, scale, alpha=150, monochrome=True)
        shadow_layer = shadow_layer.filter(ImageFilter.GaussianBlur(max(4, size // 40)))
        image.alpha_composite(shadow_layer)

    draw_arc_gradient(layer, size, scale, monochrome=monochrome)

    w = max(4, round(size * 0.068 * scale))
    cx = cy = size / 2
    prompt = [
        (cx - size * 0.09 * scale, cy - size * 0.135 * scale),
        (cx + size * 0.045 * scale, cy),
        (cx - size * 0.09 * scale, cy + size * 0.135 * scale),
    ]
    draw_round_line(draw, prompt, (*WHITE, 255), w)
    cursor_color = WHITE if monochrome else AMBER
    draw_round_line(
        draw,
        [
            (cx + size * 0.092 * scale, cy + size * 0.132 * scale),
            (cx + size * 0.195 * scale, cy + size * 0.132 * scale),
        ],
        (*cursor_color, 255),
        max(4, round(w * 0.74)),
    )

    node_r = max(4, round(size * 0.035 * scale))
    for point, color in [
        ((cx - size * 0.285 * scale, cy - size * 0.075 * scale), CYAN),
        ((cx + size * 0.295 * scale, cy - size * 0.24 * scale), BLUE),
    ]:
        fill = WHITE if monochrome else color
        x, y = point
        draw.ellipse((x - node_r, y - node_r, x + node_r, y + node_r), fill=(*fill, 255))

    image.alpha_composite(layer)


def app_icon(size: int, scale: float = 1.0) -> Image.Image:
    image = linear_gradient(size, INK_2, (5, 14, 24))
    add_glow(image, (size * 0.78, size * 0.16), BLUE, size * 0.78, 88)
    add_glow(image, (size * 0.18, size * 0.78), CYAN, size * 0.70, 74)
    add_glow(image, (size * 0.72, size * 0.75), AMBER, size * 0.72, 28)
    draw_mark(image, scale=scale, shadow=True)
    return image.convert("RGB")


def transparent_mark(size: int, scale: float = 1.0, monochrome: bool = False) -> Image.Image:
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw_mark(image, scale=scale, shadow=False, monochrome=monochrome)
    return image


def save_png(path: Path, image: Image.Image) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, "PNG", optimize=True)


def save_resized(path: Path, image: Image.Image, size: tuple[int, int]) -> None:
    save_png(path, image.resize(size, Image.Resampling.LANCZOS))


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def png_bytes(image: Image.Image) -> bytes:
    buf = io.BytesIO()
    image.save(buf, "PNG", optimize=True)
    return buf.getvalue()


def save_icns(path: Path, source: Image.Image) -> None:
    chunks = [
        ("icp4", 16),
        ("icp5", 32),
        ("icp6", 64),
        ("ic07", 128),
        ("ic08", 256),
        ("ic09", 512),
        ("ic10", 1024),
    ]
    body = bytearray()
    for kind, size in chunks:
        data = png_bytes(source.resize((size, size), Image.Resampling.LANCZOS))
        body += kind.encode("ascii")
        body += struct.pack(">I", len(data) + 8)
        body += data
    path.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)


def save_ico(path: Path, source: Image.Image) -> None:
    source.save(path, format="ICO", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])


def app_icon_svg() -> str:
    return """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>
    <linearGradient id="bg" x1="120" y1="80" x2="904" y2="944" gradientUnits="userSpaceOnUse">
      <stop stop-color="#111827"/>
      <stop offset="1" stop-color="#05111a"/>
    </linearGradient>
    <radialGradient id="blueGlow" cx="0" cy="0" r="1" gradientTransform="matrix(520 0 0 520 800 160)" gradientUnits="userSpaceOnUse">
      <stop stop-color="#2563eb" stop-opacity=".48"/>
      <stop offset="1" stop-color="#2563eb" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="cyanGlow" cx="0" cy="0" r="1" gradientTransform="matrix(500 0 0 500 180 790)" gradientUnits="userSpaceOnUse">
      <stop stop-color="#0891b2" stop-opacity=".42"/>
      <stop offset="1" stop-color="#0891b2" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="link" x1="220" y1="760" x2="820" y2="230" gradientUnits="userSpaceOnUse">
      <stop stop-color="#2dd4bf"/>
      <stop offset=".55" stop-color="#0891b2"/>
      <stop offset="1" stop-color="#2563eb"/>
    </linearGradient>
    <filter id="softShadow" x="-30%" y="-30%" width="160%" height="160%" color-interpolation-filters="sRGB">
      <feDropShadow dx="0" dy="24" stdDeviation="34" flood-color="#020617" flood-opacity=".42"/>
    </filter>
  </defs>
  <rect width="1024" height="1024" fill="url(#bg)"/>
  <rect width="1024" height="1024" fill="url(#blueGlow)"/>
  <rect width="1024" height="1024" fill="url(#cyanGlow)"/>
  <g opacity=".08" stroke="#fff" stroke-width="2">
    <path d="M128 0v1024M256 0v1024M384 0v1024M512 0v1024M640 0v1024M768 0v1024M896 0v1024"/>
    <path d="M0 128h1024M0 256h1024M0 384h1024M0 512h1024M0 640h1024M0 768h1024M0 896h1024"/>
  </g>
  <g filter="url(#softShadow)">
    <path d="M251 318c130-167 386-183 535-44 166 156 149 445-34 574-161 114-392 80-514-80" fill="none" stroke="url(#link)" stroke-width="90" stroke-linecap="round"/>
    <path d="M420 374 558 512 420 650" fill="none" stroke="#f8fafc" stroke-width="70" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M606 648h104" fill="none" stroke="#facc15" stroke-width="52" stroke-linecap="round"/>
    <circle cx="220" cy="468" r="36" fill="#0891b2"/>
    <circle cx="704" cy="266" r="36" fill="#2563eb"/>
  </g>
</svg>
"""


def mark_svg(monochrome: bool = False) -> str:
    arc = "#f8fafc" if monochrome else "url(#link)"
    cursor = "#f8fafc" if monochrome else "#facc15"
    defs = "" if monochrome else """
  <defs>
    <linearGradient id="link" x1="220" y1="760" x2="820" y2="230" gradientUnits="userSpaceOnUse">
      <stop stop-color="#2dd4bf"/>
      <stop offset=".55" stop-color="#0891b2"/>
      <stop offset="1" stop-color="#2563eb"/>
    </linearGradient>
  </defs>"""
    node_a = "#f8fafc" if monochrome else "#0891b2"
    node_b = "#f8fafc" if monochrome else "#2563eb"
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
{defs}
  <path d="M251 318c130-167 386-183 535-44 166 156 149 445-34 574-161 114-392 80-514-80" fill="none" stroke="{arc}" stroke-width="90" stroke-linecap="round"/>
  <path d="M420 374 558 512 420 650" fill="none" stroke="#f8fafc" stroke-width="70" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M606 648h104" fill="none" stroke="{cursor}" stroke-width="52" stroke-linecap="round"/>
  <circle cx="220" cy="468" r="36" fill="{node_a}"/>
  <circle cx="704" cy="266" r="36" fill="{node_b}"/>
</svg>
"""


def wordmark_svg() -> str:
    return """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 980 220">
  <rect width="980" height="220" rx="40" fill="#0f172a"/>
  <g transform="translate(42 -402) scale(.62)">
    <path d="M251 318c130-167 386-183 535-44 166 156 149 445-34 574-161 114-392 80-514-80" fill="none" stroke="#2dd4bf" stroke-width="90" stroke-linecap="round"/>
    <path d="M420 374 558 512 420 650" fill="none" stroke="#f8fafc" stroke-width="70" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M606 648h104" fill="none" stroke="#facc15" stroke-width="52" stroke-linecap="round"/>
    <circle cx="220" cy="468" r="36" fill="#0891b2"/>
    <circle cx="704" cy="266" r="36" fill="#2563eb"/>
  </g>
  <text x="250" y="118" fill="#f8fafc" font-family="Segoe UI, Inter, Arial, sans-serif" font-size="66" font-weight="750">Remote Code</text>
  <text x="253" y="158" fill="#94a3b8" font-family="Segoe UI, Inter, Arial, sans-serif" font-size="24" font-weight="500">local AI coding, controlled remotely</text>
</svg>
"""


def android_xml() -> None:
    adaptive = """<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/icon_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
    <monochrome android:drawable="@drawable/ic_launcher_monochrome" />
</adaptive-icon>
"""
    write_text(ANDROID_RES / "mipmap-anydpi-v26" / "ic_launcher.xml", adaptive)
    write_text(ANDROID_RES / "mipmap-anydpi-v26" / "ic_launcher_round.xml", adaptive)
    monochrome = """<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="1024"
    android:viewportHeight="1024">
    <path
        android:pathData="M251,318 C381,151 637,135 786,274 C952,430 935,719 752,848 C591,962 360,928 238,768"
        android:fillColor="@android:color/transparent"
        android:strokeColor="#FFFFFFFF"
        android:strokeWidth="90"
        android:strokeLineCap="round" />
    <path
        android:pathData="M420,374 L558,512 L420,650"
        android:fillColor="@android:color/transparent"
        android:strokeColor="#FFFFFFFF"
        android:strokeWidth="70"
        android:strokeLineCap="round"
        android:strokeLineJoin="round" />
    <path
        android:pathData="M606,648 L710,648"
        android:fillColor="@android:color/transparent"
        android:strokeColor="#FFFFFFFF"
        android:strokeWidth="52"
        android:strokeLineCap="round" />
    <path android:fillColor="#FFFFFFFF" android:pathData="M220,468 m-36,0 a36,36 0,1,0 72,0 a36,36 0,1,0 -72,0" />
    <path android:fillColor="#FFFFFFFF" android:pathData="M704,266 m-36,0 a36,36 0,1,0 72,0 a36,36 0,1,0 -72,0" />
</vector>
"""
    write_text(ANDROID_RES / "drawable" / "ic_launcher_monochrome.xml", monochrome)
    stat = """<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="1024"
    android:viewportHeight="1024">
    <path
        android:pathData="M288,330 C413,188 638,184 772,310 C920,449 904,700 742,814 C594,918 386,888 274,746"
        android:fillColor="@android:color/transparent"
        android:strokeColor="#FFFFFFFF"
        android:strokeWidth="96"
        android:strokeLineCap="round" />
    <path
        android:pathData="M422,380 L556,512 L422,644"
        android:fillColor="@android:color/transparent"
        android:strokeColor="#FFFFFFFF"
        android:strokeWidth="88"
        android:strokeLineCap="round"
        android:strokeLineJoin="round" />
    <path
        android:pathData="M614,648 L720,648"
        android:fillColor="@android:color/transparent"
        android:strokeColor="#FFFFFFFF"
        android:strokeWidth="62"
        android:strokeLineCap="round" />
</vector>
"""
    write_text(ANDROID_RES / "drawable" / "ic_stat_remote_code.xml", stat)


def draw_tile(width: int, height: int) -> Image.Image:
    bg = Image.new("RGBA", (width, height), INK_2 + (255,))
    add_glow(bg, (width * 0.84, height * 0.16), BLUE, max(width, height) * 0.8, 90)
    add_glow(bg, (width * 0.12, height * 0.8), CYAN, max(width, height) * 0.7, 78)
    mark_size = min(width, height)
    mark = transparent_mark(mark_size, scale=0.74)
    bg.alpha_composite(mark, ((width - mark_size) // 2, (height - mark_size) // 2))
    return bg.convert("RGB")


def load_font(size: int, bold: bool = False) -> ImageFont.ImageFont:
    candidates = [
        Path("C:/Windows/Fonts/segoeuib.ttf" if bold else "C:/Windows/Fonts/segoeui.ttf"),
        Path("C:/Windows/Fonts/arialbd.ttf" if bold else "C:/Windows/Fonts/arial.ttf"),
    ]
    for path in candidates:
        if path.exists():
            return ImageFont.truetype(str(path), size)
    return ImageFont.load_default()


def social_image() -> Image.Image:
    width, height = 1200, 630
    image = Image.new("RGBA", (width, height), INK_2 + (255,))
    add_glow(image, (width * 0.84, height * 0.10), BLUE, 720, 105)
    add_glow(image, (width * 0.15, height * 0.88), CYAN, 690, 90)
    draw = ImageDraw.Draw(image, "RGBA")
    for x in range(0, width, 80):
        draw.line((x, 0, x, height), fill=(255, 255, 255, 8), width=1)
    for y in range(0, height, 80):
        draw.line((0, y, width, y), fill=(255, 255, 255, 7), width=1)
    icon = app_icon(260).resize((220, 220), Image.Resampling.LANCZOS)
    image.alpha_composite(icon.convert("RGBA"), (92, 200))
    title_font = load_font(78, bold=True)
    sub_font = load_font(32)
    draw.text((360, 214), "Remote Code", fill=WHITE + (255,), font=title_font)
    draw.text(
        (364, 318),
        "Local AI coding, securely controlled from desktop, web, and mobile.",
        fill=(203, 213, 225, 255),
        font=sub_font,
    )
    draw.rounded_rectangle((364, 392, 760, 448), radius=22, fill=(15, 23, 42, 210), outline=(45, 212, 191, 120), width=2)
    draw.text((392, 406), "> approve, stream, ship", fill=(45, 212, 191, 255), font=load_font(24, bold=True))
    return image.convert("RGB")


def ios_app_icon_set(source: Image.Image) -> None:
    appicon = TAURI_ICONS / "ios" / "AppIcon.appiconset"
    entries: list[dict[str, str]] = []

    def add(filename: str, idiom: str, size_pt: str, scale: str, pixels: int) -> None:
        save_resized(appicon / filename, source, (pixels, pixels))
        entries.append({"size": size_pt, "idiom": idiom, "filename": filename, "scale": scale})

    add("Icon-App-20x20@2x.png", "iphone", "20x20", "2x", 40)
    add("Icon-App-20x20@3x.png", "iphone", "20x20", "3x", 60)
    add("Icon-App-29x29@2x.png", "iphone", "29x29", "2x", 58)
    add("Icon-App-29x29@3x.png", "iphone", "29x29", "3x", 87)
    add("Icon-App-40x40@2x.png", "iphone", "40x40", "2x", 80)
    add("Icon-App-40x40@3x.png", "iphone", "40x40", "3x", 120)
    add("Icon-App-60x60@2x.png", "iphone", "60x60", "2x", 120)
    add("Icon-App-60x60@3x.png", "iphone", "60x60", "3x", 180)
    add("Icon-App-20x20@1x~ipad.png", "ipad", "20x20", "1x", 20)
    add("Icon-App-20x20@2x~ipad.png", "ipad", "20x20", "2x", 40)
    add("Icon-App-29x29@1x~ipad.png", "ipad", "29x29", "1x", 29)
    add("Icon-App-29x29@2x~ipad.png", "ipad", "29x29", "2x", 58)
    add("Icon-App-40x40@1x~ipad.png", "ipad", "40x40", "1x", 40)
    add("Icon-App-40x40@2x~ipad.png", "ipad", "40x40", "2x", 80)
    add("Icon-App-76x76@1x~ipad.png", "ipad", "76x76", "1x", 76)
    add("Icon-App-76x76@2x~ipad.png", "ipad", "76x76", "2x", 152)
    add("Icon-App-83.5x83.5@2x~ipad.png", "ipad", "83.5x83.5", "2x", 167)
    add("Icon-App-1024x1024@1x.png", "ios-marketing", "1024x1024", "1x", 1024)
    write_text(appicon / "Contents.json", json.dumps({"images": entries, "info": {"version": 1, "author": "remote-code"}}, indent=2) + "\n")


def main() -> None:
    ensure_dirs()
    source = app_icon(1024)
    maskable = app_icon(1024, scale=0.84)
    mark = transparent_mark(1024, scale=1.0)
    mono = transparent_mark(1024, scale=1.0, monochrome=True)

    save_png(BRAND / "app-icon-master-1024.png", source)
    save_png(BRAND / "app-icon-maskable-1024.png", maskable)
    save_png(BRAND / "mark-1024.png", mark)
    save_png(BRAND / "mark-monochrome-1024.png", mono)
    write_text(BRAND / "app-icon-master.svg", app_icon_svg())
    write_text(BRAND / "mark.svg", mark_svg())
    write_text(BRAND / "mark-monochrome.svg", mark_svg(monochrome=True))
    write_text(BRAND / "wordmark.svg", wordmark_svg())

    save_resized(TAURI_ICONS / "32x32.png", source, (32, 32))
    save_resized(TAURI_ICONS / "128x128.png", source, (128, 128))
    save_resized(TAURI_ICONS / "128x128@2x.png", source, (256, 256))
    save_resized(TAURI_ICONS / "icon.png", source, (512, 512))
    save_ico(TAURI_ICONS / "icon.ico", source)
    save_icns(TAURI_ICONS / "icon.icns", source)

    for name, size in [
        ("Square30x30Logo.png", 30),
        ("Square44x44Logo.png", 44),
        ("StoreLogo.png", 50),
        ("Square71x71Logo.png", 71),
        ("Square89x89Logo.png", 89),
        ("Square107x107Logo.png", 107),
        ("Square142x142Logo.png", 142),
        ("Square150x150Logo.png", 150),
        ("Square284x284Logo.png", 284),
        ("Square310x310Logo.png", 310),
    ]:
        save_resized(TAURI_ICONS / name, source, (size, size))
    save_png(TAURI_ICONS / "Wide310x150Logo.png", draw_tile(310, 150))
    save_png(TAURI_ICONS / "SplashScreen.png", draw_tile(620, 300))

    write_text(PUBLIC / "favicon.svg", app_icon_svg())
    write_text(PUBLIC / "brand-mark.svg", mark_svg())
    write_text(PUBLIC / "pwa-monochrome.svg", mark_svg(monochrome=True))
    save_ico(PUBLIC / "favicon.ico", source)
    save_resized(PUBLIC / "favicon-16x16.png", source, (16, 16))
    save_resized(PUBLIC / "favicon-32x32.png", source, (32, 32))
    save_resized(PUBLIC / "apple-touch-icon.png", source, (180, 180))
    save_resized(PUBLIC / "pwa-icon.png", source, (512, 512))
    save_resized(PUBLIC / "pwa-icon-192.png", source, (192, 192))
    save_resized(PUBLIC / "pwa-icon-512.png", source, (512, 512))
    save_resized(PUBLIC / "pwa-maskable-192.png", maskable, (192, 192))
    save_resized(PUBLIC / "pwa-maskable-512.png", maskable, (512, 512))
    save_png(PUBLIC / "og-image.png", social_image())

    android_xml()
    launcher_sizes = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
    foreground_sizes = {"mdpi": 108, "hdpi": 162, "xhdpi": 216, "xxhdpi": 324, "xxxhdpi": 432}
    foreground = transparent_mark(1024, scale=0.84)
    round_source = app_icon(1024, scale=0.88)
    for density, px in launcher_sizes.items():
        out_dir = ANDROID_RES / f"mipmap-{density}"
        save_resized(out_dir / "ic_launcher.png", source, (px, px))
        save_resized(out_dir / "ic_launcher_round.png", round_source, (px, px))
    for density, px in foreground_sizes.items():
        out_dir = ANDROID_RES / f"mipmap-{density}"
        save_resized(out_dir / "ic_launcher_foreground.png", foreground, (px, px))

    ios_app_icon_set(source)


if __name__ == "__main__":
    main()
