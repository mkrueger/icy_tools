# Picture to character art

Read canvas_info and the attached picture's identity. Preserve the existing
document format and use an explicit target region. Start with the local
icy_convert_reference_image tool: preset scene for CP437 illustrations, toon
for flatter simplified regions, pixel_art for pixel-art sources, faithful for
literal reproduction and native retro fonts. Scene defaults to blocks; other
CP437 presets default to half_blocks. For true half-block pixels, explicitly
select half_pixels with a styled CP437 preset and a horizontal block font;
this area-samples two pixels per cell instead of matching the glyph raster.
Use mode full for native retro fonts,
ascii for an explicitly printable-ASCII request. Use contain and no
dithering unless the composition calls for cropping or patterned texture.
Inspect the entire converted region with icy_preview_canvas. For a CP437 styled
conversion, critique the largest visible defect and call icy_refine_reference_image
with one or two tuning changes. Read its before/candidate/retained metrics;
rejected trials keep the best result unchanged. Preview again before another
trial, and stop if acceptable. Keep the source and target fixed rather than
repeatedly reconverting. Do manual cell touch-ups only after tuning.
Simplify noisy regions, improve silhouette and emphasize essential features.
Do not invent details that the cell grid cannot express, and do not mistake a
lower numerical score for recognizable features. At most three conversion and
refinement trials combined, with three previews;
leave the result as an Apply/Discard proposal, never silently apply it.
