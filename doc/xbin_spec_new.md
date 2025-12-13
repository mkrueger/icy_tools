# XBin Format Specification (Extended)

## Overview

XBin (eXtended BIN) is a file format for storing ANSI art with embedded fonts and palettes. Default file extension is `.XB`.

An XBin consists of 4 parts:

1. **Header** (Required, 11 bytes)
2. **Palette** (Optional, 48 bytes)
3. **Font** (Optional, variable size)
4. **Image Data** (Optional, variable size)

---

## 1. Header (11 bytes)

| Offset | Field    | Size | Type    | Description |
|--------|----------|------|---------|-------------|
| 0      | ID       | 4    | Char    | Magic bytes: `"XBIN"` (0x58, 0x42, 0x49, 0x4E) |
| 4      | EOFChar  | 1    | Byte    | CP/M EOF character: `0x1A` (Ctrl-Z) |
| 5      | Width    | 2    | uint16  | Image width in characters (little-endian) |
| 7      | Height   | 2    | uint16  | Image height in characters (little-endian) |
| 9      | FontSize | 1    | Byte    | Font height in pixels (1-32). Default VGA: 16 |
| 10     | Flags    | 1    | Bits    | Feature flags (see below) |

### Flags Byte

| Bit | Name      | Description |
|-----|-----------|-------------|
| 0   | Palette   | 1 = Custom palette present |
| 1   | Font      | 1 = Custom font present |
| 2   | Compress  | 1 = Image data is compressed |
| 3   | NonBlink  | 1 = ICE color mode (16 background colors, no blink) |
| 4   | 512Chars  | 1 = 512 character mode (requires Font=1) |
| 5-7 | Reserved  | Must be 0 |

**Constraints:**

- If `512Chars=1`, then `Font` must also be `1`
- If `Font=0` and `FontSize≠16`, the file is invalid

---

## 2. Palette (48 bytes, optional)

Present only if `Flags.Palette=1`.

The palette consists of 16 RGB triplets (48 bytes total):

```text
Offset  Color   Bytes
0-2     Color 0:  R, G, B
3-5     Color 1:  R, G, B
6-8     Color 2:  R, G, B
...
45-47   Color 15: R, G, B
```

Each R, G, B value ranges from **0 to 63** (6-bit VGA palette format).

To convert to 8-bit RGB: `value_8bit = (value_6bit << 2) | (value_6bit >> 4)`

---

## 3. Font (variable size, optional)

Present only if `Flags.Font=1`.

### Font Structure

The font contains either 256 or 512 character definitions (depending on `Flags.512Chars`).

**Total font size:**

- 256-char mode: `FontSize × 256` bytes
- 512-char mode: `FontSize × 512` bytes (stored as two consecutive 256-char blocks)

### Character Bitmap Encoding

Each character is stored as a **1-bit-per-pixel bitmap**, top-to-bottom, with each row packed into 1 byte (8 pixels wide).

**Single character structure:**

```text
Byte 0:  Row 0 (topmost scanline)
Byte 1:  Row 1
...
Byte N:  Row N (where N = FontSize - 1)
```

**Bit layout within each byte:**

```text
Bit:    7   6   5   4   3   2   1   0
Pixel:  0   1   2   3   4   5   6   7
        ←─────── Left to Right ───────→
```

- Bit value `1` = Foreground color (character pixel)
- Bit value `0` = Background color (transparent)

### Example: 16-pixel high font

For ASCII character 'A' (0x41 = 65):

```text
Offset in font data: 65 × 16 = 1040

Bytes 1040-1055 contain the 16 rows of character 'A':

Byte  Binary      Visual (# = foreground)
1040: 00000000    ........
1041: 00011000    ...##...
1042: 00111100    ..####..
1043: 01100110    .##..##.
1044: 01100110    .##..##.
1045: 01111110    .######.
1046: 01100110    .##..##.
1047: 01100110    .##..##.
1048: 01100110    .##..##.
1049: 00000000    ........
1050: 00000000    ........
1051: 00000000    ........
1052: 00000000    ........
1053: 00000000    ........
1054: 00000000    ........
1055: 00000000    ........
```

### 512-Character Mode

When `Flags.512Chars=1`, two fonts are stored consecutively:

```text
Offset                          Content
0                               Font 0, Char 0
FontSize                        Font 0, Char 1
...
FontSize × 255                  Font 0, Char 255
FontSize × 256                  Font 1, Char 0
FontSize × 257                  Font 1, Char 1
...
FontSize × 511                  Font 1, Char 255
```

**Attribute encoding in 512-char mode:**

The font selection is encoded in bit 3 of the foreground color:

