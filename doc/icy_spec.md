# ICY / ICED (“Icy Draw”) Format — Technical Specification

## Overview

The **ICY** format (also called **ICED** after the header keyword) is the native project/document format used by IcyDraw/icy_engine.

An `.icy` file is always a **PNG image (RGBA, 8-bit)** that contains:

- a **preview image** (rendered snapshot; not canonical), and
- **canonical document data** stored in PNG chunks.

There are two metadata encodings in the wild:

- **v1 (current)**: custom PNG chunk type `icYD` containing binary records (optionally Zstd-compressed).
- **v0 (legacy)**: PNG `tEXt`/`zTXt` chunks containing Base64 of binary payloads.

Reference implementation:

- v1: `crates/icy_engine/src/formats/io/icy_draw.rs`
- v0: `crates/icy_engine/src/formats/io/icy_draw_v0.rs`

---

## 1. PNG container

- PNG ColorType: `RGBA`
- BitDepth: `8`
- The pixel data is a preview render and may be cropped.
  - The current implementation limits preview rendering to a window of up to `MAX_LINES = 80` text lines.

---

## 2. Common encodings

### 2.1 Little-endian

All integer values in binary payloads are **little-endian**, unless stated otherwise.

### 2.2 UTF-8 string

Strings are encoded as:

```text
u32_le byte_len
byte[byte_len] utf8
```

- `byte_len` is the number of UTF-8 bytes.
- No null terminator.

---

## 3. v1 (current): `icYD` binary records

### 3.1 Metadata transport (`icYD` chunk)

In v1, ICY metadata is stored in a custom PNG chunk type:

- Chunk type: `icYD` (4 bytes)
- Chunk payload: a self-delimiting record

Record layout:

```text
u8     record_version   // currently 1
u16_le keyword_len
byte[keyword_len] keyword_utf8
u32_le data_len
byte[data_len] data
```

Notes:

- `keyword` is UTF-8 in the current implementation.
- `data` may be compressed (see `ICED.type.compression`).

### 3.2 Chunk/keyword overview (v1)

All keywords below are stored as `icYD` records.

| Keyword | Required | Compression | Content |
| --- | ---: | ---: | --- |
| `ICED` | yes | never | Document header (version/modes/canvas size/font dims, plus compression settings) |
| `PALETTE` | optional | yes* | Palette in “ICE Palette” text format |
| `SAUCE` | optional | yes* | Raw `icy_sauce::SauceRecord` bytes |
| `FONT` | optional | yes* | Font slot + name + PSF2 bytes |
| `LAYER` | yes* | yes* | Layer header + layer payload (text or image) |
| `TAG` | optional | yes* | Tag list |
| `END` | yes | never | Terminator |

\* The writer currently compresses everything except `ICED` and `END` using the compression specified in `ICED`.

Implementation notes:

- v1 does not use `LAYER_<n>` keywords; layers are emitted as multiple `LAYER` records (order matters).
- v1 does not use continuation records in the current implementation.

Ordering rules (strict):

- The first `icYD` record **must** be `ICED`.
- `ICED` **must** appear exactly once.
- Readers should stop interpreting metadata after `END`.

### 3.3 `ICED` header (v1)

The `ICED` record contains a fixed-size binary header.

Header size: **20 bytes**.

Layout:

| Offset | Field | Type | Size | Description |
| ---: | --- | --- | ---: | --- |
| 0 | `version` | u16_le | 2 | Format version (currently `1`) |
| 2 | `type` | byte[4] | 4 | Type field: `[compression:u8][sixel_format:u8][reserved:u16]` |
| 6 | `buffer_type` | u16_le | 2 | `BufferType::to_byte()` stored as u16 |
| 8 | `ice_mode` | u8 | 1 | `IceMode::to_byte()` |
| 9 | `font_mode` | u8 | 1 | `FontMode::to_byte()` |
| 10 | `width_chars` | u32_le | 4 | Canvas width in characters |
| 14 | `height_chars` | u32_le | 4 | Canvas height in characters |
| 18 | `font_width_px` | u8 | 1 | Font cell width in pixels |
| 19 | `font_height_px` | u8 | 1 | Font cell height in pixels |

Type field enums:

- `compression`:
  - `0` = none
  - `2` = zstd
- `sixel_format`:
  - `0` = raw RGBA (reserved)
  - `1` = embedded PNG (used by writer)

### 3.4 `PALETTE` record (v1)

`PALETTE` contains a palette in the **ICE Palette** text format (UTF-8 text), as produced by `Palette::export_palette(PaletteFormat::Ice)`.

Example header:

```text
ICE Palette
#Palette Name: ...
#Author: ...
#Description: ...
#Colors: <n>
RRGGBB
...
```

