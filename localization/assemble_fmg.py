"""Assemble checked English FMG entries from the shared runtime translator output."""
import argparse
import json
import re
import textwrap
from pathlib import Path


def assemble(directory):
    def read(name):
        return json.loads((directory / name).read_text(encoding='utf-8'))
    source = read('fmg-input.json')
    translated = read('fmg-translated.json')
    if len(source) != len(translated):
        raise ValueError('Translation input/output count mismatch')
    lookup = dict(zip(source, translated))
    result = []
    for row in read('fmg-prepared.json'):
        value = ''.join(part if part.startswith('<') else lookup[part]
                        for part in re.split(r'(<[^>]*>)', row['prepared']) if part)
        if re.search('[\u3400-\u9fff]', value):
            raise ValueError(f"Untranslated entry: {row['table']}/{row['id']}")
        if re.findall(r'<[^>]*>', row['text']) != re.findall(r'<[^>]*>', value):
            raise ValueError(f"Markup changed: {row['table']}/{row['id']}")
        table, entry_id = row['table'], row['id']
        caption_limit = (
            7 if table == 25 and (100000000 <= entry_id < 120000000 or 160000000 <= entry_id < 200000000) else
            12 if table == 26 and (10000000 <= entry_id < 15000000 or 20000000 <= entry_id < 40000000) else
            12 if table == 27 and 1100000 <= entry_id < 2000000 else
            0
        )
        if caption_limit and '\n\n' in value:
            head, rest = value.split('\n\n', 1)
            if (head.startswith('<font ') and head.endswith('</font>')) or head.startswith(('Scaling:', 'Armor Type:')):
                info, lore = head, rest
            else:
                lore, info = value.rsplit('\n\n', 1)
            budget = caption_limit - len(info.splitlines()) - 1
            kept = []
            for paragraph in lore.split('\n\n'):
                lines = textwrap.wrap(' '.join(paragraph.split()), width=72, break_long_words=False, break_on_hyphens=False)
                needed = len(lines) + bool(kept)
                if needed > budget:
                    if not kept and budget > 0:
                        kept.extend(lines[:budget])
                    break
                if kept:
                    kept.append('')
                kept.extend(lines)
                budget -= needed
            value = info + ('\n\n' + '\n'.join(kept) if kept else '')
            if len(value.splitlines()) > caption_limit:
                raise ValueError(f"Affix caption exceeds {caption_limit} lines: {table}/{entry_id}")
        if re.findall(r'<[^>]*>', row['text']) != re.findall(r'<[^>]*>', value):
            raise ValueError(f"Caption fitting changed markup: {row['table']}/{row['id']}")
        result.append(dict(pack=row['pack'], table=row['table'], id=row['id'], source=row['text'], text=value))
    (directory / 'fmg-english.json').write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')
    print(f'Checked {len(result)} English FMG entries: no Chinese, markup retained, weapon captions fitted')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    assemble(parser.parse_args().directory)
