# FileGo 0.0.1 Release Notes (Draft)

> Status: release documentation draft. This document does **not** announce a GitHub Release and does not authorize tag creation or publication.
>
> A final release still requires the project owner's Windows desktop acceptance, an independent review with `APPROVED_FOR_RELEASE`, and explicit authorization to tag and publish.

## Supported platform

- Windows 10 version 22H2 and Windows 11
- x86-64 only
- Portable, unsigned `.exe` and `.zip`; no installer is provided

## Highlights

FileGo is a tray-first launcher for folders that users explicitly add. It does not scan disks, index ordinary files, upload paths, or enable telemetry by default.

0.0.1 provides:

- a tray-first search window with keyboard-oriented use;
- name, path, category, and tag search, including pinyin, English initials, multi-token, continuous-fuzzy, and edit-distance matching;
- search filters and deterministic result ordering;
- folder, category, and tag management, including safe removal of shortcut records only — never the real folder or its contents;
- settings for appearance, search behavior, launch at login, and data handling;
- versioned JSON import/export and local backups with recovery paths;
- a configurable global hotkey and single-instance activation;
- Simplified Chinese and English user interfaces.

## Getting and running FileGo

Download the Windows x86-64 ZIP (or standalone EXE) only from the intended GitHub Release once it is published. Verify its SHA-256 before running it, then extract the ZIP to a directory you manage and run `FileGo.exe`.

The portable ZIP contains:

```text
FileGo.exe
LICENSE
README.md
THIRD_PARTY_LICENSES.html
licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md
```

The build is unsigned. Windows SmartScreen may show a warning the first time it runs. Verify the download source and SHA-256 first, then use the specific Windows prompt to make your own decision. Do not broadly disable SmartScreen, antivirus protection, or other operating-system security features to run FileGo.

## Verify SHA-256

The release will provide a `FileGo-0.0.1-SHA256SUMS.txt` file beside the EXE and ZIP. In PowerShell, run the following from the download directory:

```powershell
Get-FileHash ".\FileGo-0.0.1-windows-x86_64.exe" -Algorithm SHA256
Get-FileHash ".\FileGo-0.0.1-windows-x86_64.zip" -Algorithm SHA256
Get-Content ".\FileGo-0.0.1-SHA256SUMS.txt"
```

Compare each printed value character-for-character with the value for the same filename in `SHA256SUMS`. Do not run a file whose hash does not match; download it again from the original source.

## Data location and backups

FileGo stores its live data for the current Windows user under:

```text
%LOCALAPPDATA%\FileGo
```

The primary document is `data.json`. FileGo also maintains recovery data and supports user-created backups and JSON export through the Settings → Data page.

Before updating, downgrading, importing data, or manually cleaning data, create a backup or export versioned JSON to a location outside the FileGo data directory. Keep a copy until the new version has been tested with your data.

## Upgrade, downgrade, and uninstall

- **Upgrade:** Back up or export data first. Keep the previous portable ZIP/EXE until the new build starts normally and the expected records and settings are present.
- **Downgrade:** Restore an export or backup that is known to be compatible with the version you are returning to. Do not assume an older build can read data saved by a newer schema; FileGo rejects unsupported future schema versions rather than silently overwriting them.
- **Uninstall:** Exit FileGo and delete its portable application directory. This does not delete `%LOCALAPPDATA%\FileGo` data or backups. Delete that data directory separately only after confirming that a backup is available if you want to erase local data.

## Known limitations and release-validation items

The following are not claims of successful desktop validation. They remain recorded in `m07-known-issues.md` and the user-driven manual-acceptance checklist:

- Pinyin search uses a deterministic first pronunciation for polyphonic characters, so an uncommon pronunciation may not match.
- Native Windows drag-and-drop is limited by Slint 1.18 not forwarding `DroppedFile`; use the supported picker or paste workflow where applicable.
- Actual behavior for tray recovery after Explorer restarts, display changes, sleep/wake, and temporary network disconnection must be checked on a real Windows desktop.
- Mixed-DPI displays, text scaling, high contrast, reduced motion, and screen-reader behavior require real-device validation; screen-reader support is limited by Slint 1.18.
- Microsoft Pinyin candidate-window placement and composition behavior require manual validation.
- Real-hardware measurements for 10,000-record search, hotkey-to-visible time, cold start, idle CPU/disk activity, and memory remain acceptance measurements. The listed performance values are goals, not release claims before the manual results are recorded.

Features intentionally outside the 0.0.1 MVP are not represented above as shipped capabilities.

## License and third-party notices

FileGo source code is licensed under the MIT License. Every portable package includes `LICENSE`, a generated `THIRD_PARTY_LICENSES.html` inventory, and the Slint Royalty-free 2.0 license text at `licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md`.
