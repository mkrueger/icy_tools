# Picture to character art

Read canvas_info and the attached picture's identity. Preserve the existing
document format and use an explicit target region. Start with the local
icy_convert_reference_image tool: half_blocks for CP437, full for native retro
fonts, ascii for an explicitly printable-ASCII request. Use contain and no
dithering unless the composition calls for cropping or patterned texture.
Inspect the rendered draft with icy_preview_canvas; simplify noisy regions,
improve silhouette and emphasize essential features. Do not invent details
that the cell grid cannot express. At most three conversion/preview passes;
leave the result as an Apply/Discard proposal, never silently apply it.