- Color values are **hex** per line (`RRGGBB`).
- Optional color names are stored as `#Name: <...>` directly before the corresponding color line.

### 3.5 `FONT` record (v1)

Each `FONT` record contains:

```text
u8     slot
string name
byte[] psf2
```

- `psf2` is a complete PSF2 font file as returned by `BitFont::to_psf2_bytes()`.

### 3.6 `LAYER` record (v1)

Each `LAYER` record contains a layer header followed by payload.

Layer header layout:

| Order | Field | Type | Size | Description |
| ---: | --- | --- | ---: | --- |
| 1 | `title` | string | var | Layer title |
| 2 | `role` | u8 | 1 | `0` = Normal/Text, `1` = Image |
| 3 | `reserved0` | byte[4] | 4 | unused (0) |
| 4 | `mode` | u8 | 1 | `0` = Normal, `1` = Chars, `2` = Attributes |
| 5 | `layer_color_rgba` | byte[4] | 4 | If `A == 0xFF`: color is set (RGB); else “None” |
| 6 | `flags` | u32_le | 4 | Bitflags (see below) |
| 7 | `offset_x` | i32_le | 4 | Layer offset X |
| 8 | `offset_y` | i32_le | 4 | Layer offset Y |

Flags (`u32`):

| Bit | Name | Meaning |
| ---: | --- | --- |
| 0 | `IS_VISIBLE` | Layer is visible |
| 1 | `POS_LOCK` | Position locked |
| 2 | `EDIT_LOCK` | Editing locked |
| 3 | `HAS_ALPHA` | Has alpha channel |
| 4 | `ALPHA_LOCKED` | Alpha locked |

#### 3.6.1 Text layer payload (role = 0)

```text
u8     transparency
i32_le width
i32_le height
repeated (width * height) times:
  u32_le ch
  text_attribute
```

- `ch` is intended to be a Unicode scalar value. Readers should treat invalid values robustly (current loader maps invalid code points to U+FFFD).

##### `text_attribute` encoding

`text_attribute` is encoded exactly as `TextAttribute::encode_attribute()`:

```text
attribute_color foreground
attribute_color background
u8     font_page
u16_le attr_flags
```

`attribute_color` is a small tagged encoding:

```text
u8 tag
switch tag:
  0:            // Transparent
    // no further bytes
  1..=16:       // Palette index = tag-1
    // no further bytes
  17:           // Extended palette
    u8 index
  18:           // RGB
    u8 r
    u8 g
    u8 b
```

Notes:

- `attr_flags` contains the styling flags and may include I/O markers such as `INVISIBLE`/`INVISIBLE_SHORT`.
- Unlike legacy v0, v1 currently writes a full `width*height` grid (no end-of-line sentinel compression).

#### 3.6.2 Image layer payload (role = 1)

In v1, image layers embed the pixel data as a PNG for better compression.

```text
u64_le embedded_png_len
i32_le width
i32_le height
i32_le vertical_scale
i32_le horizontal_scale
byte[embedded_png_len] embedded_png
```

- The embedded PNG is decoded to RGBA pixel data on load.

### 3.7 `TAG` record (v1)

`TAG` contains a list of tags:

```text
u16_le tag_count
repeated tag_count times:
  string preview
  string replacement_value
  i32_le x
  i32_le y
  u16_le length
  u8 enabled          // 0/1
  u8 alignment        // 0=Left, 1=Center, 2=Right
  u8 placement        // 0=InText, 1=WithGotoXY
  u8 role             // 0=Displaycode, 1=Hyperlink
  text_attribute      // same encoding as in layers
```

### 3.8 `END`

`END` is a record with empty payload and terminates metadata parsing.

---

## 4. v0 (legacy): `tEXt`/`zTXt` Base64 chunks

Legacy files may store metadata in PNG `tEXt`/`zTXt` chunks:

- Keyword: ASCII/Latin-1 (PNG standard)
- Text payload: Base64 of binary payload (or empty for `END`)

This legacy variant uses keywords like `LAYER_<n>`, `LAYER_<n>~<k>`, and `FONT_<slot>` and applies additional wire-level compression tricks (e.g. end-of-line sentinels for invisible cells).

For the authoritative legacy decoding rules, see `crates/icy_engine/src/formats/io/icy_draw_v0.rs`.

---

## 5. Robustness / error cases

- Unknown keywords should be ignored (warn/log).
- If record parsing fails (truncation/invalid lengths), reject the file.
- v1: if decompression fails, reject the file.
- `END` terminates metadata parsing (the PNG image data remains only a preview).

---

## 6. Compatibility notes

- The PNG pixel content is **not** the canonical data source of the document; it is a preview.
- Canonical data is stored in `ICED` + subsequent records.
