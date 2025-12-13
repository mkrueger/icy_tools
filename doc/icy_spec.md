# ICY / ICED (“Icy Draw”) Format — Technical Specification

## Overview

The **ICY** format (also referred to as “ICED”, after the chunk keyword) is the native project/document format used by IcyDraw/icy_engine.

Important: a `.icy` file is **a PNG image (RGBA, 8-bit)** with additional metadata stored as **PNG `zTXt` chunks**.

- Container: PNG
- Pixel data: RGBA preview (a rendered snapshot of the buffer)
- Metadata: `zTXt` chunks with keywords like `ICED`, `LAYER_0`, `FONT_0`, …

Reference implementation: `crates/icy_engine/src/formats/io/icy_draw.rs`.

---

## 1. PNG Container

- PNG ColorType: `RGBA`
- BitDepth: `8`
- The pixel image is a preview render and may be limited to a viewport.
  - Currently the preview render is limited to a window of up to `MAX_LINES = 80` text lines (see implementation).

### Metadata transport (`zTXt`)

All ICY metadata is stored in PNG `zTXt` chunks.

- Keyword: ASCII/Latin-1 (PNG standard)
- Text payload: **Base64 (STANDARD)** of a **binary payload** (or an empty string for `END`)

If Base64 decoding fails, the chunk is typically skipped during loading (warning/log).

---

## 2. Chunk overview

| Keyword | Required | Content |
|---|---:|---|
| `ICED` | yes | Document header (version/modes/canvas size, optional font dimensions) |
| `LAYER_<n>` | yes* | Layer header + payload (text layer or image layer) |
| `LAYER_<n>~<k>` | optional | Continuation of a layer’s payload (no layer header) |
| `FONT_<slot>` | optional | Font definition (PSF2) + name |
| `PALETTE` | optional | Palette in “ICE Palette” text format |
| `SAUCE` | optional | SAUCE record (binary) |
| `TAG` | optional | Tag list (preview/replacement/position/attributes/flags) |
| `END` | yes | Terminator (empty `zTXt` text) |

\* In practice at least `LAYER_0` is expected, because documents consist of layers.

---

## 3. Common encodings

### 3.1 Little-endian

All integer values in binary payloads are **little-endian**.

### 3.2 UTF-8 string encoding

Strings are encoded as:

```text
u32_le byte_len
byte[byte_len] utf8
```

- `byte_len` is the number of UTF-8 bytes.
- No null terminator.

---

## 4. `ICED` header

The `ICED` chunk contains a binary header structure.

Two variants exist based on payload length:

- **V0**: `ICED_HEADER_SIZEV0 = 19` bytes
- **V1**: `ICED_HEADER_SIZE = 21` bytes (adds font dimensions)

### 4.1 Layout

| Offset | Field | Type | Size | Description |
|---:|---|---|---:|---|
| 0 | `version` | u16_le | 2 | Format version (currently `1`) |
| 2 | `type` | u32_le | 4 | Document type (currently unused/0) |
| 6 | `buffer_type` | u16_le | 2 | `BufferType::to_byte()` stored as u16 |
| 8 | `ice_mode` | u8 | 1 | `IceMode::to_byte()` |
| 9 | `palette_mode` | u8 | 1 | `PaletteMode::to_byte()` |
| 10 | `font_mode` | u8 | 1 | `FontMode::to_byte()` |
| 11 | `width_chars` | u32_le | 4 | Canvas width in characters |
| 15 | `height_chars` | u32_le | 4 | Canvas height in characters |
| 19 | `font_width_px` | u8 | 1 | (V1 only) Font cell width in pixels |
| 20 | `font_height_px` | u8 | 1 | (V1 only) Font cell height in pixels |

---

## 5. `PALETTE` chunk

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

---

## 6. `FONT_<slot>` chunks

Each `FONT_<slot>` chunk contains:

1. Font name (UTF-8 string, see §3.2)
2. Font data in **PSF2** format (`BitFont::to_psf2_bytes()`)

```text
string name
byte[] psf2
```

- `<slot>` is an integer (e.g. `FONT_0`, `FONT_1`, …).

---

## 7. `LAYER_<n>`: layer header

Each layer begins with a header (part of the `LAYER_<n>` chunk). Continuation chunks `LAYER_<n>~<k>` contain **payload only**, without the layer header.

### 7.1 Layer header layout

