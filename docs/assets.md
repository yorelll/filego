# Visual asset provenance and processing

## Owner-provided source artwork

The project owner supplied the following source images on 2026-09-21:

- `assets/source/main-logo-original.png` — source for the application/window logo.
- `assets/source/tray-logo-original.png` — source for the system-tray logo.

Source SHA-256 values:

- main: `89abd58fa7aea5511c67eefb3e2da831ae90b5943724ba878114a5faf5f395a9`
- tray: `5035b44b49e400f3f22cab5e991be5cdffaf984a17f5a943e29cede224d58fb7`

Both source files are RGB images whose visible checkerboard is baked into the pixels; it is not a real alpha channel. The source artwork is retained byte-for-byte under normalized names for traceability and is not referenced by the application binary.

These files are owner-provided project assets. On 2026-09-21, the project owner confirmed ownership and authorized their inclusion and distribution in the public QuickFolder project and its release artifacts. The generated derivatives are therefore approved project assets; this provenance record is retained separately from third-party dependency notices.

## Generated assets

`tools/process_icons.py` reproducibly generates:

| File | Purpose | Format |
|---|---|---|
| `assets/icons/quickfolder.png` | Main window/application artwork | 512×512 RGBA PNG |
| `assets/icons/quickfolder-tray.png` | Slint `SystemTrayIcon` image | 32×32 RGBA PNG |
| `assets/icons/quickfolder.ico` | Embedded Windows executable/file icon resource | Multi-resolution ICO: 16, 20, 24, 32, 48, 64, 128, 256 px |

The script:

1. identifies foreground using blue-channel dominance over the near-neutral checkerboard;
2. retains only substantial connected components, including the separate magnifying-glass shape;
3. reconstructs smooth edge alpha and replaces matte-contaminated edge color with the nearest solid foreground color to avoid white halos;
4. crops the recovered foreground with transparent margin;
5. centers it on a square transparent canvas and downsamples with Lanczos;
6. writes optimized PNG files and a valid multi-resolution ICO.

Regeneration is a maintenance operation and requires Python, Pillow, NumPy, SciPy, and OpenCV. It is deliberately not part of the Rust build or GitHub Actions pipeline. Run from the repository root:

```powershell
python tools/process_icons.py
```

The checked-in generated files are the build inputs. `build.rs` embeds the ICO into Windows executables through `winresource`; the Slint UI embeds the optimized PNGs. Review the 32 px tray result on both light and dark Windows taskbars before release; very small icon rendering and Windows scaling remain desktop-verification items.
