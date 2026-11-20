import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'distribution'))
import upload_r2


class UploadR2(unittest.TestCase):
    def test_command_uses_remote_and_preserves_cache_policy(self):
        command = upload_r2.command(['wrangler'], 'tower', 'releases/download/v7/p.bin', Path('p.bin'))
        self.assertIn('--remote', command)
        self.assertIn('public, max-age=31536000, immutable', command)
        self.assertEqual(command[4], 'tower/releases/download/v7/p.bin')
        latest = upload_r2.command(['wrangler'], 'tower', 'releases/latest/download/tower-release.json', Path('m.json'))
        self.assertIn('no-cache', latest)

    def test_latest_is_blocked_if_any_historical_pack_fails(self):
        with patch.object(upload_r2.check_mirror, 'check', return_value=({}, ['missing historical pack'])):
            with self.assertRaisesRegex(ValueError, 'missing historical pack'):
                upload_r2.verify_references('https://example.com/releases', {'version': 7})

    def test_invalid_public_base_is_rejected(self):
        for base in ['http://example.com/releases', 'https://example.com/wrong', 'https://user:pass@example.com/releases']:
            with self.assertRaises(ValueError):
                upload_r2.validate_base(base)

    def test_local_pack_mismatch_blocks_upload(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            import json
            m = {'version': 7, 'tag': 'v7', 'pack': {'asset': 'tower-pack-v7.bin', 'size': 1, 'sha256': '0' * 64}}
            for folder in ['latest/download', 'download/v7']:
                (root / 'releases' / folder).mkdir(parents=True)
                (root / 'releases' / folder / 'tower-release.json').write_text(json.dumps(m))
            (root / 'releases/download/v7/tower-pack-v7.bin').write_bytes(b'x')
            with self.assertRaisesRegex(ValueError, 'hash'):
                upload_r2.release(root)


if __name__ == '__main__':
    unittest.main()
