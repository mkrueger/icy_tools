# icy_sixel: Persistente Sixel-Farbregister

## Ziel

`icy_sixel` braucht einen wiederverwendbaren Decoderzustand, der die 256 Sixel-Farbregister ueber mehrere Bilder hinweg behaelt.

Das wird fuer DEC Private Mode 1070 benoetigt:

- `CSI ? 1070 l`: gemeinsame (shared) Farbregister; Definitionen bleiben fuer folgende Sixel-Bilder erhalten.
- `CSI ? 1070 h`: private Farbregister; jedes Bild beginnt mit der Standardpalette.

Vollbild-Tueren wie SyncMOO, SyncDoom und SyncDuke senden die komplette Palette nur, wenn sie sich aendert. Folgeframes referenzieren vorhandene Register lediglich mit `#n`. Wenn der Decoder fuer jedes Bild eine neue Palette erzeugt, werden diese Frames transparent oder falsch gefaerbt.

DECSDM Mode 80 ist nicht Teil dieser Aenderung. Positionierung und Sixel-Scrolling behandelt der aufrufende Terminalemulator.

## Benoetigte oeffentliche API

Empfohlene API:

```rust
#[derive(Clone, Debug)]
pub struct SixelDecoder {
    palette: Palette,
}

impl Default for SixelDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SixelDecoder {
    /// Erzeugt einen Decoder mit der Sixel-Standardpalette.
    pub fn new() -> Self;

    /// Dekodiert ein einzelnes DCS-Payload.
    ///
    /// Canvas, Cursorposition, Wiederholungszaehler, Rastergroesse,
    /// Hintergrundmodus und aktuelle Farbe sind bildlokal. Nur die
    /// Farbregister werden aus dem Decoder uebernommen und nach erfolgreicher
    /// Dekodierung zurueckgeschrieben.
    pub fn decode_from_dcs(
        &mut self,
        payload: &[u8],
        settings: DcsSettings,
    ) -> Result<SixelImage>;

    /// Dekodiert eine vollstaendige ANSI-DCS-Sequenz.
    pub fn decode(&mut self, data: &[u8]) -> Result<SixelImage>;

    /// Setzt alle Farbregister auf die Sixel-Standardpalette zurueck.
    pub fn reset_palette(&mut self);
}
```

Die bestehende zustandslose API muss kompatibel bleiben:

```rust
impl SixelImage {
    pub fn decode_from_dcs(
        payload: &[u8],
        settings: DcsSettings,
    ) -> Result<Self> {
        SixelDecoder::new().decode_from_dcs(payload, settings)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        SixelDecoder::new().decode(data)
    }
}
```

Alternativ kann der Typ `SixelDecoderState` heissen. Wichtig ist, dass der Aufrufer eine Decoderinstanz ueber mehrere Bilder hinweg behalten kann.

## Interner Umbau

Der aktuelle private `SixelDecoder` in `decoder.rs` enthaelt sowohl persistenten als auch bildlokalen Zustand. Diese Verantwortlichkeiten sollten getrennt werden.

```rust
pub struct SixelDecoder {
    palette: Palette,
}

struct FrameDecoder {
    canvas: Canvas,
    palette: Palette,
    color_index: usize,
    current_color: [u8; 4],
    repeat: usize,
    pos_x: usize,
    pos_y: usize,
    max_x: usize,
    max_y: usize,
    pan: usize,
    pad: usize,
    target_width: usize,
    target_height: usize,
    background_index: usize,
    transparent_mode: bool,
}
```

`Palette` braucht `Clone`:

```rust
#[derive(Clone, Debug)]
struct Palette {
    colors: [u32; SIXEL_PALETTE_MAX],
}
```

Ablauf eines Decode-Aufrufs:

1. `FrameDecoder` mit einer Kopie der persistenten Palette erzeugen.
2. DCS-Einstellungen anwenden.
3. Payload verarbeiten.
4. Bild finalisieren.
5. Nur bei Erfolg die geaenderte Palette in den oeffentlichen Decoder zurueckschreiben.

Beispiel:

```rust
pub fn decode_from_dcs(
    &mut self,
    payload: &[u8],
    settings: DcsSettings,
) -> Result<SixelImage> {
    let mut frame = FrameDecoder::new(settings, self.palette.clone())?;
    frame.process(payload)?;

    let image = frame.finalize_image(settings)?;
    self.palette = frame.palette;

    Ok(image)
}
```

Falls `finalize()` aktuell Canvasdaten konsumiert, kann die Palette vor dem Konsum aus dem Frame entfernt oder geklont werden.

## Fehlerverhalten

Palette-Aenderungen eines fehlerhaften Bildes duerfen nicht persistent werden.

```rust
let old_palette = self.palette.clone();
let mut frame = FrameDecoder::new(settings, old_palette)?;

match frame.decode(payload) {
    Ok((image, new_palette)) => {
        self.palette = new_palette;
        Ok(image)
    }
    Err(error) => Err(error),
}
```

Das Verhalten ist absichtlich transaktional. Ein unvollstaendiges oder boesartiges Sixel darf Folgeframes nicht durch teilweise geaenderte Register beeinflussen.

## Was pro Bild zurueckgesetzt werden muss

Folgender Zustand darf nicht bilduebergreifend erhalten bleiben:

