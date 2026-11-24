"""Build the separate, user-invoked patch and recovery kit; never merge it into the mod ZIP."""
import argparse
import hashlib
from pathlib import Path
import zipfile

FILES = {
    'OnlineFix64.dll': '71a4f566abbf2ceb3b0d6159cd7950d120593c5badda4dde2711671957f16003',
    'steam_api64.dll': 'ca1fa9a7609ab10b7926400559cf073e5888423cc156af72c6027d72a89eea73',
    'winmm.dll': '54a6ece54641ee6e437caa351e493ddf4435f3e23e3d4ab4655c35f57b94fc59',
}


def build(patch, output):
    project = Path(__file__).resolve().parents[1]
    patch, output = Path(patch), Path(output)
    for name, expected in FILES.items():
        if hashlib.sha256((patch / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f'Unsupported supplied patch: {name}')
    files = {f'payload/{name}': (patch / name).read_bytes() for name in [*FILES, 'OnlineFix.ini']}
    files['payload/dlllist.txt'] = b'OnlineFix64.dll'
    for name in ['Network-Manual.ps1', 'Network-Files.ps1', '联机环境切换(A-B).cmd', '使用说明.txt']:
        files[name] = (project / 'distribution/manual-network' / name).read_bytes()
    files['Tower-Network.ps1'] = (project / 'distribution/Tower-Network.ps1').read_bytes()
    files['SHA256SUMS.txt'] = ''.join(f'{hashlib.sha256(data).hexdigest()}  {name}\n' for name, data in sorted(files.items())).encode()
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
        for name, data in files.items():
            archive.writestr('B-Manual-Patch/' + name, data)
    with zipfile.ZipFile(output) as archive:
        if archive.testzip() is not None:
            raise ValueError('Patch archive integrity failure')
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(output.suffix + '.sha256').write_text(f'{digest}  {output.name}\n', encoding='utf-8')
    print(f'Created separate manual patch: {output} ({output.stat().st_size} bytes)\nSHA256 {digest}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--patch-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    build(args.patch_dir, args.output)