| Order | Field | Type | Size | Description |
|---:|---|---|---:|---|
| 1 | `title` | string | var | Layer title |
| 2 | `role` | u8 | 1 | `0` = Normal, `1` = Image |
| 3 | `reserved0` | byte[4] | 4 | unused (0) |
| 4 | `mode` | u8 | 1 | `0` = Normal, `1` = Chars, `2` = Attributes |
| 5 | `layer_color_rgba` | byte[4] | 4 | If alpha != 0: color is set (RGB), else “None” |
| 6 | `flags` | u32_le | 4 | Bitflags (see below) |
| 7 | `transparency` | u8 | 1 | Layer transparency |
| 8 | `offset_x` | i32_le | 4 | Layer offset X |
| 9 | `offset_y` | i32_le | 4 | Layer offset Y |
| 10 | `width` | i32_le | 4 | Layer width in characters |
| 11 | `height` | i32_le | 4 | Layer height in characters |
| 12 | `default_font_page` | u16_le | 2 | Default font page |
| 13 | `payload_len` | u64_le | 8 | Length of the following payload in the first chunk |

**Flags (`u32`)**:

| Bit | Name | Meaning |
|---:|---|---|
| 0 | `IS_VISIBLE` | Layer is visible |
| 1 | `POS_LOCK` | Position locked |
| 2 | `EDIT_LOCK` | Editing locked |
| 3 | `HAS_ALPHA` | Has alpha channel |
| 4 | `ALPHA_LOCKED` | Alpha locked |

---

## 8. Layer payload (Role = Normal/Text)

### 8.1 Principle

The payload encodes layer cells scanline by scanline:

- Order: `y = 0..height-1`, for each line `x = 0..width-1`
- Per cell, an `attr: u16_le` is written first.
- Depending on `attr`, additional bytes may follow (char/colors/font).

### 8.2 Attribute flags

`attr` is based on `TextAttribute.attr` (u16) and uses additional persistence flags:

- `INVISIBLE = 0x8000`: Cell is invisible (default/empty)
- `SHORT_DATA = 0x4000`: Cell data can be stored in compact form
- `INVISIBLE_SHORT = 0xC000`: End-of-line sentinel (see below)

Note: On load, `SHORT_DATA` is removed from `attr` and used only as an encoding indicator.

### 8.3 End-of-line sentinel

To avoid storing trailing invisible cells, a line can end early:

- If `INVISIBLE_SHORT (0xC000)` is read, the line ends immediately.
- Remaining cells not encoded remain invisible.

### 8.4 Cell encoding

#### A) Invisible cell

```text
u16_le attr == INVISIBLE
// no further bytes
```

#### B) Short cell (if `attr & SHORT_DATA != 0`)

```text
u16_le attr_with_flag
u8    ch       // only 0..255
u8    fg       // only 0..255
u8    bg       // only 0..255
u8    font_page
// ext_attr is implicitly 0
```

#### C) Long cell (if `attr & SHORT_DATA == 0` and not invisible)

```text
u16_le attr
u32_le ch
u32_le fg
u32_le bg
u8     font_page
u8     ext_attr
```

- On load, `ch` is interpreted via `char::from_u32_unchecked` (so invalid Unicode code points are theoretically possible).

### 8.5 Continuations in `LAYER_<n>~<k>`

If a text layer’s payload does not fit into a single chunk, additional lines are written as raw continuations in `LAYER_<n>~1`, `~2`, …

- Continuation chunks contain **only** the continued cell stream (no layer header, no `payload_len`).

---

## 9. Layer payload (Role = Image/Sixel)

For `role = 1`, the payload is:

```text
i32_le width
i32_le height
i32_le vertical_scale
i32_le horizontal_scale
byte[] picture_data
```

- The first `LAYER_<n>` chunk contains the layer header plus these fields.
- `LAYER_<n>~<k>` contains only additional RGBA `picture_data` bytes.

---

## 10. `TAG` chunk

`TAG` contains a list of tags.

### 10.1 Layout

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
  u16_le attr_flags   // may include SHORT_DATA
  (short|long) attribute payload
  byte[16] reserved   // 4x u32 0 (future use)
```

#### Attribute payload (short) (if `attr_flags & SHORT_DATA != 0`)

```text
u8 fg
u8 bg
u8 font_page
```

#### Attribute payload (long)

```text
u32_le fg
u32_le bg
u16_le font_page
```

Note: `ext_attr` is currently not persisted for tags (loader sets `ext_attr = 0`).

---

## 11. `SAUCE` chunk

Optional chunk containing the raw bytes of an `icy_sauce::SauceRecord`.

- The binary encoding is defined by the `icy_sauce` crate.

---

## 12. Robustness / error cases

- Unknown `zTXt` keywords are ignored (warning/log).
- Base64 decoding failures typically cause the chunk to be skipped.
- `END` terminates metadata parsing (subsequent PNG data is not interpreted as ICY metadata).

---

## 13. Compatibility notes

- The PNG pixel content is **not** the canonical data source of the document; it is a preview.
- The canonical data is stored in the `ICED` / `LAYER_*` / `FONT_*` / `PALETTE` / `TAG` / `SAUCE` chunks.
