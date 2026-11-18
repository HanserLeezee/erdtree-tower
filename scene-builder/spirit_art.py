"""自制骨灰的专属图标：把 spirit-art/ 里 3 列 × 3 行的图集切成 160×160 图标。

用法：python scene-builder/spirit_art.py scene-assets
custom-spirit-definitions.tsv 里不带 capture 标记的行用 spirit-art/ash-1.png … ash-N.png，带 capture 标记的（Boss 骨灰）用 bossash-1.png …，
各自按定义表顺序从左到右、从上到下，每张 9 个，最后一张不满时其余格子留空（透明背景）。
图标编号 = 65300 + 定义表的行号（与游戏菜单图标编号一致，见 SpiritIconAssets.cs），写出 scene-assets/reward-icons/{编号}.rgba，
并把 reward-details.tsv 里这些骨灰的图标列改成对应编号。透明度不超过 HAZE（40）的像素先清成完全透明（接口常在形象周围留一层半透明的雾或杂色）。
"""
from pathlib import Path
import sys
from PIL import Image

FIRST, SIZE, PER_SHEET = 65300, 160, 9
# 生成图在形象周围常带一层透明度 5～40 的雾（有时连成一片），切图前清成完全透明；形象本身基本不透明
HAZE = 40
assets = Path(sys.argv[1])
defs = [l.split('|') for l in (assets / 'custom-spirit-definitions.tsv').read_text(encoding='utf-8').splitlines() if l.strip()]
def main_body(tile):
    """去掉不连着主体的小碎块（相邻格子的形象伸过格子边界时留下的）：保留面积不小于最大连通块 3% 的部分"""
    w, h = tile.size
    alpha = tile.getchannel('A').load()
    seen = bytearray(w * h)
    parts = []
    for sy in range(h):
        for sx in range(w):
            i = sy * w + sx
            if seen[i] or alpha[sx, sy] == 0:
                continue
            seen[i] = 1
            stack, part = [(sx, sy)], []
            while stack:
                x, y = stack.pop()
                part.append((x, y))
                for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                    if 0 <= nx < w and 0 <= ny < h and not seen[ny * w + nx] and alpha[nx, ny] > 0:
                        seen[ny * w + nx] = 1
                        stack.append((nx, ny))
            parts.append(part)
    if not parts:
        return tile
    largest = max(len(p) for p in parts)
    out = tile.copy()
    px = out.load()
    for part in parts:
        if len(part) < largest * 0.03:
            for x, y in part:
                px[x, y] = (0, 0, 0, 0)
    return out


def is_capture(p):
    return len(p) > 9 and 'capture' in p[9].split(',')
groups = [('ash', [(row, p) for row, p in enumerate(defs) if not is_capture(p)]),
          ('bossash', [(row, p) for row, p in enumerate(defs) if is_capture(p)])]
icons = {}
cache = {}
for prefix, members in groups:
  sheets = sorted((assets / 'spirit-art').glob(f'{prefix}-*.png'), key=lambda p: int(p.stem.split('-')[1]))
  if len(sheets) * PER_SHEET < len(members):
    sys.exit(f'{prefix}：需要 {len(members)} 个图标，只有 {len(sheets)} 张图集')
  for n, (row, spec) in enumerate(members):
    sheet = sheets[n // PER_SHEET]
    if sheet not in cache:
        with Image.open(sheet) as image:
            atlas = image.convert('RGBA')
        px = atlas.load()
        for y in range(atlas.height):
            for x in range(atlas.width):
                if px[x, y][3] <= HAZE:
                    px[x, y] = (0, 0, 0, 0)
        cache[sheet] = atlas
    atlas = cache[sheet]
    if True:
        cell = n % PER_SHEET
        w, h = atlas.width / 3, atlas.height / 3
        tile = atlas.crop((round(cell % 3 * w), round(cell // 3 * h), round((cell % 3 + 1) * w), round((cell // 3 + 1) * h)))
    tile = main_body(tile)
    bounds = tile.getbbox()
    if bounds is None:
        sys.exit(f'{sheet.name} 第 {cell + 1} 格（{spec[7]}）是空的')
    tile = tile.crop(bounds)
    side = max(tile.size)
    square = Image.new('RGBA', (side, side), (0, 0, 0, 0))
    square.paste(tile, ((side - tile.width) // 2, (side - tile.height) // 2))
    icon = square.resize((SIZE - 8, SIZE - 8), Image.Resampling.LANCZOS)
    canvas = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))
    canvas.paste(icon, (4, 4))
    number = FIRST + row
    (assets / 'reward-icons' / f'{number}.rgba').write_bytes(canvas.tobytes())
    canvas.save(assets / 'spirit-art' / f'{number}.png')
    icons[0x40000000 | int(spec[0])] = number
details = assets / 'reward-details.tsv'
lines = details.read_text(encoding='utf-8').split('\n')
changed = 0
for i, line in enumerate(lines):
    p = line.split('\t')
    if len(p) > 1 and p[0].isdigit() and int(p[0]) in icons:
        p[1] = str(icons[int(p[0])]); lines[i] = '\t'.join(p); changed += 1
if changed != len(icons):
    sys.exit(f'reward-details.tsv 只找到 {changed}/{len(icons)} 种骨灰')
details.write_text('\n'.join(lines), encoding='utf-8')
print(f'Wrote {len(icons)} spirit ash icons ({FIRST}-{FIRST + len(defs) - 1}) and updated reward-details.tsv')
