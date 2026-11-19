"""Audited download exclusions. Keep all source files and unknown runtime assets."""

EMBEDDED = frozenset({
    'affix-catalog.txt', 'affix-details.tsv', 'affix-rarities.tsv',
    'affix-v2-catalog.txt', 'affix-v2-details.tsv', 'affix-v2-rarities.tsv',
    'protected-ash-pools.tsv', 'random-ash-details.tsv', 'random-ash-weapon-rows.txt',
    'custom-spirit-definitions.tsv',
    'reward-rarities.tsv', 'talisman-definitions.tsv', 'talisman-details.tsv', 'talisman-rarities.tsv',
})

BUILD_ONLY = frozenset({
    'heretical-rise-start.json', 'vanilla-talisman-quality.tsv',
    'affix-effect-remap.tsv', 'random-ashes.tsv', 'talisman-catalog.txt',
    'retired-spell-strip.json', 'reward-art/extra-icons.txt',
    'reward-art/rare-blue-generation.txt', 'reward-art/rarity-generation.txt',
})

EXCLUSIONS = {**dict.fromkeys(EMBEDDED, 'embedded in the DLL via include_str!'),
              **dict.fromkeys(BUILD_ONLY, 'scene-builder input/output or art-generation notes')}


def include_scene_asset(relative):
    path = str(relative).replace('\\', '/')
    # 舞会的舞曲是用户自备的商业歌曲：只在本机测试，绝不进发布包
    if path.startswith('dance/') and path.lower().endswith('.wav'):
        return False
    return path not in EXCLUSIONS
