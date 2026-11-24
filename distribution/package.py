"""Build the playable release: the stage folder, its ZIP, and the launcher's incremental update data.

Upload to a GitHub release tagged v<N> (marked latest; never delete older releases, their packs stay referenced):
  ErdtreeTower-v<N>.zip                 full package for new players
  release-v<N>/tower-release.json       the launcher reads this from releases/latest
  release-v<N>/tower-pack-v<N>.bin      only file contents that no earlier release has
and the folder mirror-v<N>/releases to Cloudflare R2 (same layout as GitHub's download URLs): --upload does
that with Wrangler and checks the line (distribution/upload_r2.py).

Usage: python distribution/package.py [--previous <tower-release.json path or URL>] [--first-release] [--notes <file>] [--upload]
By default the previous manifest is taken from the latest GitHub release so unchanged files keep pointing at older packs.
"""
from pathlib import Path
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import urllib.error
import urllib.request
import zipfile
import re
import os
import xml.etree.ElementTree as ET
from native_build import build_native
from release_pack import build as build_release
from resource_filter import include_scene_asset
from bundle_seamless import stage_component
from session_gate_build import build_gate, manifest as gate_manifest

LATEST_MANIFEST = 'https://github.com/tamikip/erdtree-tower/releases/latest/download/tower-release.json'
parser = argparse.ArgumentParser(description='Build the release folder, ZIP and launcher update pack')
parser.add_argument('--previous', help='previous tower-release.json (path or https URL); default: the latest GitHub release')
parser.add_argument('--first-release', action='store_true', help='no earlier launcher release exists: every file goes into this pack')
parser.add_argument('--notes', type=Path, help='release notes shown by the launcher')
parser.add_argument('--display-version', required=True, help='package version shown to players, e.g. 1.0.0; internal build number stays independent')
parser.add_argument('--build-number', type=int, help='explicit monotonically increasing release build')
parser.add_argument('--upload', action='store_true', help='upload the mirror folder to Cloudflare R2 afterwards')
parser.add_argument('--wrangler', default='wrangler', help='Wrangler executable or bin/wrangler.js, for --upload')
parser.add_argument('--local-test', action='store_true', help='build a private test ZIP from local changes; never upload')
parser.add_argument('--signing-thumbprint', help='trusted code-signing certificate in the Windows personal store; absent means unsigned')
parser.add_argument('--signing-store', choices=['CurrentUser', 'LocalMachine'], default='CurrentUser')
parser.add_argument('--slim-game', type=Path, help='build a slim package using this verified local Game directory; first launch reconstructs large assets')
args = parser.parse_args()
if args.local_test and args.upload:
    parser.error('--local-test cannot be uploaded')
if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', args.display_version):
    parser.error('--display-version must be major.minor.patch')
if args.upload:
    import upload_r2
    upload_r2.validate_base(json.loads((Path(__file__).resolve().parents[1] / 'launcher/sources.json').read_text(encoding='utf-8'))['mirror'])
    if not shutil.which(args.wrangler) and not Path(args.wrangler).is_file():
        parser.error('Wrangler not found; install it or pass --wrangler <bin/wrangler.js>')

project = Path(__file__).resolve().parents[1]
os.environ['TOWER_PRODUCT_VERSION'] = args.display_version
from verify_release_sources import require_committed
if not args.local_test:
    require_committed(project)
source = project / 'release-preview'
dist = project / 'dist'
profile = (source / 'preview.me3').read_text(encoding='utf-8-sig')
native = re.findall(r"path\s*=\s*['\"](erdtree_tower_preview_v\d+\.dll)['\"]", profile)
if len(native) != 1:
    raise SystemExit('Expected one versioned native DLL in preview.me3')
version = re.search(r'_v(\d+)\.dll$', native[0]).group(1)
if args.build_number is not None:
    if args.build_number <= 0: parser.error('--build-number must be positive')
    version = str(args.build_number)
    updated_native = f'erdtree_tower_preview_v{version}.dll'
    profile = profile.replace(native[0], updated_native)
    native[0] = updated_native
stage = dist / f'ErdtreeTower-v{version}'
if stage.exists():
    raise SystemExit('Staging directory already exists; preserve it or choose a new release directory.')
if args.slim_game:
    subprocess.run(['dotnet', 'run', '--project', str(project / 'scene-builder'), '--',
                    str(args.slim_game), str(dist / f'audit-v{version}'),
                    str(project / 'scene-assets'), 'spirit-resource-verify'], cwd=project, check=True)
built_native = build_native(project)
# Launcher: exe and its config at the package root, every other build output under launcher/ (App.config probes there)
launcher_build = Path(tempfile.mkdtemp(prefix='tower-launcher-'))
subprocess.run(['dotnet', 'build', str(project / 'launcher' / 'Tower.Launcher'), '-c', 'Release', '-o', str(launcher_build)], check=True)
stage.mkdir(parents=True)

def copy_file(src, relative):
    target = stage / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, target)

