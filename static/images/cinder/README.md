# Cinder virtual pet frames

`cinderanimate.pdf` was not found in the repo, workspace, or a system-wide search. These SVG placeholders stand in until you export real frames from the PDF.

## Replace with PDF frames

When you have `cinderanimate.pdf`, export each page or animation frame as PNG (128×128 or 256×256 recommended) with the same base names:

| File | Pose |
|------|------|
| `idle.svg` / `idle.png` | Standing, eyes open |
| `blink.svg` / `blink.png` | Eyes closed |
| `bow-1.svg`, `bow-2.svg` | Play bow sequence |
| `walk-1.svg` … `walk-4.svg` | Walk cycle |

### macOS (Poppler)

```bash
brew install poppler
mkdir -p static/images/cinder/png
pdftoppm -png -r 144 cinderanimate.pdf static/images/cinder/png/page
# Rename page-01.png … to idle.png, blink.png, etc., then update paths in static/cinder-pet.js
```

### macOS (ImageMagick)

```bash
brew install imagemagick
magick -density 144 cinderanimate.pdf static/images/cinder/png/frame-%02d.png
```

### Python (PyMuPDF)

```bash
pip install pymupdf
python3 scripts/extract-cinder-pdf.py cinderanimate.pdf
```

After exporting, either keep `.png` and change `FRAME_PATHS` in `static/cinder-pet.js`, or replace the `.svg` files in this folder.
