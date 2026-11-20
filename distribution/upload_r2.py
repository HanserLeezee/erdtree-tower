"""Publish a checked release to R2 using Wrangler's local login. Dry-run by default."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.error
import urllib.request
from urllib.parse import urlparse

import check_mirror

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def validate_base(base):
    url = urlparse(base)
    if url.scheme != 'https' or not url.hostname or url.username or url.password or url.query or url.fragment or url.path != '/releases':
        raise ValueError('Public base must be https://<host>/releases')


def release(root):
    latest = root / 'releases/latest/download/tower-release.json'
    manifest = json.loads(latest.read_text(encoding='utf-8-sig'))
    tag = f'v{manifest["version"]}'
    if manifest['tag'] != tag or manifest['pack']['asset'] != f'tower-pack-{tag}.bin':
        raise ValueError('Invalid release tag/pack')
    folder = root / 'releases/download' / tag
    version = folder / 'tower-release.json'
    pack = folder / manifest['pack']['asset']
    if latest.read_bytes() != version.read_bytes():
        raise ValueError('Latest and version manifests differ')
    if pack.stat().st_size != manifest['pack']['size'] or digest(pack) != manifest['pack']['sha256']:
        raise ValueError('Local pack size/hash mismatch')
    return manifest, [pack, version], latest


def command(tool, bucket, key, local):
    cache = 'no-cache' if key.startswith('releases/latest/') else 'public, max-age=31536000, immutable'
    return tool + ['r2', 'object', 'put', f'{bucket}/{key}', '--remote', '--file', str(local),
                   '--content-type', 'application/json' if key.endswith('.json') else 'application/octet-stream',
                   '--cache-control', cache]


def verify_references(base, manifest):
    _, problems = check_mirror.check(base, manifest['version'], manifest=manifest)
    if problems:
        raise ValueError('; '.join(problems))


def remote_matches(url, local):
    request = urllib.request.Request(url, headers={'User-Agent': 'ErdtreeTowerPublisher/1.0'})
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            h, size = hashlib.sha256(), 0
            for chunk in iter(lambda: response.read(1024 * 1024), b''):
                size += len(chunk)
                if size > local.stat().st_size:
                    raise ValueError('Remote object exceeds local size; not overwritten')
                h.update(chunk)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            error.close()
            return False
        raise
    if size != local.stat().st_size or h.hexdigest() != digest(local):
        raise ValueError('Remote object differs; not overwritten')
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mirror_dir', type=Path)
    parser.add_argument('--phase', choices=['files', 'latest'], default='files')
    parser.add_argument('--bucket', default='erdtree-tower-releases')
    parser.add_argument('--base')
    parser.add_argument('--wrangler', default='wrangler', help='Wrangler executable or bin/wrangler.js')
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    base = (args.base or json.loads((ROOT / 'launcher/sources.json').read_text(encoding='utf-8'))['mirror']).rstrip('/')
    validate_base(base)
    manifest, files, latest = release(args.mirror_dir)
    tool = ['node', args.wrangler] if args.wrangler.endswith('.js') else [args.wrangler]
    if args.phase == 'latest':
        files = [latest]
        if args.apply:
            verify_references(base, manifest)
            try:
                _, data = check_mirror.fetch(f'{base}/latest/download/tower-release.json')
                current = json.loads(data)
                if current['version'] > manifest['version']:
                    raise ValueError('Refusing to downgrade latest')
                if current['version'] == manifest['version'] and current != manifest:
                    raise ValueError('Published manifest differs at the same version')
            except urllib.error.HTTPError as error:
                if error.code != 404:
                    raise
                error.close()
    for local in files:
        key = local.relative_to(args.mirror_dir).as_posix()
        print(f'{local.name} -> {args.bucket}/{key}', flush=True)
        if not args.apply:
            continue
        url = f'{base}/{local.relative_to(args.mirror_dir / "releases").as_posix()}'
        if args.phase == 'files' and remote_matches(url, local):
            continue
        subprocess.run(command(tool, args.bucket, key, local), check=True, timeout=1800)
        if not remote_matches(url, local):
            raise ValueError('Uploaded object is not publicly readable')
    if args.apply:
        verify_references(base, manifest)


if __name__ == '__main__':
    main()