- Canvas und Bilddaten
- `color_index`
- `current_color` (aus Register 0 neu ableiten)
- Repeat-Zaehler
- X/Y-Position
- Maximalposition
- Pan/Pad und Rastergroesse
- Zielbreite/-hoehe
- Hintergrundindex
- Transparenz-/P2-Modus
- P1/P2/P3-Einstellungen

Nur `Palette::colors` ist persistent.

## Tests

### 1. Farbregister bleiben erhalten

Das erste Bild definiert Register 42 als Rot und zeichnet damit:

```text
#42;2;100;0;0#42~
```

Das zweite Bild verwendet Register 42 ohne erneute Definition:

```text
#42~
```

```rust
#[test]
fn shared_palette_survives_between_images() {
    let mut decoder = SixelDecoder::new();
    let settings = DcsSettings::default();

    decoder
        .decode_from_dcs(b"#42;2;100;0;0#42~", settings)
        .unwrap();

    let image = decoder
        .decode_from_dcs(b"#42~", settings)
        .unwrap();

    assert_eq!(&image.pixels[..4], &[255, 0, 0, 255]);
}
```

Falls `DcsSettings` nicht `Copy` ist, fuer jeden Aufruf neu erzeugen.

### 2. Decoder teilen keinen Zustand

```rust
#[test]
fn decoders_do_not_share_palettes() {
    let mut first = SixelDecoder::new();
    let mut second = SixelDecoder::new();

    first
        .decode_from_dcs(
            b"#42;2;100;0;0#42~",
            DcsSettings::default(),
        )
        .unwrap();

    let image = second
        .decode_from_dcs(b"#42~", DcsSettings::default())
        .unwrap();

    assert_ne!(&image.pixels[..4], &[255, 0, 0, 255]);
}
```

### 3. Palette-Reset

```rust
#[test]
fn reset_palette_restores_defaults() {
    let mut decoder = SixelDecoder::new();

    decoder
        .decode_from_dcs(
            b"#42;2;100;0;0#42~",
            DcsSettings::default(),
        )
        .unwrap();

    decoder.reset_palette();

    let image = decoder
        .decode_from_dcs(b"#42~", DcsSettings::default())
        .unwrap();

    assert_ne!(&image.pixels[..4], &[255, 0, 0, 255]);
}
```

### 4. Fehler ist transaktional

1. Register 42 als Rot definieren.
2. Ein fehlerhaftes Payload senden, das Register 42 vorher auf Gruen setzt.
3. Danach `#42~` dekodieren.
4. Das Ergebnis muss weiterhin Rot sein.

```rust
#[test]
fn failed_frame_does_not_mutate_shared_palette() {
    let mut decoder = SixelDecoder::new();

    decoder
        .decode_from_dcs(
            b"#42;2;100;0;0#42~",
            DcsSettings::default(),
        )
        .unwrap();

    assert!(decoder
        .decode_from_dcs(
            b"#42;2;0;100;0!invalid",
            DcsSettings::default(),
        )
        .is_err());

    let image = decoder
        .decode_from_dcs(b"#42~", DcsSettings::default())
        .unwrap();

    assert_eq!(&image.pixels[..4], &[255, 0, 0, 255]);
}
```

Der konkrete ungueltige Payload muss gegebenenfalls an die Fehlerregeln des Decoders angepasst werden.

### 5. Bildlokaler Zustand wird zurueckgesetzt

Ein erstes Bild mit grosser Rastergroesse, Wiederholung oder transparenter P2-Einstellung darf Breite, Hoehe, Cursor oder Hintergrundmodus des zweiten Bildes nicht beeinflussen.

## IcyTERM-Integration nach Bereitstellung

IcyTERM wird zwei Zustaende fuehren:

```rust
pub sixel_shared_palette: bool,
pub sixel_at_cursor: bool,
pub sixel_decoder: icy_sixel::SixelDecoder,
```

Mode 1070:

```rust
DecMode::SixelPrivatePalette => {
    // DECSET 1070: private Farbregister pro Bild
    terminal.sixel_shared_palette = !enabled;
    if enabled {
        terminal.sixel_decoder.reset_palette();
    }
}
```

Dekodierung:

```rust
let image = if terminal.sixel_shared_palette {
    terminal.sixel_decoder.decode_from_dcs(payload, settings)
} else {
    icy_sixel::SixelDecoder::new().decode_from_dcs(payload, settings)
};
```

Mode 80 wird getrennt in IcyTERM umgesetzt:

- moderner DEC/xterm/foot-Sinn: `CSI ? 80 l` zeichnet am Textcursor
- `CSI ? 80 h` zeichnet am Bildschirmursprung

SyncTERM/CTerm-Versionen vor der Polaritaetskorrektur interpretieren den Modus teilweise umgekehrt. IcyTERM sollte den modernen dokumentierten Sinn implementieren; termgfx erkennt bzw. korrigiert alte CTerm-Versionen separat.

## Akzeptanzkriterien

Die Aenderung ist fertig, wenn:

1. Bestehende zustandslose APIs unveraendert funktionieren.
2. Zwei Bilder mit derselben Decoderinstanz Farbregister teilen.
3. Zwei unterschiedliche Decoderinstanzen keine Farbregister teilen.
4. `reset_palette()` die Standardpalette wiederherstellt.
5. Fehlerhafte Bilder den persistenten Zustand nicht veraendern.
6. Alle bestehenden icy_sixel-Tests weiterhin bestehen.
7. SyncMOO-Folgeframes ohne erneute Palette korrekt und deckend dargestellt werden koennen.
