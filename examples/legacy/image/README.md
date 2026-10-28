# Example image fixtures

The geometric PNG, animated GIF, circle-arrow SVG and colored-shapes SVG are
original fixtures generated for WGPUI under Apache-2.0. Regenerate them with
`python3 examples/legacy/image/generate_fixtures.py`; the script uses only the
Python standard library. `fixture-inventory.json` records their reproducible
hashes.

The PNG is 512×512 with transparent, translucent and opaque regions. The GIF is
500×500 with 25 frames at 40ms and an infinite loop. The arrow is a 16×16
`currentColor` SVG; the 480×480 SVG contains distinct colors and transparency.
These preserve the image-loading, animation, opacity, scaling and SVG examples.

The replaced app-icon, cat animation and earlier SVGs remain recoverable in Git
at commit `0222ee37f267a4126af910b95af74600783c7fcb`; their permissions were not
established by the release audit. They are not shipped in the current package.
The separate [Dragon illustration](../svg/NOTICE.txt) retains CC-BY-SA-3.0 and
its own author/source attribution.
