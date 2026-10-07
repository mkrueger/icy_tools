# RIP BBS menu and graphics workflow

Original Icy Draw workflow. Start with icy_rip_info and icy_rip_api, then read
the existing scene in bounded command pages.

1. Confirm the BBS/terminal supports RIP. For an unspecified BBS use Icy Board
   and state the assumption; historical PCBoard remains a distinct target.
2. Plan a 640x350 pixel layout with title, readable menu groups and a text
   window where needed. Extract required keys, labels and actual host commands.
3. Use stateful palette/font/fill commands followed by a small number of
   geometric primitives. Prefer coherent vector-like batches over pixel loops.
4. Keep button and mouse-region actions consistent with actual board
   configuration; visual labels alone do not make a working clickable menu.
5. Preserve existing commands and any read-only mixed-stream prefix. Keep
   wire parameters fixed-width base-36; new source is ASCII.
6. Use picture/file references only as read-only design inputs. Do not add
   external icon/scene loads, file transfers or other unsupported commands.
7. Review the rendered draft for bounds, contrast, command coverage and
   preserved drawing state. Describe the changes; the user must Apply them.
