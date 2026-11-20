# Separate manual network patch

The mod package contains no B payload or installation/restoration scripts. Its launcher
detects the installed game profile and validates files without changing the game's DLLs.
`Start-Tower.ps1` and the direct launch entry default to the same automatic detection.
The settings page shows a read-only network environment; stale network settings cannot
request a patch switch.

Build the separate kit with `package_manual_network.py --patch-dir <supplied patch> --output <zip>`.
Users explicitly run `Install-B.cmd` or `Restore-A.cmd` with the game and launcher closed.
The kit retains the original local files and recovery journal under `Game/.tower-network`.
It also places `Restore-A.cmd` and its support scripts there, so recovery does not require
the original ZIP. Existing backups from the previous switching build are supported.
Missing/damaged B DLLs do not prevent restoration from an intact original backup.

The B payload has only the pinned proxy DLLs, OnlineFix.ini and a canonical DLL list with
no final newline. ME3 loads the mod package's existing Seamless component exactly once.
The main package rejects incomplete/incorrect external patch installations and pending
transactions; users must run the independent recovery/install tool themselves.

Do not redistribute the old build121 package or its A/B update ZIP under this policy.
Extract the new main ZIP to a fresh directory; overlaying an old directory does not remove
its previously bundled patch files. Never repackage an old installation as the clean main ZIP.
