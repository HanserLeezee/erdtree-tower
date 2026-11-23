"""Hide the top direction compass in the HUD (01_000_fe.gfx) using FFDec.

The compass is PlayerHUD.PersonalCompass (_01_000_FE_fla.Compass_Ani_151, symbol 469) with inner clip `_` (symbol 468).
Keeping the MovieClip intact while hiding both itself and all child clips on construction, timeline keyframes, and
ENTER_FRAME prevents the native C++ FrontEndView from hitting null Scaleform lookups or re-showing the bar after
menus/warps. Usage:
python scene-builder/native_compass_hide.py --ffdec <ffdec.jar> --original <dir with 01_000_fe.gfx> --output <new dir> [--java <java>] [--rebase]
"""
import argparse
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_name_glow as glow  # noqa: E402

STEM = '01_000_fe'
PACKAGE = '_01_000_FE_fla'
CLASS = 'Compass_Ani_151'

MEMBER = '''
      private function towerHideCompass(event:flash.events.Event = null) : void
      {
         this.visible = false;
         this.alpha = 0;
         if(this._ != null)
         {
            this._.visible = false;
            this._.alpha = 0;
         }
         for(var i:int = 0; i < this.numChildren; i++)
         {
            var child:flash.display.DisplayObject = this.getChildAt(i);
            if(child != null)
            {
               child.visible = false;
               child.alpha = 0;
            }
         }
      }
'''


def strip_previous(source):
    lines = source.split('\n')
    kept = []
    skipping = False
    for line in lines:
        if skipping:
            if line == '      }':
                skipping = False
            continue
        if re.match(r'\s*private function towerHideCompass\(', line):
            skipping = True
            continue
        if 'towerHideCompass' in line:
            continue
        kept.append(line)
    return re.sub(r'\n(?:[ \t]*\n){2,}', '\n\n', '\n'.join(kept))


def patch_source(source, rebase=False):
    if rebase:
        source = strip_previous(source)
    anchor = f'      public function {CLASS}()'
    frame = 'addFrameScript(0,this.frame1,8,this.frame9,16,this.frame17);'
    if source.count(anchor) != 1 or source.count(frame) != 1 or 'towerHideCompass' in source:
        raise ValueError('Unexpected Compass_Ani_151 source; refusing to patch')
    source = source.replace(anchor, MEMBER + '\n' + anchor)
    source = source.replace(
        frame,
        frame
        + '\n         this.towerHideCompass();'
        + '\n         addEventListener(flash.events.Event.ENTER_FRAME,this.towerHideCompass);',
    )
    for fn in ('frame1', 'frame9', 'frame17'):
        target = f'      internal function {fn}() : *\n      {{\n         stop();\n      }}'
        replacement = f'      internal function {fn}() : *\n      {{\n         stop();\n         this.towerHideCompass();\n      }}'
        if source.count(target) != 1:
            raise ValueError(f'Unexpected {fn} body in Compass_Ani_151')
        source = source.replace(target, replacement)
    return source


def build(ffdec, original, output, rebase=False, java='java'):
    output.mkdir(parents=True, exist_ok=False)
    (output / 'menu').mkdir()

    def run(*args):
        result = subprocess.run(
            [java, '-Djava.awt.headless=true', '-jar', str(ffdec), *map(str, args)],
            capture_output=True,
            text=True,
            encoding='utf-8',
            errors='replace',
        )
        with (output / 'build.log').open('a', encoding='utf-8') as log:
            log.write(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError('FFDec failed; see build.log')

    src = original / (STEM + '.gfx')
    dst = output / 'menu' / src.name
    with tempfile.TemporaryDirectory(prefix='tower-compass-') as tmp:
        tmp = Path(tmp)
        run('-export', 'script', tmp / 'before', src)
        relative = Path(PACKAGE) / (CLASS + '.as')
        script = tmp / 'before' / 'scripts' / relative
        changed = tmp / 'changed' / relative
        changed.parent.mkdir(parents=True)
        changed.write_text(patch_source(script.read_text(encoding='utf-8-sig').replace('\r\n', '\n'), rebase=rebase), encoding='utf-8')
        run('-importScript', src, dst, tmp / 'changed')
        count = glow.verify_tags(src.read_bytes(), dst.read_bytes())
        run('-export', 'script', tmp / 'after', dst)
        for before in (tmp / 'before' / 'scripts').rglob('*.as'):
            rel = before.relative_to(tmp / 'before' / 'scripts')
            after = tmp / 'after' / 'scripts' / rel
            if rel != relative and before.read_bytes() != after.read_bytes():
                raise ValueError(f'Unrelated script changed: {rel}')
        actual = (tmp / 'after' / 'scripts' / relative).read_text(encoding='utf-8-sig')
        if 'towerHideCompass' not in actual or 'ENTER_FRAME' not in actual:
            raise ValueError('Compiled compass hide handler missing')
        (output / (STEM + '-compass.as')).write_text(actual, encoding='utf-8')
    manifest = {
        'class': f'{PACKAGE}.{CLASS}',
        'path': 'PlayerHUD.PersonalCompass',
        'effect': 'hide top direction compass and all child clips without removing Scaleform instances',
        'files': [{
            'file': 'menu/' + src.name,
            'baseSha256': hashlib.sha256(src.read_bytes()).hexdigest(),
            'patchedSha256': hashlib.sha256(dst.read_bytes()).hexdigest(),
            'tagsVerified': count,
            'changedTag': 'DoABC',
        }],
    }
    (output / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    print(f'Built and structurally verified compass-hidden HUD ({count} tags verified, DoABC only).')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--ffdec', type=Path, required=True)
    parser.add_argument('--original', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--rebase', action='store_true')
    parser.add_argument('--java', default='java')
    args = parser.parse_args()
    build(args.ffdec.resolve(), args.original.resolve(), args.output.resolve(), args.rebase, args.java)
