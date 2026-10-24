"""'뿌' 예비 아이콘 생성 (doc/issues/09-ppu-icon-candidate.md).

'나' 원본(key2gksrmf-na-source.png)의 글꼴을 알 수 없어, '나' 획(두께 약 24px@256,
꺾인 머리, 둥근 끝)에 맞춰 획을 직접 그린다. 2048px로 그린 뒤 축소한다.

    python -I assets/icons/candidate-ppu/make_ppu_icon.py [비교이미지.png]

출력은 이 폴더에 실제 파일과 같은 이름으로 만든다.
"""
import io
import math
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
S = 8
N = 256 * S
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]


def stroke(d, pts, w, head=1.35, tail=0.95):
    """256 좌표 꺾은선을 원을 이어 찍어 그린다. 폭은 앞 25%에서 head→1, 끝 25%에서 1→tail."""
    segs = list(zip(pts, pts[1:]))
    total = sum(math.dist(a, b) for a, b in segs)
    acc = 0
    for a, b in segs:
        seg = math.dist(a, b)
        n = max(2, int(seg * S / 2))
        for i in range(n + 1):
            t = i / n
            x = (a[0] + (b[0] - a[0]) * t) * S
            y = (a[1] + (b[1] - a[1]) * t) * S
            u = (acc + seg * t) / total
            if u < 0.25:
                f = head + (1 - head) * u / 0.25
            else:
                f = 1 + (tail - 1) * max(0, (u - 0.75) / 0.25)
            r = w * f * S / 2
            d.ellipse((x - r, y - r, x + r, y + r), fill=255)
        acc += seg


def draw_mask():
    mask = Image.new("L", (N, N), 0)
    d = ImageDraw.Draw(mask)
    wb = 18
    for ox in (0, 108):  # ㅃ = ㅂ 두 개
        stroke(d, [(22 + ox, 38), (30 + ox, 41), (33 + ox, 118)], wb, head=1.45)
        stroke(d, [(94 + ox, 34), (100 + ox, 37), (101 + ox, 120)], wb, head=1.45)
        stroke(d, [(33 + ox, 78), (100 + ox, 75)], wb * 0.85, head=1.0)
        stroke(d, [(33 + ox, 118), (102 + ox, 115)], wb, head=1.15)
    stroke(d, [(22, 160), (128, 154), (234, 149)], 24, head=1.45)  # ㅜ 가로
    stroke(d, [(128, 155), (129, 224)], 24, head=1.0, tail=0.9)  # ㅜ 세로
    return mask


def colored(mask, rgb):
    im = Image.new("RGBA", mask.size, rgb + (0,))
    im.putalpha(mask)
    return im


def write_ico(im, path):
    frames = []
    for s in ICO_SIZES:
        b = io.BytesIO()
        im.resize((s, s), Image.LANCZOS).save(b, "PNG", optimize=True)
        frames.append(b.getvalue())
    out = struct.pack("<HHH", 0, 1, len(frames))
    off = 6 + 16 * len(frames)
    for s, f in zip(ICO_SIZES, frames):
        out += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(f), off)
        off += len(f)
    path.write_bytes(out + b"".join(frames))


def write_og(mask, path):
    # 현재 og-image.png(512, 흰 배경)의 '나'처럼 글자 폭 약 354px, 가운데 정렬
    glyph = colored(mask, (0, 0, 0)).crop(mask.getbbox())
    k = 354 / glyph.width
    glyph = glyph.resize((round(glyph.width * k), round(glyph.height * k)), Image.LANCZOS)
    og = Image.new("RGBA", (512, 512), (255, 255, 255, 255))
    og.alpha_composite(glyph, ((512 - glyph.width) // 2, (512 - glyph.height) // 2))
    og.convert("RGB").save(path, optimize=True)


def write_compare(path):
    na = [Image.open(ROOT / "doc/images" / n).convert("RGBA") for n in ("app-icon.png", "app-icon-dark.png")]
    pu = [Image.open(OUT / n).convert("RGBA") for n in ("app-icon.png", "app-icon-dark.png")]
    sheet = Image.new("RGBA", (2 * (256 + 48 + 32 + 16 + 70) + 40, 540), (255, 255, 255, 255))
    sheet.paste((32, 32, 32, 255), (0, 270, sheet.width, 540))
    for row in range(2):
        x = 10
        for g in (na[row], pu[row]):
            for s in (256, 48, 32, 16):
                sheet.alpha_composite(g.resize((s, s), Image.LANCZOS), (x, row * 270 + 7 + (256 - s) // 2))
                x += s + 14
            x += 20
    sheet.save(path)


def main():
    mask = draw_mask()
    colored(mask, (0, 0, 0)).save(OUT / "key2gksrmf-ppu-source.png", optimize=True)
    for name, rgb, png in (("light", (0, 0, 0), "app-icon.png"), ("dark", (255, 255, 255), "app-icon-dark.png")):
        im = colored(mask, rgb)
        write_ico(im, OUT / f"key2gksrmf_{name}.ico")
        im.resize((256, 256), Image.LANCZOS).save(OUT / png, optimize=True)
    write_og(mask, OUT / "og-image.png")
    if len(sys.argv) > 1:
        write_compare(Path(sys.argv[1]))


if __name__ == "__main__":
    main()
