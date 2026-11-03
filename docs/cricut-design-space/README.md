# Cricut Design Space feature library

Design Space needs a Cricut account sign-in before any design screen opens, and
StencilCraft does not bypass sign-in (DS 9.83.76, 2026-11-02). So this library is built
from Cricut's public documentation. Each page describes, in our own words, what a feature
does, its options, where it sits in the interface, and which platforms, machines and
subscription tiers have it, with a source link for each. Nothing here is copied text, a
screenshot, an icon or an image. The clean-room posture is in
[`reference-apps.md`](../reference-apps.md#clean-room-posture), which will hold the
StencilCraft status for each feature.

## How it was gathered (2026-11-02)

- **Primary**: [Cricut Help Center](https://help.cricut.com/hc/en-us) articles. The site
  refuses automated page fetches (HTTP 403). The articles were read through search-engine
  extracts of those pages, so details should be checked against the live article in a
  browser before they drive an implementation.
- **Primary**: the Cricut blog on cricut.com, for launch announcements (Joy 2, Explore 5,
  Create AI, AI Project Designer, Create Sticker).
- **Machine manuals**: Cricut publishes the machine manuals as help-centre quick start
  guides and specification pages. These are cited on [machines](machines.md) and
  [materials and tools](materials-and-tools.md); no separate PDF manuals were found.
- **Not yet covered**:
  - **learn.cricut.com** did not surface in search. The help centre has a "Tutorial Videos"
    page for an introduction to Design Space course, but its lesson list was not readable.
  - **Cricut's official YouTube channel** could not be confirmed from search results. The
    Design Space tutorials found were by independent creators, so they are not recorded
    here as official sources. See *Secondary sources* below.

## Areas

| Area | Page | Features | Sources |
| --- | --- | --- | --- |
| Home and projects | [home-and-projects.md](home-and-projects.md) | 10 | 10 |
| Canvas | [canvas.md](canvas.md) | 10 | 6 |
| Design panel | [design-panel.md](design-panel.md) | 7 | 9 |
| Text | [text.md](text.md) | 11 | 5 |
| Shapes | [shapes.md](shapes.md) | 4 | 3 |
| Images and upload | [images-and-upload.md](images-and-upload.md) | 10 | 8 |
| Edit bar | [edit-bar.md](edit-bar.md) | 16 | 8 |
| Layers panel | [layers-panel.md](layers-panel.md) | 13 | 8 |
| Operations and linetypes | [operations-and-linetypes.md](operations-and-linetypes.md) | 12 (10 operations + 2) | 10 |
| Prepare and Make screens | [make-screen.md](make-screen.md) | 15 | 11 |
| Materials and tools | [materials-and-tools.md](materials-and-tools.md) | 19 (5 + 14 tools) | 17 |
| Print Then Cut | [print-then-cut.md](print-then-cut.md) | 10 | 14 |
| Machines | [machines.md](machines.md) | 12 (10 machines + 2) | 18 |
| Settings and account | [settings-and-account.md](settings-and-account.md) | 10 | 13 |
| Mobile differences | [mobile.md](mobile.md) | 14 | 11 |
| Shortcuts | [shortcuts.md](shortcuts.md) | 8 | 3 |
| Release history | [release-history.md](release-history.md) | 15 entries | 6 |
| **Total** | | **196** | **82 distinct** (77 help centre, 5 blog) |

## Secondary sources

These are independent tutorials, not checked against Cricut. Use them only to fill gaps;
they are not used for any claim above.

- "Cricut Design Space Slice, Weld, Contour, Group & Attach Tools - for Beginners": <https://www.youtube.com/watch?v=dpu3XixLDGk>, layers-panel tools.
- "Cricut Design Space Tutorial - Part 2 - Slice, Weld, Attach, Flatten and Contour Tools": <https://www.youtube.com/watch?v=XNw0h4wCjT4>, layers-panel tools.
- "Slice, Weld, Attach, Flatten — Cricut Side Panel Explained": <https://www.youtube.com/watch?v=RjNkMGInnbk>, layers-panel tools.
- "How to use the Offset Feature in Cricut Design Space": <https://www.youtube.com/watch?v=-HXQmfiBOmE>, Offset.
- "Free Cricut Class! Flatten to Print then Cut, Weld, Slice, Contour!": <https://www.youtube.com/watch?v=6jqGs6ER8Yo>, Flatten and Print Then Cut.
- Cricut blog, "Using the Offset feature": <https://cricut.com/blog/offset-design-space/> (official blog), Offset.
- JenniferMaker, "Cricut weld vs attach": <https://jennifermaker.com/cricut-weld-vs-attach/>, Weld vs Attach.
