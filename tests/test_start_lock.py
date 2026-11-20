import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest


@unittest.skipUnless(os.name == 'nt', 'Windows PowerShell startup test')
class StartLockTests(unittest.TestCase):
    def test_migration_does_not_copy_its_own_held_launch_lock(self):
        project = Path(__file__).resolve().parents[1]
        source = (project / 'Launch-Seamless-Tower.ps1').read_text(encoding='utf-8-sig')
        function = source[source.index('function Copy-TowerSoloData('):source.index('function Set-TowerSpectatorMode(')]
        shell = Path(os.environ['SystemRoot']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
        with tempfile.TemporaryDirectory(prefix='tower-migrate-lock-') as folder:
            root = Path(folder)
            runtime = root / 'runtime'
            runtime.mkdir()
            (root / 'tower-0-0.json').write_text('progress')
            (root / 'tower-0-1.json').write_text('old progress')
            (runtime / 'tower-0-1.json').write_text('current progress')
            (root / 'tower-debug.enable').touch()
            (root / 'tower-lobby.json').write_text('private')
            fixture = root / 'migration.ps1'
            fixture.write_text("$ErrorActionPreference='Stop'\n$env:APPDATA=$null\n" + function + '''
$guard=[IO.File]::Open((Join-Path $PSScriptRoot '.tower-launch.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
try {Copy-TowerSoloData $PSScriptRoot (Join-Path $PSScriptRoot 'runtime') 'ertowercoop'} finally {$guard.Dispose()}
''', encoding='utf-8-sig')
            result = subprocess.run([str(shell), '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', str(fixture)], capture_output=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((runtime / 'tower-0-0.json').read_text(), 'progress')
            self.assertEqual((runtime / 'tower-0-1.json').read_text(), 'current progress')
            self.assertEqual(sorted(p.name for p in runtime.iterdir()), ['tower-0-0.json', 'tower-0-1.json'])

    def test_two_launchers_cannot_start_the_same_package_and_lock_releases(self):
        script = Path(__file__).resolve().parents[1] / 'distribution/Start-Tower.ps1'
        shell = Path(os.environ['SystemRoot']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
        with tempfile.TemporaryDirectory(prefix='tower-launch-lock-') as folder:
            root = Path(folder)
            shutil.copy2(script, root / script.name)
            (root / 'eldenring.exe').write_bytes(b'fixture, never executed')
            entry = '''param($GameDir,$Loader,$Network,[switch]$CheckOnly)
if($CheckOnly){return}
[IO.File]::AppendAllText((Join-Path $PSScriptRoot 'entered.txt'),"entry`n")
$deadline=[DateTime]::UtcNow.AddSeconds(10)
while(!(Test-Path -LiteralPath (Join-Path $PSScriptRoot 'release'))){
    if([DateTime]::UtcNow -ge $deadline){throw 'Fixture timeout'}
    Start-Sleep -Milliseconds 50
}
'''
            (root / 'Launch-Seamless-Tower.ps1').write_text(entry, encoding='utf-8-sig')
            command = [str(shell), '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                       '-File', str(root / script.name), '-GameDir', str(root), '-Network', 'Steam']
            first = subprocess.Popen(command + ['-ErrorFile', str(root / 'first-error.txt')], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                deadline = time.monotonic() + 7
                while not (root / 'entered.txt').exists():
                    if first.poll() is not None:
                        stdout, stderr = first.communicate()
                        self.fail(str((stdout, stderr, (root / 'first-error.txt').read_text(encoding='utf-8-sig') if (root / 'first-error.txt').exists() else 'no error file')))
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.05)
                error = root / 'second-error.txt'
                second = subprocess.run(command + ['-ErrorFile', str(error)], capture_output=True, timeout=7)
                self.assertNotEqual(second.returncode, 0)
                self.assertIn('已有启动任务正在运行', error.read_text(encoding='utf-8-sig'))
                self.assertEqual((root / 'entered.txt').read_text().splitlines(), ['entry'])
                (root / 'release').touch()
                first.communicate(timeout=7)
                self.assertEqual(first.returncode, 0)
                third = subprocess.run(command, capture_output=True, timeout=7)
                self.assertEqual(third.returncode, 0, third.stderr)
                self.assertEqual(len((root / 'entered.txt').read_text().splitlines()), 2)
            finally:
                if first.poll() is None:
                    first.kill()
                first.communicate()


if __name__ == '__main__':
    unittest.main()