def copy_tree(src, relative):
    for item in src.rglob('*'):
        if item.is_symlink() or item.stat().st_file_attributes & 0x400:
            raise RuntimeError(f'Reparse point rejected: {item}')
        if item.is_file():
            if item.suffix.lower() in ('.pdb', '.log', '.bak'):
                continue
            if relative == 'scene-assets' and (item.name == 'generation.txt' or item.name.endswith('-build.json') or item.name.endswith('-audit.tsv') or item.suffix.lower() == '.png'):
                continue
            if relative == 'scene-assets' and not include_scene_asset(item.relative_to(src)):
                continue
            copy_file(item, Path(relative) / item.relative_to(src))

copy_file(built_native, native[0])
copy_file(built_native.with_name('tower_transmog.dll'), 'tower_transmog.dll')
for name in ['LICENSE', 'ADDITIONAL-TERMS.md']:
    copy_file(project / name, 'docs/' + name)
(stage / 'preview.me3').write_text(profile, encoding='utf-8')
# Ship launcher sources directly so a stale release-preview copy cannot leak into the package.
for name in ['Launch-Preview.ps1', 'Launch-Seamless-Tower.ps1', 'New-PartyConfig.ps1', 'seamless-preset.ini']:
    copy_file(project / name, name)
copy_file(project / 'THIRD_PARTY_NOTICES.md', 'docs/THIRD_PARTY_NOTICES.md')
for name in ['launch-steamos.sh', 'STEAMOS.md']:
    copy_file(project / name, name)
for name in ['scene-assets', 'licenses']:
    copy_tree(project / name, name)
copy_tree(source / 'loader/bin', 'loader/bin')
copy_tree(project / 'loader/yafsml', 'loader/yafsml')
stage_component(project / 'SeamlessCoop', stage / 'SeamlessCoop', project / 'SeamlessCoop/default.ini')
copy_file(build_gate(project), 'SeamlessCoop/Tower.SeamlessGate.dll')
copy_file(project / 'native/vendor/hudhook/vendor/minhook/LICENSE.txt', 'licenses/SessionGate-MinHook.txt')
copy_file(project / 'SeamlessCoop/README.md', 'SeamlessCoop/README.md')
for name in ['LICENSE-MIT', 'LICENSE-APACHE']:
    copy_file(source / 'loader' / name, 'loader/' + name)
for name in ['Start-Tower.ps1', 'Launch-Logged.ps1', 'Tower-Loader.ps1', 'Tower-Network.ps1', 'Tower-SessionGate.ps1', 'Check-TowerNetwork.ps1', 'Diagnose-Startup.ps1', '启动崩溃诊断.cmd']:
    copy_file(Path(__file__).parent / name, name)
copy_file(Path(__file__).parent / '安装教程.md', 'docs/安装教程.md')
for name in ['ErdtreeTowerLauncher.exe', 'ErdtreeTowerLauncher.exe.config']:
    copy_file(launcher_build / name, name)
for library in sorted(launcher_build.glob('*.dll')):
    if library.name != 'Microsoft.Web.WebView2.Wpf.dll':
        copy_file(library, 'launcher/' + library.name)
copy_file(launcher_build / 'runtimes/win-x64/native/WebView2Loader.dll', 'launcher/WebView2Loader.dll')
copy_tree(launcher_build / 'launcher/ui', 'launcher/ui')
copy_file(launcher_build / 'launcher/sources.json', 'launcher/sources.json')
mirror_url = json.loads((launcher_build / 'launcher/sources.json').read_text(encoding='utf-8')).get('mirror', '')
if not mirror_url:
    print('Warning: launcher/sources.json has no mirror, players will only update from GitHub')
shutil.rmtree(launcher_build)
for entry in ['Start-Tower']:
    (stage / f'{entry}.cmd').write_text(
        '@echo off\nsetlocal\n'
        '"%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" '
        '-NoProfile -ExecutionPolicy Bypass -File "%~dp0Start-Tower.ps1" %*\n'
        'set "tower_exit_code=%errorlevel%"\nif not "%tower_exit_code%"=="0" pause\n'
        'exit /b %tower_exit_code%\n', encoding='ascii')
(stage / 'asset-mode.txt').write_text('full\n', encoding='ascii')
if args.local_test:
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=project).decode().strip()
    patch = subprocess.check_output(['git', 'diff', '--binary', 'HEAD'], cwd=project)
    untracked = subprocess.check_output(['git', 'ls-files', '--others', '--exclude-standard', '-z'], cwd=project).decode('utf-8').split('\0')
    extras = {name: hashlib.sha256((project / name).read_bytes()).hexdigest() for name in untracked
              if name and not name.startswith(('work/', '.playwright-cli/')) and (project / name).is_file()}
    (stage / 'BUILD-INFO.json').write_text(json.dumps({'localTest': True, 'gitHead': head, 'includesLocalChanges': bool(patch or extras), 'patchSha256': hashlib.sha256(patch).hexdigest(), 'additionalSourceHashes': extras}, ensure_ascii=False, indent=2), encoding='utf-8')
    if args.notes:
        (stage / 'docs/好友测试说明.txt').write_text(args.notes.read_text(encoding='utf-8'), encoding='utf-8-sig')
if args.slim_game:
    from slim_assets import prepare
    prepare(project, stage, args.slim_game)
