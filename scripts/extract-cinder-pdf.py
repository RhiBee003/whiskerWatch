#!/usr/bin/env python3
"""Extract cinderanimate.pdf pages to static/images/cinder/png/frame-NN.png."""

from __future__ import annotations

import sys
from pathlib import Path

try:
    import fitz  # PyMuPDF
except ImportError:
    print("Install PyMuPDF: pip install pymupdf", file=sys.stderr)
    sys.exit(1)

NAMES = [
    "idle",
    "blink",
    "bow-1",
    "bow-2",
    "walk-1",
    "walk-2",
    "walk-3",
    "walk-4",
]


def main() -> None:
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} cinderanimate.pdf", file=sys.stderr)
        sys.exit(1)

    pdf_path = Path(sys.argv[1]).expanduser().resolve()
    out_dir = Path(__file__).resolve().parents[1] / "static" / "images" / "cinder" / "png"
    out_dir.mkdir(parents=True, exist_ok=True)

    doc = fitz.open(pdf_path)
    for i, page in enumerate(doc):
        name = NAMES[i] if i < len(NAMES) else f"frame-{i:02d}"
        pix = page.get_pixmap(matrix=fitz.Matrix(2, 2), alpha=True)
        target = out_dir / f"{name}.png"
        pix.save(target)
        print(f"Wrote {target}")

    if len(doc) > len(NAMES):
        print(f"Note: PDF has {len(doc)} pages; named first {len(NAMES)} frames.")
    doc.close()


if __name__ == "__main__":
    main()