- Foreground colors 0-7: Use Font 0
- Foreground colors 8-15: Use Font 1, actual color = (fg - 8)

This limits 512-char mode to 8 foreground colors per character.

---

## 4. Image Data (variable size, optional)

Present only if `Width > 0` and `Height > 0`.

### Uncompressed Format (Flags.Compress=0)

Raw character/attribute pairs, row by row, left to right:

```text
Total size: Width × Height × 2 bytes

For each position (x, y):
  Byte 0: Character code (0-255)
  Byte 1: Attribute byte
```

### Attribute Byte Layout

**Blink Mode (Flags.NonBlink=0):**

```text
Bit:  7      6  5  4     3  2  1  0
      Blink  Background   Foreground
      
- Bits 0-3: Foreground color (0-15)
- Bits 4-6: Background color (0-7)
- Bit 7:    Blink flag
```

**ICE Mode (Flags.NonBlink=1):**

```text
Bit:  7  6  5  4     3  2  1  0
      Background      Foreground
      
- Bits 0-3: Foreground color (0-15)
- Bits 4-7: Background color (0-15)
```

**512-Char Mode Attribute (both Blink and ICE):**

Bit 3 of the foreground selects the font:

- `fg & 0x08 == 0`: Font 0, foreground = fg
- `fg & 0x08 == 1`: Font 1, foreground = fg - 8

---

## 5. XBin Compression (Flags.Compress=1)

Compression uses Run-Length Encoding with 4 compression types.

**Important:** Compression works **row by row**. Runs do NOT span across line boundaries.

### Compression Byte Format

```text
Bit:  7  6     5  4  3  2  1  0
      Type     Count (0-63)

Type:
  00 = No compression
  01 = Character compression
  10 = Attribute compression
  11 = Full compression (char+attr)

Count: Actual repeat = Count + 1 (range 1-64)
```

### Compression Type 00: No Compression

Used when consecutive char/attr pairs have no pattern.

```text
Format: [00,count] Char0 Attr0 Char1 Attr1 ... CharN AttrN

Data bytes: (count + 1) × 2
```

Example: `AaBbCc` → `[00,2] A a B b C c`

### Compression Type 01: Character Compression

Same character, different attributes.

```text
Format: [01,count] Char Attr0 Attr1 ... AttrN

Data bytes: 1 + (count + 1)
```

Example: `AaAbAc` → `[01,2] A a b c`

### Compression Type 10: Attribute Compression

Different characters, same attribute.

```text
Format: [10,count] Attr Char0 Char1 ... CharN

Data bytes: 1 + (count + 1)

⚠️ Note: Attribute comes BEFORE characters (only case where this happens)
```

Example: `AaBaCa` → `[10,2] a A B C`

### Compression Type 11: Full Compression (RLE)

Identical char+attr pairs repeated.

```text
Format: [11,count] Char Attr

Data bytes: 2 (regardless of count)
```

Example: `AaAaAaAa` → `[11,3] A a`

---

## Complete File Layout

```text
┌─────────────────────────────────────────┐
│ Header (11 bytes, always present)       │
├─────────────────────────────────────────┤
│ Palette (48 bytes, if Flags.Palette=1)  │
├─────────────────────────────────────────┤
│ Font (FontSize×256 or ×512 bytes,       │
│       if Flags.Font=1)                  │
├─────────────────────────────────────────┤
│ Image Data (variable, if W×H > 0)       │
│   - Compressed or uncompressed          │
└─────────────────────────────────────────┘
```

---

## Reference Implementation Notes

### Size Limits

| Field     | Min   | Max      |
|-----------|-------|----------|
| Width     | 0     | 65535    |
| Height    | 0     | 65535    |
| FontSize  | 1     | 32       |

### Common Font Sizes

| FontSize | Description              |
|----------|--------------------------|
| 8        | EGA 8×8 font             |
| 14       | EGA 8×14 font            |
| 16       | VGA 8×16 font (default)  |
| 19       | VGA 8×19 font            |

### Default Values (when flags not set)

- **Palette:** Standard 16-color DOS/VGA palette
- **Font:** Standard VGA 8×16 CP437 font
- **FontSize:** 16 (must be 16 if Font flag is 0)

---

## Version History

- **1996:** Original XBin specification by Tasmaniac (ACiD)
- **2025:** Extended documentation with font encoding details

## References

- Original spec: <https://web.archive.org/web/20120204063040/http://www.acid.org/info/xbin/xbin.htm>
- XBin Tutorial: <https://web.archive.org/web/20120204063045/http://www.acid.org/info/xbin/xbintut.htm>
