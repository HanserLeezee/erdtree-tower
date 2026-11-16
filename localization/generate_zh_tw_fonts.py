"""繁体界面的补充字形：把现有界面字表（scene-assets/fonts/ui-glyphs.txt）逐字转成繁体，只取字表里没有的那些字，
从思源宋体裁出补充字体（只在语言设为繁体时才合并加载，简体与英文的包体和字体贴图不变）。
写出 scene-assets/fonts/TowerSerif-TW-Regular.otf、TowerTitleSerif-TW-Bold.otf 与字表 ui-glyphs-tw.txt。
字体按 SIL OFL 1.1，改名为 Tower Serif TW / Tower Title Serif TW，许可见 licenses/NotoSerifCJK-OFL.txt。
用法：python3 localization/generate_zh_tw_fonts.py <NotoSerifCJK-Regular.ttc> <NotoSerifCJK-Bold.ttc>（需要 fontTools）"""
import pathlib, sys
from fontTools.ttLib import TTCollection
from fontTools.subset import Options, Subsetter
root = pathlib.Path(__file__).resolve().parent.parent
fonts = root / "scene-assets/fonts"; tables = root / "localization/zh-TW"
def load(name):
    return dict(line.split("\t") for line in (tables / name).read_text(encoding="utf-8").splitlines())
st, phrases, variants = load("st.tsv"), load("phrases.tsv"), load("variants.tsv")
single = {k: v for k, v in st.items() if len(k) == 1}
base = set((fonts / "ui-glyphs.txt").read_text(encoding="utf-8"))
wanted = set()
for c in base: wanted.update(single.get(c, c))
for v in list(phrases.values()) + list(variants.values()): wanted.update(v)
extra = sorted(c for c in wanted - base if ord(c) >= 0x80 and c.isprintable())
(fonts / "ui-glyphs-tw.txt").write_text("".join(extra), encoding="utf-8")
def make(source, face, family, style, name):
    font = next(f for f in TTCollection(str(source)).fonts if f["name"].getDebugName(4) == face)
    options = Options(); options.layout_features = ["*"]; options.name_IDs = ["*"]; options.notdef_outline = True
    subsetter = Subsetter(options); subsetter.populate(unicodes=[ord(c) for c in extra]); subsetter.subset(font)
    for record in font["name"].names:
        if record.nameID in (1, 16): record.string = family
        elif record.nameID == 4: record.string = f"{family} {style}"
        elif record.nameID == 6: record.string = f"{family.replace(' ', '')}-{style}"
        elif record.nameID == 3: record.string = f"{family} {style}; subset of {face}"
    out = fonts / name; font.save(str(out))
    covered = sum(1 for c in extra if ord(c) in font.getBestCmap())
    print(name, f"{covered}/{len(extra)} glyphs,", out.stat().st_size // 1024, "KB")
make(pathlib.Path(sys.argv[1]), "Noto Serif CJK SC", "Tower Serif TW", "Regular", "TowerSerif-TW-Regular.otf")
make(pathlib.Path(sys.argv[2]), "Noto Serif CJK SC Bold", "Tower Title Serif TW", "Bold", "TowerTitleSerif-TW-Bold.otf")