if args.signing_thumbprint:
    subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-File', str(project / 'distribution/Sign-Tower.ps1'),
                    '-PackageRoot', str(stage), '-CertificateThumbprint', args.signing_thumbprint, '-Store', args.signing_store], check=True)
gate_manifest(stage / 'SeamlessCoop')
files = sorted(p for p in stage.rglob('*') if p.is_file())
for file in files:
    name = file.relative_to(stage).as_posix()
    if name.startswith('network-patches/') or file.name in ['OnlineFix64.dll', 'OnlineFix.ini', 'Switch-TowerNetwork.ps1', 'Network-Manual.ps1', 'Network-Files.ps1', 'Install-B.cmd', 'Restore-A.cmd', '联机环境切换(A-B).cmd']:
        raise RuntimeError(f'External patch must not be shipped in the main package: {name}')
    if (file.suffix in ['.log', '.sl2', '.co2', '.ertowercoop', '.ertowerparty', '.party-award', '.appearance'] or
            file.suffix.lower() in ['.pdb', '.bak'] or
            file.name in ['host.json', 'guest.json', 'GamePath.txt', 'GamePath.OnlineFix.txt', 'tower-options.ini', 'tower-debug.enable', 'ersc_settings.ini'] or
            (name.startswith('tower-') and file.suffix in ['.json', '.jsonl', '.class', '.fp', '.history']) or
            '/runtime/' in name or '/party-runtime/' in name or '/onlinefix-runtime/' in name or '/onlinefix-party-runtime/' in name):
        raise RuntimeError(f'Private runtime file rejected: {name}')
manifest = ''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(stage).as_posix()}\n' for p in files)
(stage / 'SHA256SUMS.txt').write_text(manifest, encoding='utf-8')


def previous_manifest():
    if args.first_release:
        return None
    source = args.previous or LATEST_MANIFEST
    if not source.startswith('https://'):
        return Path(source)
    target = dist / f'previous-for-v{version}-tower-release.json'
    try:
        with urllib.request.urlopen(source, timeout=60) as response:
            target.write_bytes(response.read())
    except urllib.error.HTTPError as error:
        if error.code == 404:
            raise SystemExit('The latest GitHub release has no tower-release.json: pass --first-release for the first launcher '
                             'release, or --previous <file>. Stage kept at ' + str(stage))
        raise
    return target


release_dir = dist / f'release-v{version}'
launcher_version = ET.parse(project / 'launcher/Tower.Launcher/Tower.Launcher.csproj').findtext('.//Version')
release = build_release(stage, version, release_dir, previous_manifest(), args.notes.read_text(encoding='utf-8') if args.notes else '', args.display_version, launcher_version)
# Mirror upload folder: the same paths as GitHub's release download URLs
mirror_dir = dist / f'mirror-v{version}' / 'releases'
for folder in ([] if args.local_test else [mirror_dir / 'download' / release['tag'], mirror_dir / 'latest' / 'download']):
    folder.mkdir(parents=True, exist_ok=True)
    shutil.copy2(release_dir / 'tower-release.json', folder / 'tower-release.json')
if not args.local_test:
    shutil.copy2(release_dir / release['pack']['asset'], mirror_dir / 'download' / release['tag'] / release['pack']['asset'])
# The ZIP carries the install record, so the launcher in a freshly extracted package knows its exact version and files
(stage / 'tower-install.json').write_text(json.dumps({'version': release['version'], 'displayVersion': release['displayVersion'], 'launcherVersion': release['launcherVersion'], 'localTest': args.local_test, 'files': release['files']}, ensure_ascii=False),
                                          encoding='utf-8')
archive = dist / f'ErdtreeTower-v{version}.zip'
with zipfile.ZipFile(archive, 'x', zipfile.ZIP_DEFLATED, compresslevel=6) as output:
    for file in sorted(stage.rglob('*')):
        if file.is_file():
            output.write(file, Path(stage.name) / file.relative_to(stage))
with zipfile.ZipFile(archive) as output:
    assert output.testzip() is None
checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
(dist / f'ErdtreeTower-v{version}.zip.sha256').write_text(f'{checksum}  {archive.name}\n', encoding='ascii')
print(f'Packaged {len(files) + 2} files, {archive.stat().st_size / 1024**2:.1f} MiB: {archive}')
print(f'SHA256 {checksum}')
if not args.local_test:
    print(f'Upload to GitHub release {release["tag"]} (latest): {archive.name}, {release_dir / "tower-release.json"}, '
          f'{release_dir / release["pack"]["asset"]}')
if args.upload:
    import sys
    for phase in ['files', 'latest']:
        subprocess.run([sys.executable, str(project / 'distribution/upload_r2.py'), str(mirror_dir.parent),
                        '--phase', phase, '--wrangler', args.wrangler, '--apply'], check=True)
elif args.local_test:
    print('Local test ZIP only; no GitHub or R2 publication performed.')
elif mirror_url:
    print(f'Mirror: python distribution/upload_r2.py {mirror_dir.parent}  (or upload {mirror_dir} by hand, '
          f'download/ first, then latest/, and run check_mirror.py {mirror_url} --expect {version})')
