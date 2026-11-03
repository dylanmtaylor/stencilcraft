# Silhouette Studio documents (.studio, .studio3)

This is our own description of Silhouette Studio's native document files. We learned it
only by **differential analysis of documents we drew and saved ourselves** in Silhouette
Studio 5.0.539 Basic edition (Windows 11 VM, 2026-11-02): save two documents that differ
in one thing, then compare the bytes. We did not disassemble, decompile or debug the app.
We did not read any other project's code for these formats. Posture:
[`docs/reference-apps.md`](../reference-apps.md) (branch `docs/reference-apps`).

Reader: [`crates/studio`](../../crates/studio/src/lib.rs). Status: **read, lossy** (straight-sided
outlines and page size). Writing is not done yet (see [Writing](#writing)).

## Protected designs

Designs bought from the Silhouette Design Store can be protected or encrypted. StencilCraft
never decrypts or works around that protection. A `.studio3` file whose header parses but
whose document section is not in the plain layout below is rejected with "this design is
protected or encrypted … StencilCraft does not open protected designs". We have no
protected sample. The rule is: anything we cannot read as plain records is refused, never
guessed at.

## Samples

None of these files are committed. They contain the app's material-preset text and a
thumbnail. Keep them locally under `corpora/silhouette/`, where the corpus tests pick them up.

| File | sha256 | Document |
| --- | --- | --- |
| `rect-v2.studio` | `7304b266…0210f7` | one rectangle 5.469 × 4.375 in at X 2.456, Y 2.292 in; 12 × 12 in CAMEO 5 page; saved as v2 |
| `rect-v3.studio3` | `8d74c1aa…daacbd` | the same document saved as v3 |
| `empty-v3.studio3` | `51b60e87…41f70` | empty 12 × 12 in page, same app session, v3 |

Pair tags used below: **R2/R3** = rect-v2 vs rect-v3; **E3/R3** = empty-v3 vs rect-v3;
**R** = seen in both rect files, with values checked against the known geometry.

## Conventions

- Little-endian everywhere. (R)
- Geometry is **millimetres** stored as **f32**, origin at the **page's top-left corner**, y
  pointing **down**. Rectangle X 2.456 in is stored as 62.380, Y 2.292 in as 58.213. The page
  is stored as 304.8 × 304.8. (R)
- Strings are a u32 byte count followed by bytes without a terminator, for example
  `06 00 00 00 "square"` and `0a 00 00 00 "Untitled-1"`. (R)
- **Records.** Most structure is nested records: marker byte `0x01`, a u32 length that
  **counts the 4 length bytes but not the marker**, then the payload. The document root
  is one record right after the header. (R; holds for every record walked by hand)
- Fields inside a payload are packed with no alignment. For example, the page width sits
  at byte 0x29 of its record. (R)

## .studio (v2) layout

| Offset | Content | Evidence |
| --- | --- | --- |
| 0 | ASCII `silhouette04;` (13 bytes) | R2 |
| 13 | root record (`01`, u32 length, payload). In R2 it ends at 0x1aad. A second top-level record follows (0x1acc, 300 bytes, holds a 32-hex-digit id) | R2 |

The root's children, in order (only the first two are relied on):

1. A 60-byte record: f32 25.4 (0x41cb3333), u32 4, …, string `"square"`, then RGBA-looking
   f32 quadruples (0.5, 0.5, 0.5, 1.0). Meaning unknown. Could be grid or display settings. (R)
2. The **page record**. At payload +0x00 to +0x28: zeros, then four f32 15.875 (5/8 in,
   possibly margins), f32 20, 0.5, 2.0. At **payload +0x29: f32 page width, f32 page height
   (mm)**. Child records follow: mat name string `"12x12"`, f32 325.2 and 344.8 (12.8 ×
   13.57 in, which looks like the physical mat outline), and more. (R; that width comes
   before height is **assumed**, because only a square page has been seen)
3. Media/material settings: `"Print Paper"`, `"(Light)"`, `"PrintPaperLight"`, a
   description sentence, `"Blue"`, `"adjustable"`. These are the app's preset names. We do
   not interpret or reproduce them. (R)
4. Layer and object tree: `"Default Layer"`, tool/cut-style records (`"cut"`, `"Cut"`,
   `"rounded"`), and the shape (below). (R)
5. A long run of `layer_@cnon:` / `@ccut:` / `@cced:` / `@cper:` / `@cped:` / … records,
   each with `no_tool`, a fixed 32-hex id, a 31-character `2222…A` string, `auto`,
   `no_action`, `studio4.3extension1_…`. These look like per-cut-action tool assignments.
   Not interpreted. (R)

### Shape (path) record

The rectangle is stored as:

```text
01 L1                      path record, L1 = 90
   u32 0                   unknown (path or contour kind?)
   01 L2                   point list, L2 = L1 - 9 = 81
      01 u32 3             unknown: 3 for 4 points (n-1? closed flag + count?)
      n × point:           each 18 bytes
         01 11 00 00 00    a record of length 17
         f32 x, f32 y      page millimetres
         5 bytes           first point 00 00 01 00 00, others all zero (node flags?)
```

The reader accepts a path only when all of this holds: `L1 = L2 + 9`, `L2 = 9 + 18n`, every
point record has length 17, and every coordinate is finite and within ±1 km. It finds paths
by scanning the root record for this exact nested pattern. (R)

The points run (x0,y0) → (x0,y1) → (x1,y1) → (x1,y0), counter-clockwise on screen. There is
no repeated closing point. (R)

Right after the point list come:

- a 3 × 3 f32 matrix `[1 0 -207.27; 0 1 -204.49; 0 0 1]`;
- six f32 `-75.43 -90.71 69.46 0 0 55.57`, which are the rectangle's centre in the shifted
  frame and its half-width and half-height;
- a second copy of the points (`00000000`, a u32 length **without** a `01` marker, `01 u32 3`,
  then 4 × {f32 x, f32 y, u16 0}). These points equal the first list moved by the matrix:
  62.38 − 207.27 = −144.89. (R)

The record then carries the name `"Rectangle"`, a 32-hex id and `"create_rectangle"`
(the tool that made it). (R)

**Which list is authoritative?** The first list matches the document geometry exactly in
page coordinates, so the reader imports it. What the matrix means (the inverse of an object
transform? a pivot frame?) is unknown until the rotated-rectangle sample arrives.

## .studio3 (v3) layout

| Offset | Content | Evidence |
| --- | --- | --- |
| 0 | ASCII `silhouette05;` then 3 zero bytes | R3, E3 |
| 0x10 – 0x3e7 | table of u64 **section lengths**. 14 are non-zero-or-used, the rest 0. **The sum equals file length − 1024** | R3, E3 |
| 0x3e8 | u64 500000539: app version 5.0.539 | R3, E3 |
| 0x3f0, 0x3f8 | u64 300000000 twice: format version 3.0.0? | R3, E3 |
| 0x400 | sections back to back | R3, E3 |

| # | R3 length | E3 length | Content |
| --- | --- | --- | --- |
| 0 | 10974 | 8318 | **the document**: `silhouette05;` and the same record tree as v2. Up to and including the page record it is byte-identical to v2 apart from the root length. The shape record is identical to v2 (R2/R3). It differs between E3 and R3 by the shape (E3/R3) |
| 1 | 17 | 17 | `01 10 00 00 00` … `ff` × 8: unknown |
| 2 | 2443 | 2248 | **PNG thumbnail** |
| 3 | 42656 | 42656 | all `ff` in both: unknown, maybe a reserved bitmap |
| 4 | 392 | 392 | u32 50, then i32s. R3: `-2897 -2925 -119 -702`, which is the shape bounding box in the shifted frame at **20 units/mm** (2778 = 138.9 mm × 20). E3: zeros. Then `6066 6066 1 6066` in both |
| 5, 6 | 50, 54 | same | identical scrambled-looking bytes in both files. Not interpreted |
| 7, 8 | 4, 4 | same | `ff ff ff ff` |
| 9 | 1851 | 1851 | u64 length, then UTF-8 (with BOM) CSV of cut-setting column names (`studio_condition,full_name,…`) |
| 10, 11, 13 | 94 each | same | u64 length 86, then base64 JSON `{"root_uuid":"…","children":{}}` |
| 12 | 0 | 0 | empty |

The reader checks the table (no overflow, no section past end of file), takes section 0,
and requires it to start with `silhouette05;` and `01`. A bare body (`silhouette05;01…`
with no table) is also accepted.

## What the reader imports

- Every path record, as a closed polyline on one `cut` layer. This is a **shortcut**: open
  paths and curves are not yet distinguished (see below).
- Page width and height, rounded to 1 µm. If they are missing or not plausible, the
  default sheet is used and a warning is returned.
- Caps: 64 MiB input, 100,000 shapes, 2,000,000 points.

## Unknowns

- Curves (Bézier nodes): what the 5 trailing point bytes and the `01 u32 3` prefix mean.
  Needs the ellipse sample.
- Open versus closed paths: needs the line and open-polyline samples.
- Object transforms and rotation: what the 3 × 3 matrix means.
- Line and fill colour, layer membership, multiple layers, groups, compound paths, text.
- Page width versus height order (only a square page so far). Letter-page sample needed.
- Sections 1, 3, 5, 6, 7, 8 of v3.
- What protected store designs look like (we have none and will not seek a way to read one).

## Writing

Not implemented. A writer must produce the whole record tree (page, media, layer, tool
tables), and most of its fields are still unknown, so a writer today would be guesswork.
The next step is single-change experiments on our own files: change only the first point
list in a copy of `rect-v3.studio3`, load it in the VM, and see whether the rectangle moves.
That shows which copy of the geometry the app trusts.

## GSD / GSP

These are legacy Silhouette/Graphtec formats. We have no samples yet (requested). Nothing
is known.
