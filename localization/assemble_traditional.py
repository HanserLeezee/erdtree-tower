"""Prepare/assemble changed native FMGs using the runtime Traditional converter."""
import argparse
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('directory', type=Path)
parser.add_argument('--assemble', action='store_true')
args = parser.parse_args()
rows = json.loads((args.directory / 'custom-fmg.json').read_text(encoding='utf-8'))
texts = list(dict.fromkeys(row['text'] for row in rows))
if not args.assemble:
    (args.directory / 'traditional-input.json').write_text(json.dumps(texts, ensure_ascii=False), encoding='utf-8')
    print(f'Prepared {len(texts)} unique Traditional strings')
else:
    converted = json.loads((args.directory / 'traditional-translated.json').read_text(encoding='utf-8'))
    if len(texts) != len(converted): raise ValueError('Traditional translation count mismatch')
    lookup = dict(zip(texts, converted))
    result = []
    for row in rows:
        source = row['text']; text = lookup[source]
        if not text.strip() or re.findall(r'<[^>]*>', source) != re.findall(r'<[^>]*>', text):
            raise ValueError(f"Empty translation or changed markup: {row['table']}/{row['id']}")
        result.append(dict(pack=row['pack'], table=row['table'], id=row['id'], source=source, text=text))
    (args.directory / 'fmg-traditional.json').write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')
    print(f'Checked {len(result)} Traditional FMG entries; markup retained')
