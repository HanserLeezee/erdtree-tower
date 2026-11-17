"""Build the ERSC-only session adapter using the existing local Steam SDK cache."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ORIGINAL = '2ebde62be864ba3d759728aac44c459f0f813e8b6d7a342304ce68f6513f280c'

def manifest(folder):
    folder = Path(folder)
    hashes = {name: hashlib.sha256((folder / name).read_bytes()).hexdigest()
              for name in ('ersc.dll', 'Tower.SeamlessGate.dll')}
    if hashes['ersc.dll'] != ORIGINAL:
        raise ValueError('Session adapter requires the unmodified pinned ERSC 2.0.3')
    (folder / 'gate-manifest.json').write_text(json.dumps({'protocol': 1, 'files': hashes}, indent=2), encoding='utf-8')

def build_gate(project):
    project = Path(project).resolve()
    cache = project.parents[1] / 'work/tower-transmog-build'
    cmake = os.environ.get('TOWER_CMAKE') or shutil.which('cmake') or str(cache / 'cmake/cmake-4.4.5-windows-x86_64/bin/cmake.exe')
    sdk = Path(os.environ.get('TOWER_STEAMWORKS_SDK', str(cache / 'steamworks')))
    if not (sdk / 'include/steam/steam_api.h').exists():
        raise RuntimeError('Build the integrated transmog dependency first to prepare the Steam SDK.')
    build = Path(os.environ.get('TOWER_GATE_BUILD_DIR', str(project.parents[1] / 'work/session-gate-build')))
    subprocess.run([cmake, '-S', str(project / 'seamless-gate'), '-B', str(build), '-A', 'x64', '-DSTEAMWORKS_ROOT=' + str(sdk)], check=True)
    subprocess.run([cmake, '--build', str(build), '--config', 'Release', '--parallel', '6'], check=True)
    subprocess.run([str(build / 'Release/gate-tests.exe')], check=True)
    subprocess.run([str(build / 'abi/Release/gate-hook-tests.exe')], cwd=build / 'abi/Release', timeout=20, check=True)
    return build / 'Release/Tower.SeamlessGate.dll'
