use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum LoopTarget {
    /// Single command identifier, e.g. 'L', 'S', 'G'.
    Single(char),

    /// Chain-Gang sequence, e.g. ">CL@".
    ChainGang {
        /// Raw representation including leading '>' and trailing '@' for roundtrip.
        raw: String,
        /// Extracted command identifiers inside the chain.
        commands: Vec<char>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoopModifiers {
    /// XOR stepping ("|" after the command identifier).
    pub xor_stepping: bool,
    /// For W command: fetch text each iteration ("@" after the command identifier).
    pub refresh_text_each_iteration: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoopParamToken {
    /// Plain numeric value.
    Number(i32),
    /// Symbolic value, usually 'x' or 'y'.
    Symbol(char),
    /// Expression like "+10", "-10", "!99".
    Expr(String),
    /// Group separator corresponding to ':' in the text representation.
    GroupSeparator,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopCommandData {
    pub from: i32,
    pub to: i32,
    pub step: i32,
    pub delay: i32,
    pub target: LoopTarget,
    pub modifiers: LoopModifiers,
    pub param_count: u16,
    pub params: Vec<LoopParamToken>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IgsCommand {
    // Drawing commands
    /// Box/Rectangle drawing command (B)
    ///
    /// IGS: `G#B>x1,y1,x2,y2,rounded:`
    ///
    /// Draws a rectangle or box between two points with optional rounded corners.
    /// All fill attributes, patterns, and border settings from the `AttributeForFills`
    /// command affect this command.
    ///
    /// # Parameters
    /// * `x1` - Upper left corner X coordinate
    /// * `y1` - Upper left corner Y coordinate  
    /// * `x2` - Lower right corner X coordinate
    /// * `y2` - Lower right corner Y coordinate
    /// * `rounded` - Corner style: `false` for square corners, `true` for rounded corners
    ///
    /// # Example
    /// `G#B>0,0,100,100,0:` - Draws a square box from (0,0) to (100,100)
    Box {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        rounded: bool,
    },

    /// Line drawing command (L)
    ///
    /// IGS: `G#L>x1,y1,x2,y2:`
    ///
    /// Draws a line between two specified points. The line style, color, thickness,
    /// and end styles are controlled by the `LineStyle` and `ColorSet` commands.
    /// This command also sets the starting point for subsequent `LineDrawTo` commands.
    ///
    /// # Parameters
    /// * `x1` - Beginning X coordinate
    /// * `y1` - Beginning Y coordinate
    /// * `x2` - Ending X coordinate
    /// * `y2` - Ending Y coordinate
    ///
    /// # See Also
    /// * `LineStyle` - Controls line appearance
    /// * `ColorSet` - Sets line color
    /// * `LineDrawTo` - Draws from last endpoint
    Line {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    },

    /// Draw line from last position (D)
    ///
    /// IGS: `G#D>x,y:`
    ///
    /// Draws a line from the last polymarker plot, line endpoint, or drawto position
    /// to the specified coordinates. Use `PolymarkerPlot`, `Line`, or `SetDrawtoBegin`
    /// to establish the starting point.
    ///
    /// # Parameters
    /// * `x` - Target X coordinate
    /// * `y` - Target Y coordinate
    ///
    /// # Note
    /// If no starting point has been established, behavior is undefined.
    LineDrawTo {
        x: i32,
        y: i32,
    },

    /// Circle drawing command (O)
    ///
    /// IGS: `G#O>x,y,radius:`
    ///
    /// Draws a circle or filled disc depending on the `HollowSet` command state.
    /// When hollow mode is active, only the outline is drawn. Fill patterns and
    /// colors are controlled by `AttributeForFills` and `ColorSet`.
    ///
    /// # Parameters
    /// * `x` - Center X coordinate
    /// * `y` - Center Y coordinate
    /// * `radius` - Circle radius in pixels
    Circle {
        x: i32,
        y: i32,
        radius: i32,
    },

    /// Ellipse/Oval drawing command (Q)
    ///
    /// IGS: `G#Q>x,y,x_radius,y_radius:`
    ///
    /// Draws an ellipse (oval) with different horizontal and vertical radii.
    /// Affected by `HollowSet` for filled vs outline mode, and `AttributeForFills`
    /// for fill patterns.
    ///
    /// # Parameters
    /// * `x` - Center X coordinate
    /// * `y` - Center Y coordinate
    /// * `x_radius` - Horizontal radius
    /// * `y_radius` - Vertical radius
    Ellipse {
        x: i32,
        y: i32,
        x_radius: i32,
        y_radius: i32,
    },

    /// Circular arc drawing command (K)
    ///
    /// IGS: `G#K>x,y,radius,start_angle,end_angle:`
    ///
    /// Draws a portion of a circle's circumference between two angles.
    /// Angles are measured in degrees, with 0° at 3 o'clock position,
    /// increasing clockwise.
    ///
    /// # Parameters
    /// * `x` - Center X coordinate
    /// * `y` - Center Y coordinate
    /// * `radius` - Arc radius
    /// * `start_angle` - Starting angle in degrees (0-360)
    /// * `end_angle` - Ending angle in degrees (0-360)
    Arc {
        x: i32,
        y: i32,
        radius: i32,
        start_angle: i32,
        end_angle: i32,
    },

    /// Polyline drawing command (z)
    ///
    /// IGS: `G#z>n,x1,y1,x2,y2,...:`
    ///
    /// Draws a connected series of lines through multiple points. Lines are drawn
    /// sequentially from point to point. Minimum 2 points required, maximum 128.
    /// Line color, style, and end styles apply.
    ///
    /// # Parameters
    /// * `points` - Flat array of X,Y coordinate pairs [x1,y1,x2,y2,...]
    ///
    /// # Example
    /// `G#z>3,100,0,150,100,50,100:` - Triangle with 3 points
    PolyLine {
        points: Vec<i32>,
    },

    /// Polygon fill command (f)
    ///
    /// IGS: `G#f>n,x1,y1,x2,y2,...:`
    ///
    /// Draws and fills a polygon defined by a series of points. The last point
    /// is automatically connected to the first. Fill pattern, color, and border
    /// are controlled by `AttributeForFills`.
    ///
    /// # Parameters
    /// * `points` - Flat array of X,Y coordinate pairs forming the polygon vertices
    ///
    /// # Note
    /// Passing 1 or 2 points draws a point or line respectively
    PolyFill {
        points: Vec<i32>,
    },

    /// Flood fill command (F)
    ///
    /// IGS: `G#F>x,y:`
    ///
    /// Fills a bounded area by replacing all pixels of the same color at the
    /// specified position until a different color or screen edge is reached.
    /// Uses the current fill color set by `ColorSet`.
    ///
    /// # Parameters
    /// * `x` - Starting X coordinate
    /// * `y` - Starting Y coordinate
    FloodFill {
        x: i32,
        y: i32,
    },

    /// Polymarker plot command (P)
    ///
    /// IGS: `G#P>x,y:`
    ///
    /// Plots a point or polymarker shape at the specified position. The marker
    /// type (point, plus, star, square, etc.) and size are controlled by the
    /// `LineStyle` command with type=1. This also sets the starting point for
    /// `LineDrawTo` commands.
    ///
    /// # Parameters
    /// * `x` - X coordinate
    /// * `y` - Y coordinate
    PolymarkerPlot {
        x: i32,
        y: i32,
    },

    // Color/Style commands
    /// Color selection command (C)
    ///
    /// IGS: `G#C>pen,color:`
    ///
    /// Selects which pen number (0-15) to use for different screen operations.
    /// This maps a logical pen to a color palette entry.
    ///
    /// # Parameters
    /// * `pen` - Screen operation to set:
    ///   - 0: Polymarker color (for `PolymarkerPlot`)
    ///   - 1: Line color
    ///   - 2: Fill color
    ///   - 3: Text color (for `WriteText`)
    /// * `color` - Pen number to use (0-15)
    ColorSet {
        pen: u8,
        color: u8,
    },

    /// Fill attributes command (A)
    ///
    /// IGS: `G#A>pattern_type,pattern_index,border:`
    ///
    /// Sets attributes for all fill operations including boxes, circles, polygons.
    /// Controls the fill pattern type, specific pattern, and whether borders are drawn.
    ///
    /// # Parameters
    /// * `pattern_type` - Fill type:
    ///   - 0: Hollow (no fill)
    ///   - 1: Solid color
    ///   - 2: Pattern (uses pattern_index 1-24)
    ///   - 3: Hatch (uses pattern_index 1-12)
    ///   - 4: User defined (uses patterns 0-9)
    /// * `pattern_index` - Specific pattern within the type:
    ///   - For type 2: patterns 1-24 (see ST BASIC manual)
    ///   - For type 3: hatches 1-12
    ///   - For type 4: user patterns 0-9 (8 sets pattern 0 as random, 9+ as default)
    /// * `border` - Draw border around filled areas: `true` for yes, `false` for no
    AttributeForFills {
        pattern_type: u8,
        pattern_index: u8,
        border: bool,
    },

    /// Line and marker style command (T)
    ///
    /// IGS: `G#T>type,style,size:`
    ///
    /// Controls the appearance of lines and polymarkers including style, thickness,
    /// and end decorations (arrows, rounded ends).
    ///
    /// # Parameters
    /// * `style` - What to modify:
    ///   - 1: Polymarkers (affects `PolymarkerPlot`)
    ///   - 2: Lines (affects `Line` and `LineDrawTo`)
    /// * `thickness` - Style selection:
    ///   - For polymarkers: 1=point, 2=plus, 3=star, 4=square, 5=diagonal cross, 6=diamond
    ///   - For lines: 1=solid, 2=long dash, 3=dotted, 4=dash-dot, 5=dashed, 6=dash-dot-dot, 7=user defined
    ///
    /// # Third Parameter Usage
    /// - For polymarkers: size 1-8
    /// - For solid lines: thickness 1-41
    /// - For user defined lines: pattern number 1-32
    /// - Line end styles (add to size):
    ///   - 0: Square ends
    ///   - 50: Arrows both ends
    ///   - 51: Arrow left, square right
    ///   - 52: Arrow right, square left
    ///   - 60: Rounded both ends
    LineStyle {
        kind: u8,   // 1=polymarkers, 2=lines
        style: u8,  // style code
        value: u16, // third parameter: size/thickness/pattern/end-style composite
    },

    /// Set pen RGB color command (S)
    ///
    /// IGS: `G#S>pen,red,green,blue:`
    ///
    /// Directly sets the RGB color values for a specific pen number.
    /// Each color component uses 3 bits (0-7 range) for a total of 512 possible colors.
    ///
    /// # Parameters
    /// * `pen` - Pen number to modify (0-15)
    /// * `red` - Red component (0-7)
    /// * `green` - Green component (0-7)
    /// * `blue` - Blue component (0-7)
    SetPenColor {
        pen: u8,
        red: u8,
        green: u8,
        blue: u8,
    },

    /// Drawing mode command (M)
    ///
    /// IGS: `G#M>mode:`
    ///
    /// Sets the logical operation used when drawing pixels, allowing for
    /// effects like XOR drawing for erasable graphics.
    ///
    /// # Parameters
    /// * `mode` - Drawing mode:
    ///   - 1: Replace (normal drawing)
    ///   - 2: Transparent (skip background pixels)
    ///   - 3: XOR (reversible drawing)
    ///   - 4: Reverse transparent
    DrawingMode {
        mode: u8,
    },

    /// Hollow/filled toggle command (H)
    ///
    /// IGS: `G#H>enabled:`
    ///
    /// Controls whether shapes are drawn filled or as outlines only.
    /// When enabled, circles become rings, rectangles become frames, etc.
    ///
    /// # Parameters
    /// * `enabled` - `true` for hollow (outline only), `false` for filled
    HollowSet {
        enabled: bool,
    },

    // Text commands
    /// Write text command (W)
    ///
    /// IGS: `G#W>x,y,text@`
    ///
    /// Spec (IG220) shows only X, Y and a text string terminated with `@`.
    /// A justification parameter is not documented; older files may include an
    /// extra numeric which we intentionally omit for spec conformity.
    WriteText {
        x: i32,
        y: i32,
        text: String,
    },

    /// Text effects command (E)
    ///
    /// IGS: `G#E>effects,size,rotation:`
    ///
    /// Sets VDI text attributes for text drawn with `WriteText` including
    /// style effects, point size, and rotation angle.
    ///
    /// # Parameters
    /// * `effects` - Bit flags for text effects (can be combined):
    ///   - 0: Normal
    ///   - 1: Thickened (bold)
    ///   - 2: Ghosted
    ///   - 4: Skewed (italic)
    ///   - 8: Underlined
    ///   - 16: Outlined
    /// * `size` - Text size in points (1/72 inch). Common sizes: 8, 9, 10, 16, 18, 20
    /// * `rotation` - Text angle:
    ///   - 0: 0 degrees (normal)
    ///   - 1: 90 degrees
    ///   - 2: 180 degrees
    ///   - 3: 270 degrees
    TextEffects {
        effects: u8,
        size: u8,
        rotation: u8,
    },

    // Special commands
    /// Sound effects command (b)
    ///
    /// IGS: `G#b>sound_number:`
    ///
    /// Plays predefined sound effects using the ST's sound chip.
    /// Effects 0-4 are internally looped for extended duration.
    ///
    /// # Parameters
    /// * `sound_number` - Effect to play:
    ///   - 0: Alien Invasion
    ///   - 1: Red Alert
    ///   - 2: Gunshot
    ///   - 3: Laser 1
    ///   - 4: Jackhammer
    ///   - 5: Teleport
    ///   - 6: Explosion
    ///   - 7: Laser 2
    ///   - 8: Longbell
    ///   - 9: Surprise
    ///   - 10: Radio Broadcast
    ///   - 11: Bounce Ball
    ///   - 12: Eerie Sound
    ///   - 13: Harley Motorcycle
    ///   - 14: Helicopter
    ///   - 15: Steam Locomotive
    ///   - 16: Wave
    ///   - 17: Robot Walk
    ///   - 18: Passing Plane
    ///   - 19: Landing
    ///
    /// # Extended Functions
    /// - 20: Alter sound effect parameters
    /// - 21: Stop all sounds immediately
    /// - 22: Restore sound effect to default
    /// - 23: Set loop count for effects 0-4
    BellsAndWhistles {
        sound_number: u8,
    },
    AlterSoundEffect {
        play_flag: u8,
        snd_num: u8,
        element_num: u8,
        negative_flag: u8,
        thousands: u16,
        hundreds: u16,
    },
    StopAllSound,
    RestoreSoundEffect {
        snd_num: u8,
    },
    SetEffectLoops {
        count: u32,
    },

    /// Graphic scaling command (g)
    ///
    /// IGS: `G#g>enabled:`
    ///
    /// Enables coordinate scaling to a virtual 10000x10000 screen, allowing
    /// resolution-independent graphics. Coordinates are automatically scaled
    /// to the actual screen resolution.
    ///
    /// # Parameters
    /// * `enabled` - Scaling mode:
    ///   - `false`: Normal pixel coordinates
    ///   - `true`: Scale to 10000x10000 virtual screen
    ///
    /// # Special Mode
    /// Value 2 doubles Y coordinates in monochrome mode for aspect correction
    GraphicScaling {
        mode: u8,
    },

    /// Screen grab/BitBlit command (G)
    ///
    /// IGS: `G#G>type,mode,...:`
    ///
    /// Performs bit-block transfer operations for copying screen regions,
    /// saving to memory, or restoring from memory. Supports various logical
    /// operations for combining source and destination.
    ///
    /// # Parameters
    /// * `blit_type` - Operation type:
    ///   - 0: Screen to screen
    ///   - 1: Screen to memory
    ///   - 2: Memory to screen
    ///   - 3: Piece of memory to screen
    ///   - 4: Memory to memory
    /// * `mode` - Logical operation (0-15):
    ///   - 0: Clear destination
    ///   - 3: Replace
    ///   - 6: XOR
    ///   - 7: Transparent (OR)
    ///   - 15: Fill destination
    /// * `params` - Additional coordinates depending on blit_type
    GrabScreen {
        blit_type: u8,
        mode: u8,
        params: Vec<i32>,
    },

    /// Initialize command (I)
    ///
    /// IGS: `G#I>mode:`
    ///
    /// Initializes the IG environment, resetting colors, attributes, and optionally
    /// the resolution. Should be called at the start of each graphics sequence
    /// for a consistent starting point.
    ///
    /// # Parameters
    /// * `mode` - Initialization type:
    ///   - 0: Set desktop palette and attributes
    ///   - 1: Set desktop palette only
    ///   - 2: Set desktop attributes only
    ///   - 3: Set IG default palette
    ///   - 4: Set VDI default palette
    ///   - 5: Set desktop resolution and VDI clipping
    ///
    /// # Note
    /// Mode 5 should be used FIRST before any palette commands
    Initialize {
        mode: u8,
    },

    /// Elliptical arc command (J)
    ///
    /// IGS: `G#J>x,y,x_radius,y_radius,start_angle,end_angle:`
    ///
    /// Draws an arc of an ellipse between two angles.
    ///
    /// # Parameters
    /// * `x` - Center X coordinate
    /// * `y` - Center Y coordinate
    /// * `x_radius` - Horizontal radius
    /// * `y_radius` - Vertical radius
    /// * `start_angle` - Starting angle in degrees
    /// * `end_angle` - Ending angle in degrees
    EllipticalArc {
        x: i32,
        y: i32,
        x_radius: i32,
        y_radius: i32,
        start_angle: i32,
        end_angle: i32,
    },

    /// Cursor control command (k)
    ///
    /// IGS: `G#k>mode:`
    ///
    /// Controls text cursor visibility and backspace behavior.
    ///
    /// # Parameters
    /// * `mode` - Cursor mode:
    ///   - 0: Cursor off
    ///   - 1: Cursor on
    ///   - 2: Destructive backspace
    ///   - 3: Non-destructive backspace
    Cursor {
        mode: u8,
    },

    /// Chip music command (n)
    ///
    /// IGS: `G#n>effect,voice,volume,pitch,timing,stop_type:`
    ///
    /// Plays musical notes using sound effects as instruments. Allows multi-voice
    /// music with timing control. No flow control during playback.
    ///
    /// # Parameters
    /// * `effect` - Sound effect to use as instrument (0-19)
    /// * `voice` - Voice channel (0-2)
    /// * `volume` - Volume level (0-15)
    /// * `pitch` - Note pitch (0-255, 0 = no sound but processes timing)
    /// * `timing` - Duration in 200ths of a second (0-9999)
    /// * `stop_type` - How to stop the note:
    ///   - 0: No effect, sound continues
    ///   - 1: Move voice to release phase
    ///   - 2: Stop voice immediately
    ///   - 3: Move all voices to release
    ///   - 4: Stop all voices immediately
    ChipMusic {
        effect: u8,
        voice: u8,
        volume: u8,
        pitch: u8,
        timing: i32,
        stop_type: u8,
    },

    /// Noise/MIDI command (N)
    ///
    /// IGS: `G#N>operation,...:`
    ///
    /// Handles sound through MIDI or sound chip. Can load, execute, or replay
    /// sound data from a buffer. Supports up to 9999 bytes of MIDI data.
    ///
    /// # Parameters
    /// * `params` - Variable parameters depending on operation:
    ///   - First param 0: Load MIDI buffer only
    ///   - First param 1: Load and execute MIDI
    ///   - First param 2: Execute loaded MIDI buffer
    ///   - First param 3: Load sound chip data
    ///   - First param 4: Load and execute sound chip
    ///   - First param 5: Execute loaded sound chip buffer
    ///   - First param 6: Execute buffer from position x to y
    Noise {
        params: Vec<i32>,
    },

    /// Rounded rectangles command (U)
    ///
    /// IGS: `G#U>x1,y1,x2,y2,fill:`
    ///
    /// Draws rectangles with rounded corners. Can be filled or outline only.
    ///
    /// # Parameters
    /// * `x1` - Upper left X coordinate
    /// * `y1` - Upper left Y coordinate
    /// * `x2` - Lower right X coordinate
    /// * `y2` - Lower right Y coordinate
    /// * `fill` - `false` for filled with no border, `true` for outline affected by attributes
    RoundedRectangles {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        fill: bool,
    },

    /// Pie slice command (V)
    ///
    /// IGS: `G#V>x,y,radius,start_angle,end_angle:`
    ///
    /// Draws a pie slice (sector) of a circle, like a piece of pie chart.
    ///
    /// # Parameters
    /// * `x` - Center X coordinate
    /// * `y` - Center Y coordinate
    /// * `radius` - Circle radius
    /// * `start_angle` - Starting angle in degrees
    /// * `end_angle` - Ending angle in degrees
    PieSlice {
        x: i32,
        y: i32,
        radius: i32,
        start_angle: i32,
        end_angle: i32,
    },

    /// Elliptical pie slice command (Y)
    ///
    /// IGS: `G#Y>x,y,x_radius,y_radius,start_angle,end_angle:`
    ///
    /// Draws a pie slice of an ellipse.
    ///
    /// # Parameters
    /// * `x` - Center X coordinate
    /// * `y` - Center Y coordinate
    /// * `x_radius` - Horizontal radius
    /// * `y_radius` - Vertical radius
    /// * `start_angle` - Starting angle in degrees
    /// * `end_angle` - Ending angle in degrees
    EllipticalPieSlice {
        x: i32,
        y: i32,
        x_radius: i32,
        y_radius: i32,
        start_angle: i32,
        end_angle: i32,
    },

    /// Filled rectangle command (Z)
    ///
    /// IGS: `G#Z>x1,y1,x2,y2:`
    ///
    /// Draws a filled rectangle without border. The border setting from
    /// `AttributeForFills` has no effect on this command.
    ///
    /// # Parameters
    /// * `x1` - Upper left X coordinate
    /// * `y1` - Upper left Y coordinate
    /// * `x2` - Lower right X coordinate
    /// * `y2` - Lower right Y coordinate
    FilledRectangle {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    },

    /// Input command (<)
    ///
    /// IGS: `G#<>type,input_type,output_options:`
    ///
    /// Gets input from user's keyboard or mouse. Can handle single keys, strings,
    /// or mouse zone clicks. Used for interactive menus and user input.
    ///
    /// # Parameters
    /// * `input_type` - Type of input:
    ///   - 0: Transmit carriage return at end
    ///   - 1: No carriage return
    /// * `params` - Additional parameters:
    ///   - Param 2: 0=single key, 1=string, 2-10=mouse zones with different pointers
    ///   - Param 3: 0=don't echo, 1=echo, 2=echo but discard, 3=don't echo and discard
    InputCommand {
        input_type: u8,
        params: Vec<i32>,
    },

    /// Ask IG command (?)
    ///
    /// IGS: `G#?>query:`
    ///
    /// Queries the IG terminal for information and transmits the response
    /// back to the host system.
    ///
    /// # Parameters
    /// * `query` - Information to request:
    ///   - 0: Version number
    ///   - 1: Cursor position and mouse button state
    ///   - 2: Mouse position and button state
    ///   - 3: Current resolution (0=low, 1=medium, 2=high)
    AskIG {
        query: u8,
    },

    /// Screen clear command (s)
    ///
    /// IGS: `G#s>mode:`
    ///
    /// Clears all or part of the screen using various methods.
    ///
    /// # Parameters
    /// * `mode` - Clear mode:
    ///   - 0: Clear screen and home cursor
    ///   - 1: Clear from home to cursor
    ///   - 2: Clear from cursor to bottom
    ///   - 3: Clear whole screen with VDI
    ///   - 4: Clear with VDI and home cursor
    ///   - 5: Quick VT52 reset (clear, home, reverse off, reset colors)
    ScreenClear {
        mode: u8,
    },

    /// Set resolution command (R)
    ///
    /// IGS: `G#R>resolution,palette:`
    ///
    /// Switches between low (320x200, 16 colors) and medium (640x200, 4 colors)
    /// resolution. Can optionally reset the color palette.
    ///
    /// # Parameters
    /// * `resolution` - Target resolution:
    ///   - 0: Low resolution (320x200, 16 colors)
    ///   - 1: Medium resolution (640x200, 4 colors)
    /// * `palette` - Palette to load:
    ///   - 0: No change
    ///   - 1: Desktop colors
    ///   - 2: IG default palette
    ///   - 3: VDI default palette
    SetResolution {
        resolution: u8,
        palette: u8,
    },

    /// Quick pause command (t/q)
    ///
    /// IGS: `G#t>vsyncs:` or `G#q>vsyncs:`
    ///
    /// Pauses execution for a specified number of vertical syncs (1/60th second each).
    /// Also used to control double-stepping for BitBlit operations.
    ///
    /// # Parameters
    /// * `vsyncs` - Number of vsyncs to wait (max 180), or special values:
    ///   - 9995: Double-step with 3 vsyncs
    ///   - 9996: Double-step with 2 vsyncs
    ///   - 9997: Double-step with 1 vsync
    ///   - 9998: Double-step with 0 vsyncs
    ///   - 9999: Turn double-step off
    PauseSeconds {
        seconds: u8,
    },
    VsyncPause {
        vsyncs: i32,
    },

    /// Loop command (&)
    ///
    /// IGS: `G#&>from,to,step,delay,command,...:`
    ///
    /// Loops a command or chain of commands with variable substitution.
    /// Powerful for animations, patterns, and data-driven drawing.
    Loop(LoopCommandData),

    // Extended X commands
    /// Spray paint effect (X 0)
    ///
    /// IGS: `G#X>0,x,y,width,height,density:`
    ///
    /// Randomly plots polymarkers within a rectangular area defined by
    /// the top-left corner (`x`,`y`) and the rectangle size (`width`,`height`).
    /// `density` controls how many points (maximum 9999) are sprayed.
    ///
    /// # Parameters
    /// * `x` - Upper left X coordinate
    /// * `y` - Upper left Y coordinate
    /// * `width` - Width of spray area (max 255)
    /// * `height` - Height of spray area (max 255)
    /// * `density` - Number of points to plot (max 9999)
    SprayPaint {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        density: i32,
    },

    /// Set color register (X 1)
    ///
    /// IGS: `G#X>1,register,value:`
    ///
    /// Directly sets a hardware color register value using Xbios(7).
    /// Differs from `SetPenColor` which sets a pen's register.
    ///
    /// # Parameters
    /// * `register` - Color register number (0-15)
    /// * `value` - Color value (0-1911 for ST, higher for STE)
    SetColorRegister {
        register: u8,
        value: i32,
    },

    /// Set random number range (X 2)
    ///
    /// IGS: `G#X>2,min,max:` or `G#X>2,min,min,max:`
    ///
    /// Sets the range for random values when 'r' or 'R' is used in
    /// command parameters. Default range is 0-199.
    ///
    /// # Parameters
    /// * `params` - [min, max] for 'r', or [min, min, max] for 'R'
    SetRandomRange {
        params: Vec<i32>,
    },

    /// Right mouse button macro (X 3)
    ///
    /// IGS: `G#X>3,operation,...:`
    ///
    /// Defines a string to transmit when right mouse button is clicked.
    /// Useful for function menus or navigation.
    ///
    /// # Parameters
    /// * `params` - Operation and data:
    ///   - [0]: Deactivate macro
    ///   - [1, cr]: Reactivate (cr: 0=no CR, 1=add CR)
    ///   - [2, on, cr, len, ...]: Load and configure macro
    RightMouseMacro {
        params: Vec<i32>,
    },

    /// Define mouse click zones (X 4)
    ///
    /// IGS: `G#X>4,zone_id,x1,y1,x2,y2,len,string:`
    ///
    /// Defines a rectangular mouse zone associated with a return string.
    /// The `len` parameter gives the length of `string`. Special zone IDs
    /// perform global operations (clear / loopback control).
    ///
    /// # Parameters
    /// * `zone_id` - Zone number (0-47 normal, 9999/9998/9997 special)
    /// * `x1`, `y1` - Upper-left corner
    /// * `x2`, `y2` - Lower-right corner
    /// * `length` - Length of `string` in bytes
    /// * `string` - Data transmitted when zone is activated
    DefineZone {
        zone_id: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        length: u16,
        string: String,
    },

    /// Flow control settings (X 5)
    ///
    /// IGS: `G#X>5,mode,...:`
    ///
    /// Controls XON/XOFF flow control behavior. Can disable flow control
    /// or customize the control characters and repetitions.
    ///
    /// # Parameters
    /// * `mode` - Flow control mode:
    ///   - 0: Off
    ///   - 1: On
    ///   - 2: On with custom XON settings
    ///   - 3: On with custom XOFF settings
    ///   - 4: Reset to IG defaults
    /// * `params` - Additional parameters for custom settings
    FlowControl {
        mode: u8,
        params: Vec<i32>,
    },

    /// Left mouse button behavior (X 6)
    ///
    /// IGS: `G#X>6,mode:`
    ///
    /// Configures left mouse button to send carriage return or other actions.
    ///
    /// # Parameters
    /// * `mode` - Button behavior:
    ///   - 0: Normal (default)
    ///   - 1: Send CR on click
    ///   - 2: Send CR+LF on click
    LeftMouseButton {
        mode: u8,
    },

    /// Load fill pattern (X 7)
    ///
    /// IGS: `G#X>7,pattern,data...:`
    ///
    /// Loads a custom 16x16 bit pattern for fills. Patterns 6-7 also
    /// serve as line patterns.
    ///
    /// # Parameters
    /// * `pattern` - Pattern slot (0-7)
    /// * `data` - 16 strings of 16 characters each, 'X' for set bits
    ///
    /// # Example
    /// Each row ends with '@', use 'X' for 1 bits, anything else for 0
    LoadFillPattern {
        pattern: u8,
        data: String,
    },

    /// Rotate color registers (X 8)
    ///
    /// IGS: `G#X>8,start,end,count,delay:`
    ///
    /// Animates colors by rotating register values. Useful for water,
    /// fire, and rainbow effects. Most effective in low resolution.
    ///
    /// # Parameters
    /// * `start_reg` - Starting register number
    /// * `end_reg` - Ending register number
    /// * `count` - 0 reset  -  otherwise  number of times to shift colors.
    /// * `delay` - number of 200 hundredths of a second between each color shift.  range 0-9999.
    /// # Note
    /// If start < end, rotates right. If start > end, rotates left.
    RotateColorRegisters {
        start_reg: u8,
        end_reg: u8,
        count: i32,
        delay: i32,
    },

    /// Load/execute MIDI or IG commands (X 9)
    ///
    /// IGS: `G#X>9,operation,...:`
    ///
    /// Uses the MIDI buffer to store and execute IG graphics commands.
    /// Buffer holds 10001 bytes (~121 lines of commands).
    ///
    /// # Parameters
    /// * `params` - Operation:
    ///   - [0, ...]: Load commands until ||} marker
    ///   - [1]: Execute loaded commands
    ///   - [2]: Clear buffer
    LoadMidiBuffer {
        params: Vec<i32>,
    },

    /// Set DrawTo begin point (X 10)
    ///
    /// IGS: `G#X>10,x,y:`
    ///
    /// Sets the starting point for `LineDrawTo` commands without drawing.
    /// Same effect as `Line` or `PolymarkerPlot` but leaves screen unchanged.
    ///
    /// # Parameters
    /// * `x` - Starting X coordinate
    /// * `y` - Starting Y coordinate
    SetDrawtoBegin {
        x: i32,
        y: i32,
    },

    /// Load BitBlit memory (X 11)
    ///
    /// IGS: `G#X>11,operation,section,...:`
    ///
    /// Manages the 32KB BitBlit memory buffer for storing/displaying bitmaps.
    /// Can load full screen or 4KB horizontal sections.
    ///
    /// # Parameters
    /// * `params` - Operation and data:
    ///   - [0, section, value]: Wipe memory with value
    ///   - [1, section]: Load and show bitmap
    ///   - [2, section]: Load without showing
    ///
    /// # Sections
    /// - 0: Entire 32KB buffer
    /// - 1-8: 4KB horizontal bands (1=top, 8=bottom)
    LoadBitblitMemory {
        params: Vec<i32>,
    },

    /// Load color palette (X 12)
    ///
    /// IGS: `G#X>12,bank,c0,c1,c2,c3:`
    ///
    /// Loads hardware color register values in groups of 4.
    /// For medium resolution, call once. For low resolution, call 4 times.
    ///
    /// # Parameters
    /// * `params` - Bank and colors:
    ///   - [0, ...]: Registers 0-3
    ///   - [1, ...]: Registers 4-7
    ///   - [2, ...]: Registers 8-11
    ///   - [3, ...]: Registers 12-15
    LoadColorPalette {
        params: Vec<i32>,
    },

    // IGS-specific color commands (ESC b/c are IGS extensions, not standard VT52)
    /// Set text foreground color (ESC b)
    ///
    /// IGS: `\x1bb[color]` or `G#c>1,color:`
    ///
    /// Sets the foreground color for VT52 text.
    ///
    /// # Parameters
    /// * `color` - Color register (0-15)
    SetForeground {
        color: u8,
    },

    /// Set text background color (ESC c)
    ///
    /// IGS: `\x1bc[color]` or `G#c>0,color:`
    ///
    /// Sets the background color for VT52 text.
    ///
    /// # Parameters
    /// * `color` - Color register (0-15)
    SetBackground {
        color: u8,
    },

    // VT52 additional IGS commands (not removed because they have IGS G# equivalents)
    /// Delete lines (ESC d)
    ///
    /// IGS: `G#d>count:` or `\x1bd[count]`
    ///
    /// Deletes specified number of lines starting at cursor.
    /// Lines below scroll up to fill the gap.
    ///
    /// # Parameters
    /// * `count` - Number of lines to delete
    DeleteLine {
        count: u8,
    },

    /// Insert lines (ESC i)
    ///
    /// IGS: `G#i>mode,count:` or `\x1bi[count]`
    ///
    /// Inserts blank lines at cursor position.
    /// Mode parameter is only present in IG form; ESC form omits it.
    ///
    /// # Parameters
    /// * `mode` - Insertion mode (0 = normal). If absent (ESC form) use 0.
    /// * `count` - Number of lines to insert
    InsertLine {
        mode: u8,
        count: u8,
    },

    /// Clear line (ESC l)
    ///
    /// IGS: `G#l>mode:`
    ///
    /// Clears the current line. Mode parameter only in IG form; ESC form implies 0.
    ///
    /// # Parameters
    /// * `mode` - 0 = clear line & keep cursor column, other modes reserved
    ClearLine {
        mode: u8,
    },

    /// Cursor motion (ESC m)
    ///
    /// IGS: `G#m>direction,count:` or `\x1bm[x],[y]`
    ///
    /// Moves cursor by specified amount in given direction.
    /// Two encodings exist; unified here. For IG form use direction/count.
    /// For ESC form store x/y mapping to direction/count internally.
    ///
    /// # Parameters
    /// * `direction` - 0=up,1=down,2=left,3=right (IG form) or derived from sign of x/y (ESC form)
    /// * `count` - Number of positions to move
    CursorMotion {
        direction: u8,
        count: i32,
    },

    /// Position cursor (p)
    ///
    /// IGS: `G#p>column,row:`
    ///
    /// Positions cursor at character column and row.
    ///
    /// # Parameters
    /// * `x` - Column (0-79)
    /// * `y` - Row (0-24)
    PositionCursor {
        x: i32,
        y: i32,
    },

    /// Remember cursor position (ESC r)
    ///
    /// IGS: `G#r>value:` or `\x1br`
    ///
    /// Remembers current cursor position for recall.
    /// Different from save/restore cursor commands. Value only present in IG form.
    ///
    /// # Parameters
    /// * `value` - Usually 0; reserved for future use.
    RememberCursor {
        value: u8,
    },

    /// Inverse video toggle (v)
    ///
    /// IGS: `G#v>enabled:`
    ///
    /// Toggles inverse video mode where foreground and background
    /// colors are swapped for text display.
    ///
    /// # Parameters
    /// * `enabled` - `true` for inverse, `false` for normal
    InverseVideo {
        enabled: bool,
    },

    /// Line wrap toggle (w)
    ///
    /// IGS: `G#w>enabled:`
    ///
    /// Controls whether text wraps to next line at right margin.
    ///
    /// # Parameters
    /// * `enabled` - `true` for wrap enabled, `false` for disabled
    LineWrap {
        enabled: bool,
    },
}

impl fmt::Display for IgsCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IgsCommand::Box { x1, y1, x2, y2, rounded } => {
                if *rounded {
                    write!(f, "G#B>{},{},{},{},1:", x1, y1, x2, y2)
                } else {
                    write!(f, "G#B>{},{},{},{},0:", x1, y1, x2, y2)
                }
            }
            IgsCommand::Line { x1, y1, x2, y2 } => write!(f, "G#L>{},{},{},{}:", x1, y1, x2, y2),
            IgsCommand::LineDrawTo { x, y } => write!(f, "G#D>{},{}:", x, y),
            IgsCommand::Circle { x, y, radius } => write!(f, "G#O>{},{},{}:", x, y, radius),
            IgsCommand::Ellipse { x, y, x_radius, y_radius } => write!(f, "G#Q>{},{},{},{}:", x, y, x_radius, y_radius),
            IgsCommand::Arc {
                x,
                y,
                start_angle,
                end_angle,
                radius,
            } => {
                write!(f, "G#K>{},{},{},{},{}:", x, y, radius, start_angle, end_angle)
            }
            IgsCommand::PolyLine { points } => {
                write!(f, "G#z>{}", points.len() / 2)?;
                for point in points {
                    write!(f, ",{}", point)?;
                }
                write!(f, ":")
            }
            IgsCommand::PolyFill { points } => {
                write!(f, "G#f>{}", points.len() / 2)?;
                for point in points {
                    write!(f, ",{}", point)?;
                }
                write!(f, ":")
            }
            IgsCommand::FloodFill { x, y } => write!(f, "G#F>{},{}:", x, y),
            IgsCommand::PolymarkerPlot { x, y } => write!(f, "G#P>{},{}:", x, y),
            IgsCommand::ColorSet { pen, color } => write!(f, "G#C>{},{}:", pen, color),
            IgsCommand::AttributeForFills {
                pattern_type,
                pattern_index,
                border,
            } => {
                let border_val = if *border { 1 } else { 0 };
                write!(f, "G#A>{},{},{}:", pattern_type, pattern_index, border_val)
            }
            IgsCommand::LineStyle { kind, style, value } => write!(f, "G#T>{},{},{}:", kind, style, value),
            IgsCommand::SetPenColor { pen, red, green, blue } => {
                write!(f, "G#S>{},{},{},{}:", pen, red, green, blue)
            }
            IgsCommand::DrawingMode { mode } => write!(f, "G#M>{}:", mode),
            IgsCommand::HollowSet { enabled } => {
                let val = if *enabled { 1 } else { 0 };
                write!(f, "G#H>{}:", val)
            }
            IgsCommand::WriteText { x, y, text } => {
                // Spec-compliant format: G#W>x,y,text@ (no justification parameter)
                // The text follows directly after the second parameter separator
                write!(f, "G#W>{},{},{}@", x, y, text)
            }
            IgsCommand::TextEffects { effects, size, rotation } => {
                write!(f, "G#E>{},{},{}:", effects, size, rotation)
            }
            IgsCommand::BellsAndWhistles { sound_number } => write!(f, "G#b>{}:", sound_number),
            IgsCommand::AlterSoundEffect {
                play_flag,
                snd_num,
                element_num,
                negative_flag,
                thousands,
                hundreds,
            } => {
                write!(
                    f,
                    "G#b>20,{},{},{},{},{},{}:",
                    play_flag, snd_num, element_num, negative_flag, thousands, hundreds
                )
            }
            IgsCommand::StopAllSound => write!(f, "G#b>21:"),
            IgsCommand::RestoreSoundEffect { snd_num } => write!(f, "G#b>22,{}:", snd_num),
            IgsCommand::SetEffectLoops { count } => write!(f, "G#b>23,{}:", count),
            IgsCommand::GraphicScaling { mode } => {
                write!(f, "G#g>{}:", mode)
            }
            IgsCommand::GrabScreen { blit_type, mode, params } => {
                write!(f, "G#G>{},{}", blit_type, mode)?;
                for param in params {
                    write!(f, ",{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::Initialize { mode } => write!(f, "G#I>{}:", mode),
            IgsCommand::EllipticalArc {
                x,
                y,
                x_radius,
                y_radius,
                start_angle,
                end_angle,
            } => {
                write!(f, "G#J>{},{},{},{},{},{}:", x, y, x_radius, y_radius, start_angle, end_angle)
            }
            IgsCommand::Cursor { mode } => write!(f, "G#k>{}:", mode),
            IgsCommand::ChipMusic {
                effect,
                voice,
                volume,
                pitch,
                timing,
                stop_type,
            } => {
                write!(f, "G#n>{},{},{},{},{},{}:", effect, voice, volume, pitch, timing, stop_type)
            }
            IgsCommand::Noise { params } => {
                write!(f, "G#N")?;
                for param in params {
                    write!(f, ">{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::RoundedRectangles { x1, y1, x2, y2, fill } => {
                let fill_val = if *fill { 1 } else { 0 };
                write!(f, "G#U>{},{},{},{},{}:", x1, y1, x2, y2, fill_val)
            }
            IgsCommand::PieSlice {
                x,
                y,
                radius,
                start_angle,
                end_angle,
            } => {
                write!(f, "G#V>{},{},{},{},{}:", x, y, radius, start_angle, end_angle)
            }
            IgsCommand::EllipticalPieSlice {
                x,
                y,
                x_radius,
                y_radius,
                start_angle,
                end_angle,
            } => {
                write!(f, "G#Y>{},{},{},{},{},{}:", x, y, x_radius, y_radius, start_angle, end_angle)
            }
            IgsCommand::FilledRectangle { x1, y1, x2, y2 } => {
                write!(f, "G#Z>{},{},{},{}:", x1, y1, x2, y2)
            }
            IgsCommand::InputCommand { input_type, params } => {
                write!(f, "G#<>{}", input_type)?;
                for param in params {
                    write!(f, ",{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::AskIG { query } => write!(f, "G#?>{}:", query),
            IgsCommand::ScreenClear { mode } => write!(f, "G#s>{}:", mode),
            IgsCommand::SetResolution { resolution, palette } => write!(f, "G#R>{},{}:", resolution, palette),
            IgsCommand::PauseSeconds { seconds } => write!(f, "G#t>{}:", seconds),
            IgsCommand::VsyncPause { vsyncs } => write!(f, "G#q>{}:", vsyncs),
            IgsCommand::Loop(data) => {
                write!(f, "G#&>{},{},{},{}", data.from, data.to, data.step, data.delay)?;

                // Render target + modifiers
                match &data.target {
                    LoopTarget::Single(ch) => {
                        write!(f, ",{}", ch)?;
                    }
                    LoopTarget::ChainGang { raw, .. } => {
                        write!(f, ",{}", raw)?;
                    }
                }

                if data.modifiers.xor_stepping {
                    write!(f, "|")?;
                }
                if data.modifiers.refresh_text_each_iteration {
                    write!(f, "@")?;
                }

                write!(f, ",{}", data.param_count)?;

                let mut last_was_colon = false;
                for token in &data.params {
                    match token {
                        LoopParamToken::GroupSeparator => {
                            write!(f, ":")?;
                            last_was_colon = true;
                        }
                        LoopParamToken::Number(n) => {
                            if !last_was_colon {
                                write!(f, ",")?;
                            }
                            write!(f, "{}", n)?;
                            last_was_colon = false;
                        }
                        LoopParamToken::Symbol(c) => {
                            if !last_was_colon {
                                write!(f, ",")?;
                            }
                            write!(f, "{}", c)?;
                            last_was_colon = false;
                        }
                        LoopParamToken::Expr(s) => {
                            if !last_was_colon {
                                write!(f, ",")?;
                            }
                            write!(f, "{}", s)?;
                            last_was_colon = false;
                        }
                    }
                }

                write!(f, ":")
            }
            IgsCommand::SprayPaint { x, y, width, height, density } => {
                write!(f, "G#X>0,{},{},{},{},{}:", x, y, width, height, density)
            }
            IgsCommand::SetColorRegister { register, value } => {
                write!(f, "G#X>1,{},{}:", register, value)
            }
            IgsCommand::SetRandomRange { params } => {
                write!(f, "G#X>2")?;
                for param in params {
                    write!(f, ",{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::RightMouseMacro { params } => {
                write!(f, "G#X>3")?;
                for param in params {
                    write!(f, ",{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::DefineZone {
                zone_id,
                x1,
                y1,
                x2,
                y2,
                length,
                string,
            } => {
                // Special case: zone_id 9997-9999 are clear/loopback commands with no additional params
                if (9997..=9999).contains(zone_id) {
                    write!(f, "G#X>4,{}:", zone_id)
                } else {
                    write!(f, "G#X>4,{},{},{},{},{},{},{}:", zone_id, x1, y1, x2, y2, length, string)
                }
            }
            IgsCommand::FlowControl { mode, params } => {
                write!(f, "G#X>5,{}", mode)?;
                for param in params {
                    write!(f, ",{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::LeftMouseButton { mode } => {
                write!(f, "G#X>6,{}:", mode)
            }
            IgsCommand::LoadFillPattern { pattern, data } => {
                write!(f, "G#X>7,{},{}:", pattern, data)
            }
            IgsCommand::RotateColorRegisters {
                start_reg,
                end_reg,
                count,
                delay,
            } => {
                write!(f, "G#X>8,{},{},{},{}:", start_reg, end_reg, count, delay)
            }
            IgsCommand::LoadMidiBuffer { params } => {
                write!(f, "G#X>9")?;
                for param in params {
                    write!(f, ",{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::SetDrawtoBegin { x, y } => {
                write!(f, "G#X>10,{},{}:", x, y)
            }
            IgsCommand::LoadBitblitMemory { params } => {
                write!(f, "G#X>11")?;
                for param in params {
                    write!(f, ",{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::LoadColorPalette { params } => {
                write!(f, "G#X>12")?;
                for param in params {
                    write!(f, ",{}", param)?;
                }
                write!(f, ":")
            }
            IgsCommand::SetForeground { color } => write!(f, "\x1bb{}", *color as u8 as char),
            IgsCommand::SetBackground { color } => write!(f, "\x1bc{}", *color as u8 as char),
            IgsCommand::DeleteLine { count } => write!(f, "\x1bd{}", *count as u8 as char),
            IgsCommand::InsertLine { mode: _, count } => {
                // Emit VT52 ESC form to match input format
                write!(f, "\x1bi{}", *count as u8 as char)
            }
            IgsCommand::ClearLine { mode } => {
                // Emit VT52 ESC form; parameter only if non-zero
                if *mode == 0 {
                    write!(f, "\x1bl")
                } else {
                    write!(f, "\x1bl{}", *mode as u8 as char)
                }
            }
            IgsCommand::CursorMotion { direction, count } => {
                // Emit VT52 ESC style for round-trip of ESC m sequences
                write!(f, "\x1bm{},{}", direction, count)
            }
            IgsCommand::PositionCursor { x, y } => write!(f, "G#p>{},{}:", x, y),
            IgsCommand::RememberCursor { value } => {
                // Emit VT52 ESC form; parameter only if non-zero
                if *value == 0 {
                    write!(f, "\x1br")
                } else {
                    write!(f, "\x1br{}", *value as u8 as char)
                }
            }
            IgsCommand::InverseVideo { enabled } => {
                let val = if *enabled { 1 } else { 0 };
                write!(f, "G#v>{}:", val)
            }
            IgsCommand::LineWrap { enabled } => {
                let val = if *enabled { 1 } else { 0 };
                write!(f, "G#w>{}:", val)
            }
        }
    }
}
