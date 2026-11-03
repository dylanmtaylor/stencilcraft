# Pop-out cutting research

Status: **not implemented; do not send a matless through-cut job as Pop-out Cut**.

The [Cameo 4 user manual](https://makerspace.tulane.edu/images/8/89/Cameo-4-en.pdf)
describes cutting cardstock without a mat by leaving small uncut sections and using
an underlying cutting channel. It requires the correct material size, suitable
stiff material, a compatible machine, and a test cut. Silhouette's [material loading
guidance](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000273078-features-cutting-machines-)
requires both rollers to grip the material evenly; sheets narrower than the roller
spacing require a mat.

The existing `Mat::None` sends ordinary cut paths and does not implement Pop-out Cut.
The user manual does not specify how the cutter is positioned over its cutting
channel, the machine commands used, tab placement or material-specific settings.
Do not assume selecting no mat or splitting a polyline alone reproduces the machine
operation. In particular, do not run full-depth cardstock cuts on this basis.

Before enabling the mode, obtain a clean-room public protocol specification for
channel positioning and settings, implement and test bounded tab planning for open,
closed, short and nested contours, validate sheet dimensions and roller margins,
model supported machines, and expose an explicit reviewed job through engine and UI.
Test the encoded protocol with a mock transport and validate on compatible hardware
with a small test cut. Record what is measured and what remains unknown.
