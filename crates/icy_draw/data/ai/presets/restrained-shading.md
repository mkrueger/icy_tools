# Restrained ANSI shading

Original workflow; references for further study:
- Lord Soth's tips: https://www.roysac.com/tutorial/LordSothAnsiTips.html
- Archived advanced picture tutorial: https://16colo.rs/pack/newbie01/ANSI-TUT.014
- Halaster shading notes: https://www.roysac.com/tutorial/ansitut-hal-shade.html
- Tutorial collection: https://github.com/xero/ansi-art-tutorials

First make the unshaded silhouette, proportions and major flat-color regions
readable. Choose a light direction and a small value ramp per material.
Place broad shadow and highlight shapes before transitions. Use shade blocks
at selected boundaries, not as texture covering every flat surface.
Do not assume every color ramp needs black, dark gray and white: strong hues
often need fewer extremes. Let neighboring values and the material determine
the ramp. Match half-block edges to the adjacent shaded value rather than
adding a bright outline to a dark crease.
Prefer compact transitions and preserved flat areas to long muddy gradients.
Check contrast and focal-point readability at normal size. Remove detail or
shading that obscures the form. Treat these as adjustable design heuristics,
not mandatory scene rules. Respect the document's palette and encoding,
and avoid bright backgrounds unless the target supports them without blinking.
