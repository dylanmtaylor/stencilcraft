# Silhouette Studio documents (.studio, .studio3)

This is our own description of Silhouette Studio's native document files. We learned it
only by **differential analysis of documents we drew and saved ourselves** in Silhouette
Studio 5.0.539 Basic edition (Windows 11 VM, 2026-11-02): save two documents that differ
in one thing, then compare the bytes. We did not disassemble, decompile or debug the app.
We did not read any other project's code for these formats. Posture:
[`docs/reference-apps.md`](../reference-apps.md) (branch `docs/reference-apps`).

Reader: [`crates/studio`](../../crates/studio/src/lib.rs). Status: **read, lossy** (outlines,
both straight and Bézier, with each object's transform applied, plus page size). Writing is not done yet (see [Writing](#writing)).

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
| `rect-1in-at-1-1.studio` / `.studio3` | `bebfc1b0…4e0739` / `ff4315a1…c24291` | rectangle W 1 H 1 at X 1 Y 1 in, checked on screen |
| `rect-1in-at-2-1.studio` / `.studio3` | `1188097c…05f2e1` / `dcf11d30…dcda3f` | the same rectangle at X 2 (only X differs) |
| `rect-2x1in-at-1-1.studio` / `.studio3` | `77ebadc4…203c3e` / `3f0c9b51…19e416` | W 2 H 1 at X 1 Y 1 (only W differs) |
| `ellipse-2x1in-at-1-1.studio` / `.studio3` | `aa73e855…d6be6a` / `70abbfb1…277b4b` | ellipse W 2 H 1 at X 1 Y 1 |

Pair tags used below: **R2/R3** = rect-v2 vs rect-v3; **E3/R3** = empty-v3 vs rect-v3;
**R** = seen in both rect files, with values checked against the known geometry;
**X** = rect-1in-at-1-1 vs rect-1in-at-2-1 (only X); **W** = rect-1in-at-1-1 vs
rect-2x1in-at-1-1 (only W); **C** = ellipse vs rect-2x1in-at-1-1 (same box, curve versus
corner nodes).

## Conventions

- Little-endian everywhere. (R)
- Geometry is **millimetres** stored as **f32**, origin at the **page's top-left corner**, y
  pointing **down**. The page is stored as 304.8 × 304.8. A shape's nodes are kept in the
  frame where the shape was first drawn. A per-object 3 × 3 matrix maps them onto the page,
  and moving or resizing changes only the matrix, never the nodes. In X and W the node
  coordinates stay the same and the matrix changes. Applying the matrix gives exactly the
  on-screen X/Y/W/H. (X, W, C)
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

```text
01 L1                      path record
   u32 0                   unknown (contour kind?)
   01 L2                   node list, L2 = L1 - 9
      01 u32 3             unknown: 3 for both 4-node shapes (n-1? closed flag + count?)
      n × node:
         01 len            len 17 (no handles) or 33 (two handles)          (R, C)
         f32 x, f32 y      node, in the drawing frame (mm)
         in handle         00 = none | 01 f32 dx f32 dy, relative to the node (C)
         out handle        same encoding
         3 bytes           rect 00 00 00 (rect-v2 first node 01 00 00), ellipse 00 01 00: unknown
f32 × 9                    object matrix, row-major: x' = a x + b y + c, y' = d x + e y + f;
                           last row 0 0 1                                   (X, W)
f32 × 6                    cx cy a b c d: the on-page frame. Centre plus the half-size axes:
                           rect-1in-at-1-1 gives 38.1 38.1 12.7 0 0 12.7    (X, W)
01|02 u32 0                unknown (01 in .studio, 02 in .studio3)
01 L3 u32 0 u32 L4 01 u32 3  second node list: n × {f32 x, f32 y, in handle, out handle}
                           (10 bytes for corners, 26 for curves) = the nodes after the matrix
"Rectangle"/"Ellipse", 32-hex id, "create_rectangle" (the tool used), …
```

- Rectangle nodes run (x0,y0) → (x0,y1) → (x1,y1) → (x1,y0). There is no repeated closing
  node. (R)
- Ellipse: 4 nodes (right, bottom, left, top), each with both handles. The handle length is
  0.5523 × the radius, the usual cubic circle constant: 12.265 = 22.2 × 0.5523. The in handle
  points back along the incoming segment, the out handle forward. (C)
- Segment i → i+1 is a cubic with control points `node_i + out_i` and `node_{i+1} + in_{i+1}`.
  With no handles at either end it is a straight line. (C)
- Moving by 1 in (X) changes only matrix `c` (and the frame centre). Doubling the width (W)
  changes matrix `a` and `c`. The nodes are byte-identical in X and W, but in rect-1in-at-1-1
  they differ from the other two (that rectangle was drawn separately). (X, W)
- The reader accepts a path only when every length agrees (`L1 = L2 + 9`, node lengths match
  their handle flags), every coordinate is finite and within ±1 km, and the matrix is affine.
  It finds paths by scanning the root record for this nested pattern and applies the matrix.

**rect-v2/rect-v3 puzzle.** Applying their matrix puts the rectangle at about (−144.9, −146.3)
mm, which is off the page. Their raw nodes, by contrast, match the X 2.456, Y 2.292 in we were
told. The later on-screen-verified samples follow the matrix rule exactly, so the reader
follows it. The first pair needs re-checking in the VM (it was not verified on screen).

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

- Every path record as a closed path (lines and cubics) on one `cut` layer, after its object
  matrix is applied. This is a **shortcut**: open paths are not yet told apart (see below).
- Page width and height, rounded to 1 µm. If they are missing or not plausible, the
  default sheet is used and a warning is returned.
- Caps: 64 MiB input, 100,000 shapes, 2,000,000 points.

## Unknowns

- The 3 trailing node bytes, and the `01 u32 3` list prefix.
- Open versus closed paths: needs the line and open-polyline samples.
- Rotation: which matrix entries a rotation changes (needs the rotated-rectangle sample).
- Where rect-v2/rect-v3 really sat on the page (see the puzzle above).
- v3 embeds precomputed cut data ("Include Cut Data", on by default). Which section holds it
  is not yet identified; section 4's bounding box may be part of it.
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
