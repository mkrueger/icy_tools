use super::*;
use std::fmt;

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
        x1: IgsParameter,
        y1: IgsParameter,
        x2: IgsParameter,
        y2: IgsParameter,
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
        x1: IgsParameter,
        y1: IgsParameter,
        x2: IgsParameter,
        y2: IgsParameter,
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
    LineDrawTo { x: IgsParameter, y: IgsParameter },

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
    Circle { x: IgsParameter, y: IgsParameter, radius: IgsParameter },

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
        x: IgsParameter,
        y: IgsParameter,
        x_radius: IgsParameter,
        y_radius: IgsParameter,
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
        x: IgsParameter,
        y: IgsParameter,
        radius: IgsParameter,
        start_angle: IgsParameter,
        end_angle: IgsParameter,
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
    PolyLine { points: Vec<IgsParameter> },

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
    PolyFill { points: Vec<IgsParameter> },

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
    FloodFill { x: IgsParameter, y: IgsParameter },

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
    PolymarkerPlot { x: IgsParameter, y: IgsParameter },

    // Color/Style commands
    /// Color selection command (C)
    ///
    /// IGS: `G#C>pen,color:`
    ///
    /// Selects which pen number (0-15) to use for different screen operations.
    /// This maps a logical pen to a color palette entry.
    ///
    /// # Parameters
    /// * `pen` - Screen operation to set (see PenType enum)
    /// * `color` - Pen number to use (0-15)
    ColorSet { pen: PenType, color: u8 },

    /// Fill attributes command (A)
    ///
    /// IGS: `G#A>pattern_type,pattern_index,border:`
    ///
    /// Sets attributes for all fill operations including boxes, circles, polygons.
    /// Controls the fill pattern type, specific pattern, and whether borders are drawn.
    ///
    /// # Parameters
    /// * `pattern_type` - Fill type (Hollow, Solid, Pattern(1-24), Hatch(1-12), UserDefined(0-9))
    /// * `border` - Draw border around filled areas: `true` for yes, `false` for no
    AttributeForFills { pattern_type: PatternType, border: bool },

    /// Line and marker style command (T)
    ///
    /// IGS: `G#T>type,style,size:`
    ///
    /// Controls the appearance of lines and polymarkers including style, thickness,
    /// and end decorations (arrows, rounded ends).
    ///
    /// # Parameters
    /// * `kind` - Style kind (Polymarker or Line with specific type)
    /// * `value` - Third parameter: size/thickness/pattern/end-style composite
    ///   - For polymarkers: size 1-8
    ///   - For solid lines: thickness 1-41
    ///   - For user defined lines: pattern number 1-32
    ///   - Line end styles (add to size): 0=square, 50=arrows both, 51=arrow left, 52=arrow right, 60=rounded
    LineStyle { kind: LineStyleKind, value: u16 },

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
    SetPenColor { pen: u8, red: u8, green: u8, blue: u8 },

    /// Drawing mode command (M)
    ///
    /// IGS: `G#M>mode:`
    ///
    /// Sets the logical operation used when drawing pixels, allowing for
    /// effects like XOR drawing for erasable graphics.
    ///
    /// # Parameters
    /// * `mode` - Drawing mode (Replace, Transparent, Xor, or ReverseTransparent)
    DrawingMode { mode: DrawingMode },

    /// Hollow/filled toggle command (H)
    ///
    /// IGS: `G#H>enabled:`
    ///
    /// Controls whether shapes are drawn filled or as outlines only.
    /// When enabled, circles become rings, rectangles become frames, etc.
    ///
    /// # Parameters
    /// * `enabled` - `true` for hollow (outline only), `false` for filled
    HollowSet { enabled: bool },

    // Text commands
    /// Write text command (W)
    ///
    /// IGS: `G#W>x,y,text@`
    ///
    /// Spec (IG220) shows only X, Y and a text string terminated with `@`.
    /// A justification parameter is not documented; older files may include an
    /// extra numeric which we intentionally omit for spec conformity.
    WriteText { x: IgsParameter, y: IgsParameter, text: Vec<u8> },

    /// Text effects command (E)
    ///
    /// IGS: `G#E>effects,size,rotation:`
    ///
    /// Sets VDI text attributes for text drawn with `WriteText` including
    /// style effects, point size, and rotation angle.
    ///
    /// # Parameters
    /// * `effects` - Text effect flags (can be combined: THICKENED=1, GHOSTED=2, SKEWED=4, UNDERLINED=8, OUTLINED=16)
    /// * `size` - Text size in points (1/72 inch). Common sizes: 8, 9, 10, 16, 18, 20
    /// * `rotation` - Text rotation angle (Degrees0, Degrees90, Degrees180, Degrees270)
    TextEffects { effects: TextEffects, size: u8, rotation: TextRotation },

    // Special commands
    /// Sound effects command (b)
    ///
    /// IGS: `G#b>sound_number:`
    ///
    /// Plays predefined sound effects using the ST's sound chip.
    /// Effects 0-4 are internally looped 5*200 times by default (settable via b 23).
    ///
    /// # Sound Effects (0-19)
    /// * 0: Alien Invasion
    /// * 1: Red Alert
    /// * 2: Gunshot
    /// * 3: Laser 1
    /// * 4: Jackhammer
    /// * 5: Teleport
    /// * 6: Explosion
    /// * 7: Laser 2
    /// * 8: Long Bell
    /// * 9: Surprise
    /// * 10: Radio Broadcast
    /// * 11: Bounce Ball
    /// * 12: Eerie Sound
    /// * 13: Harley Motorcycle
    /// * 14: Helicopter
    /// * 15: Steam Locomotive
    /// * 16: Wave
    /// * 17: Robot Walk
    /// * 18: Passing Plane
    /// * 19: Landing
    ///
    /// # Extended Commands (handled separately)
    /// * 20: AlterSoundEffect - Modifies sound effect parameters
    /// * 21: StopAllSound - Stops all playing effects immediately
    /// * 22: RestoreSoundEffect - Restores altered effect to original
    /// * 23: SetEffectLoops - Sets loop count for effects 0-4
    BellsAndWhistles { sound_effect: SoundEffect },

    /// Alter sound effect command (b 20)
    ///
    /// IGS: `G#b>20,play_flag,sound_num,element_num,negative_flag,thousands,hundreds:`
    ///
    /// Modifies parameters of sound effects 0-19. Allows customizing pitch, duration,
    /// and other characteristics. Changes persist until RestoreSoundEffect is called.
    ///
    /// # Parameters
    /// * `play_flag` - 0=don't play after altering, 1=play immediately
    /// * `sound_effect` - Sound effect number to alter (0-19)
    /// * `element_num` - Which parameter to modify (effect-specific)
    /// * `negative_flag` - 0=positive value, 1=negative value
    /// * `thousands` - Thousands digit of value
    /// * `hundreds` - Last 3 digits of value (0-999)
    ///
    /// # Example
    /// `G#b>20,1,0,0,0,1,500:` - Alters element 0 of Alien Invasion to 1500, then plays
    AlterSoundEffect {
        play_flag: u8,
        sound_effect: SoundEffect,
        element_num: u8,
        negative_flag: u8,
        thousands: u16,
        hundreds: u16,
    },

    /// Stop all sound command (b 21)
    ///
    /// IGS: `G#b>21:`
    ///
    /// Immediately stops all currently playing sound effects and music.
    /// Useful for silencing overlapping effects or canceling long-running sounds.
    StopAllSound,

    /// Restore sound effect command (b 22)
    ///
    /// IGS: `G#b>22,sound_num:`
    ///
    /// Restores a sound effect to its original parameters after being altered
    /// by the AlterSoundEffect command. Only affects the specified effect.
    ///
    /// # Parameters
    /// * `sound_effect` - Sound effect number to restore (0-19)
    RestoreSoundEffect { sound_effect: SoundEffect },

    /// Set effect loops command (b 23)
    ///
    /// IGS: `G#b>23,count:`
    ///
    /// Sets how many times sound effects 0-4 are looped when played.
    /// Default is 5*200 (1000) loops. Change persists until reset or program exit.
    ///
    /// # Parameters
    /// * `count` - Number of times to loop effects 0-4
    ///
    /// # Note
    /// Only affects effects 0-4: Alien Invasion, Red Alert, Gunshot, Laser 1, Jackhammer
    SetEffectLoops { count: u32 },

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
    GraphicScaling { mode: u8 },

    /// Screen grab/BitBlit command (G)
    ///
    /// IGS: `G#G>type,mode,...:`
    ///
    /// Performs bit-block transfer operations for copying screen regions,
    /// saving to memory, or restoring from memory. Supports various logical
    /// operations for combining source and destination.
    ///
    /// # Parameters
    /// * `operation` - The blit operation with its specific parameters
    /// * `mode` - Logical operation (Clear, Replace, Xor, Transparent, Fill, etc.)
    GrabScreen { operation: BlitOperation, mode: BlitMode },

    /// Initialize command (I)
    ///
    /// IGS: `G#I>mode:`
    ///
    /// Initializes the IG environment, resetting colors, attributes, and optionally
    /// the resolution. Should be called at the start of each graphics sequence
    /// for a consistent starting point.
    ///
    /// # Parameters
    /// * `mode` - Initialization type (see InitializationType enum)
    ///
    /// # Note
    /// Mode DesktopResolutionAndClipping should be used FIRST before any palette commands
    Initialize { mode: InitializationType },

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
        x: IgsParameter,
        y: IgsParameter,
        x_radius: IgsParameter,
        y_radius: IgsParameter,
        start_angle: IgsParameter,
        end_angle: IgsParameter,
    },

    /// Cursor control command (k)
    ///
    /// IGS: `G#k>mode:`
    ///
    /// Controls text cursor visibility and backspace behavior.
    ///
    /// # Parameters
    /// * `mode` - Cursor mode (see CursorMode enum)
    Cursor { mode: CursorMode },

    /// Chip music command (n)
    ///
    /// IGS: `G#n>effect,voice,volume,pitch,timing,stop_type:`
    ///
    /// Plays musical notes using sound effects as instruments. Allows multi-voice
    /// music with timing control. No flow control during playback.
    ///
    /// # Parameters
    /// * `sound_effect` - Sound effect to use as instrument (see SoundEffect enum)
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
        sound_effect: SoundEffect,
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
    Noise { params: Vec<IgsParameter> },

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
        x1: IgsParameter,
        y1: IgsParameter,
        x2: IgsParameter,
        y2: IgsParameter,
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
        x: IgsParameter,
        y: IgsParameter,
        radius: IgsParameter,
        start_angle: IgsParameter,
        end_angle: IgsParameter,
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
        x: IgsParameter,
        y: IgsParameter,
        x_radius: IgsParameter,
        y_radius: IgsParameter,
        start_angle: IgsParameter,
        end_angle: IgsParameter,
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
        x1: IgsParameter,
        y1: IgsParameter,
        x2: IgsParameter,
        y2: IgsParameter,
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
    InputCommand { input_type: u8, params: Vec<IgsParameter> },

    /// Ask IG command (?)
    ///
    /// IGS: `G#?>query[,param]:`
    ///
    /// Queries the IG terminal for information and transmits the response
    /// back to the host system.
    ///
    /// # Parameters
    /// * `query` - Information to request (see `AskQuery` enum)
    ///
    /// # Examples
    /// - `G#?>0:` - Query version number
    /// - `G#?>1,0:` - Query cursor position (immediate)
    /// - `G#?>2,3:` - Query mouse position with arrow pointer
    /// - `G#?>3:` - Query current resolution
    AskIG { query: AskQuery },

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
    ScreenClear { mode: u8 },

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
    /// Set resolution and palette (R)
    ///
    /// IGS: `G#R>resolution,palette:`
    ///
    /// Changes terminal resolution and optionally loads a palette.
    ///
    /// # Parameters
    /// * `resolution` - Terminal resolution mode:
    ///   - Low: 320×200, 16 colors
    ///   - Medium: 640×200, 4 colors
    ///   - High: 640×400, 2 colors
    /// * `palette` - Palette mode:
    ///   - NoChange: No palette change
    ///   - Desktop: Desktop colors
    ///   - IgDefault: IG default palette
    ///   - VdiDefault: VDI default palette
    SetResolution { resolution: TerminalResolution, palette: PaletteMode },

    /// Time pause command (t)
    ///
    /// IGS: `G#t>seconds:`
    ///
    /// Sends ^S (XOFF), waits for specified seconds, then sends ^Q (XON).
    /// Any key press aborts the pause prematurely. Used to prevent BBS timeouts
    /// during graphics display. Maximum 30 seconds; chain multiple for longer pauses.
    ///
    /// # Parameters
    /// * `seconds` - Number of seconds to pause (max 30)
    ///
    /// # Example
    /// `G#t>30:t>5:` - Pauses for 35 seconds total (30 + 5)
    PauseSeconds { seconds: u8 },

    /// Vsync pause command (q)
    ///
    /// IGS: `G#q>vsyncs:`
    ///
    /// Pauses execution for specified vertical syncs (1/60th second each).
    /// Vsync() waits until the screen's next vertical blank, ensuring smooth animations.
    /// Also controls internal double-stepping for BitBlit's GrabScreen command.
    ///
    /// # Parameters
    /// * `vsyncs` - Number of vertical syncs to wait, or special values:
    ///   - 0-9994: Normal pause for N vsyncs (max ~180 recommended)
    ///   - 9995: Enable double-step with 3 vsync delay
    ///   - 9996: Enable double-step with 2 vsync delay
    ///   - 9997: Enable double-step with 1 vsync delay
    ///   - 9998: Enable double-step with 0 vsync delay
    ///   - 9999: Disable double-stepping
    ///
    /// # Note
    /// Double-step mode makes GrabScreen command internally step by 2 for XOR operations
    VsyncPause { vsyncs: i32 },

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
        x: IgsParameter,
        y: IgsParameter,
        width: IgsParameter,
        height: IgsParameter,
        density: IgsParameter,
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
    SetColorRegister { register: u8, value: i32 },

    /// Set random number range (X 2)
    ///
    /// IGS: `G#X>2,min,max:` or `G#X>2,min,min,max:`
    ///
    /// Sets the range for random values when 'r' or 'R' is used in
    /// command parameters. Default range is 0-199.
    ///
    /// # Parameters
    /// * `range_type` - Small for 'r' [min,max] or Big for 'R' [min,min,max]
    SetRandomRange { range_type: RandomRangeType },

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
    RightMouseMacro { params: Vec<IgsParameter> },

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
        x1: IgsParameter,
        y1: IgsParameter,
        x2: IgsParameter,
        y2: IgsParameter,
        length: u16,
        string: Vec<u8>,
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
    FlowControl { mode: u8, params: Vec<IgsParameter> },

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
    LeftMouseButton { mode: u8 },

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
    LoadFillPattern { pattern: u8, data: Vec<u8> },

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
    RotateColorRegisters { start_reg: u8, end_reg: u8, count: i32, delay: i32 },

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
    LoadMidiBuffer { params: Vec<IgsParameter> },

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
    SetDrawtoBegin { x: IgsParameter, y: IgsParameter },

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
    LoadBitblitMemory { params: Vec<IgsParameter> },

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
    LoadColorPalette { params: Vec<IgsParameter> },

    // IGS-specific color commands (ESC b/c are IGS extensions, not standard VT52)
    /// Set text color (ESC b/c or G#c)
    ///
    /// IGS: `G#c>layer,color:` where layer is 0 (background) or 1 (foreground)
    /// VT52: `\x1bb[color]` (foreground) or `\x1bc[color]` (background)
    ///
    /// Sets either the foreground or background color for VT52 text.
    ///
    /// # Parameters
    /// * `layer` - Which color layer to modify (Background or Foreground)
    /// * `color` - Color register (0-15)
    SetTextColor { layer: TextColorLayer, color: u8 },

    // VT52 additional IGS commands (not removed because they have IGS G# equivalents)
    /// Delete lines (ESC d)
    ///
    /// IGS: `G#d>count:`
    ///
    /// Deletes specified number of lines starting at cursor.
    /// Lines below scroll up to fill the gap.
    ///
    /// # Parameters
    /// * `count` - Number of lines to delete
    DeleteLine { count: u8 },

    /// Insert lines (ESC i)
    ///
    /// IGS: `G#i>mode,count:`
    ///
    /// Inserts blank lines at cursor position.
    /// Mode parameter is only present in IG form; ESC form omits it.
    ///
    /// # Parameters
    /// * `mode` - Insertion mode (0 = normal). If absent (ESC form) use 0.
    /// * `count` - Number of lines to insert
    InsertLine { mode: u8, count: u8 },

    /// Clear line (ESC l)
    ///
    /// IGS: `G#l>mode:`
    ///
    /// Clears the current line. Mode parameter only in IG form; ESC form implies 0.
    ///
    /// # Parameters
    /// * `mode` - 0 = clear line & keep cursor column, other modes reserved
    ClearLine { mode: u8 },

    /// Cursor motion (ESC m)
    ///
    /// IGS: `G#m>direction,count:`
    ///
    /// Moves cursor by specified amount in given direction.
    /// Two encodings exist; unified here. For IG form use direction/count.
    /// For ESC form store x/y mapping to direction/count internally.
    ///
    /// # Parameters
    /// * `direction` - Direction to move (Up, Down, Left, Right)
    /// * `count` - Number of positions to move
    CursorMotion { direction: crate::Direction, count: i32 },

    /// Position cursor (p)
    ///
    /// IGS: `G#p>column,row:`
    ///
    /// Positions cursor at character column and row.
    ///
    /// # Parameters
    /// * `x` - Column (0-79)
    /// * `y` - Row (0-24)
    PositionCursor { x: IgsParameter, y: IgsParameter },

    /// Remember cursor position (ESC r)
    ///
    /// IGS: `G#r>value:`
    ///
    /// Remembers current cursor position for recall.
    /// Different from save/restore cursor commands. Value only present in IG form.
    ///
    /// # Parameters
    /// * `value` - Usually 0; reserved for future use.
    RememberCursor { value: u8 },

    /// Inverse video toggle (v)
    ///
    /// IGS: `G#v>enabled:`
    ///
    /// Toggles inverse video mode where foreground and background
    /// colors are swapped for text display.
    ///
    /// # Parameters
    /// * `enabled` - `true` for inverse, `false` for normal
    InverseVideo { enabled: bool },

    /// Line wrap toggle (w)
    ///
    /// IGS: `G#w>enabled:`
    ///
    /// Controls whether text wraps to next line at right margin.
    ///
    /// # Parameters
    /// * `enabled` - `true` for wrap enabled, `false` for disabled
    LineWrap { enabled: bool },
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
            IgsCommand::ColorSet { pen, color } => write!(f, "G#C>{},{}:", *pen as u8, color),
            IgsCommand::AttributeForFills { pattern_type, border } => {
                let (type_val, index_val) = match pattern_type {
                    PatternType::Hollow => (0, 1),
                    PatternType::Solid => (1, 1),
                    PatternType::Pattern(idx) => (2, *idx),
                    PatternType::Hatch(idx) => (3, *idx),
                    PatternType::UserDefined(idx) => (4, *idx),
                };
                let border_val = if *border { 1 } else { 0 };
                write!(f, "G#A>{},{},{}:", type_val, index_val, border_val)
            }
            IgsCommand::LineStyle { kind, value } => {
                let (type_val, style_val) = match kind {
                    LineStyleKind::Polymarker(pk) => (1, *pk as u8),
                    LineStyleKind::Line(lk) => (2, *lk as u8),
                };
                write!(f, "G#T>{},{},{}:", type_val, style_val, value)
            }
            IgsCommand::SetPenColor { pen, red, green, blue } => {
                write!(f, "G#S>{},{},{},{}:", pen, red, green, blue)
            }
            IgsCommand::DrawingMode { mode } => write!(f, "G#M>{}:", *mode as u8),
            IgsCommand::HollowSet { enabled } => {
                let val = if *enabled { 1 } else { 0 };
                write!(f, "G#H>{}:", val)
            }
            IgsCommand::WriteText { x, y, text } => {
                // Spec-compliant format: G#W>x,y,text@ (no justification parameter)
                // The text follows directly after the second parameter separator
                write!(f, "G#W>{},{},{}@", x, y, String::from_utf8_lossy(text))
            }
            IgsCommand::TextEffects { effects, size, rotation } => {
                write!(f, "G#E>{},{},{}:", effects.bits(), size, *rotation as u8)
            }
            IgsCommand::BellsAndWhistles { sound_effect } => write!(f, "G#b>{}:", *sound_effect as u8),
            IgsCommand::AlterSoundEffect {
                play_flag,
                sound_effect,
                element_num,
                negative_flag,
                thousands,
                hundreds,
            } => {
                write!(
                    f,
                    "G#b>20,{},{},{},{},{},{}:",
                    play_flag, *sound_effect as u8, element_num, negative_flag, thousands, hundreds
                )
            }
            IgsCommand::StopAllSound => write!(f, "G#b>21:"),
            IgsCommand::RestoreSoundEffect { sound_effect } => write!(f, "G#b>22,{}:", *sound_effect as u8),
            IgsCommand::SetEffectLoops { count } => write!(f, "G#b>23,{}:", count),
            IgsCommand::GraphicScaling { mode } => {
                write!(f, "G#g>{}:", mode)
            }
            IgsCommand::GrabScreen { operation, mode } => match operation {
                BlitOperation::ScreenToScreen {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                    dest_x,
                    dest_y,
                } => {
                    write!(f, "G#G>0,{},{},{},{},{},{},{}:", *mode as u8, src_x1, src_y1, src_x2, src_y2, dest_x, dest_y)
                }
                BlitOperation::ScreenToMemory {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                } => {
                    write!(f, "G#G>1,{},{},{},{},{}:", *mode as u8, src_x1, src_y1, src_x2, src_y2)
                }
                BlitOperation::MemoryToScreen { dest_x, dest_y } => {
                    write!(f, "G#G>2,{},{},{}:", *mode as u8, dest_x, dest_y)
                }
                BlitOperation::PieceOfMemoryToScreen {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                    dest_x,
                    dest_y,
                } => {
                    write!(f, "G#G>3,{},{},{},{},{},{},{}:", *mode as u8, src_x1, src_y1, src_x2, src_y2, dest_x, dest_y)
                }
                BlitOperation::MemoryToMemory {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                    dest_x,
                    dest_y,
                } => {
                    write!(f, "G#G>4,{},{},{},{},{},{},{}:", *mode as u8, src_x1, src_y1, src_x2, src_y2, dest_x, dest_y)
                }
            },
            IgsCommand::Initialize { mode } => write!(f, "G#I>{}:", *mode as u8),
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
            IgsCommand::Cursor { mode } => write!(f, "G#k>{}:", *mode as u8),
            IgsCommand::ChipMusic {
                sound_effect,
                voice,
                volume,
                pitch,
                timing,
                stop_type,
            } => {
                write!(f, "G#n>{},{},{},{},{},{}:", *sound_effect as u8, voice, volume, pitch, timing, stop_type)
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
            IgsCommand::AskIG { query } => match query {
                AskQuery::VersionNumber => write!(f, "G#?>0:"),
                AskQuery::CursorPositionAndMouseButton { pointer_type } => {
                    write!(f, "G#?>1,{}:", *pointer_type as i32)
                }
                AskQuery::MousePositionAndButton { pointer_type } => {
                    write!(f, "G#?>2,{}:", *pointer_type as i32)
                }
                AskQuery::CurrentResolution => write!(f, "G#?>3:"),
            },
            IgsCommand::ScreenClear { mode } => write!(f, "G#s>{}:", mode),
            IgsCommand::SetResolution { resolution, palette } => write!(f, "G#R>{},{}:", *resolution as u8, *palette as u8),
            IgsCommand::PauseSeconds { seconds } => write!(f, "G#t>{}:", seconds),
            IgsCommand::VsyncPause { vsyncs } => write!(f, "G#q>{}:", vsyncs),
            IgsCommand::Loop(data) => {
                write!(f, "G#&>{},{},{},{}", data.from, data.to, data.step, data.delay)?;

                // Render target + modifiers
                match &data.target {
                    LoopTarget::Single(cmd_type) => {
                        write!(f, ",{}", cmd_type.to_char())?;
                    }
                    LoopTarget::ChainGang { commands } => {
                        write!(f, ",>")?;
                        for cmd in commands {
                            write!(f, "{}", cmd.to_char())?;
                        }
                        write!(f, "@")?;
                    }
                }

                if data.modifiers.xor_stepping {
                    write!(f, "|")?;
                }
                let has_modifiers = data.modifiers.xor_stepping || data.modifiers.refresh_text_each_iteration;
                if data.modifiers.refresh_text_each_iteration {
                    write!(f, "@")?;
                }

                // No comma before param_count if command has modifiers (| or @)
                // Spec format: W@2 not W@,2
                if !has_modifiers {
                    write!(f, ",")?;
                }
                write!(f, "{}", data.param_count)?;

                let mut last_was_colon = false;
                let mut last_was_text = false;
                for token in &data.params {
                    match token {
                        LoopParamToken::GroupSeparator => {
                            write!(f, ":")?;
                            last_was_colon = true;
                            last_was_text = false;
                        }
                        LoopParamToken::Number(n) => {
                            if !last_was_colon {
                                write!(f, ",")?;
                            }
                            write!(f, "{}", n)?;
                            last_was_colon = false;
                            last_was_text = false;
                        }
                        LoopParamToken::StepForward => {
                            if !last_was_colon {
                                write!(f, ",")?;
                            }
                            write!(f, "x")?;
                            last_was_colon = false;
                            last_was_text = false;
                        }
                        LoopParamToken::StepReverse => {
                            if !last_was_colon {
                                write!(f, ",")?;
                            }
                            write!(f, "y")?;
                            last_was_colon = false;
                            last_was_text = false;
                        }
                        LoopParamToken::Random => {
                            if !last_was_colon {
                                write!(f, ",")?;
                            }
                            write!(f, "r")?;
                            last_was_colon = false;
                            last_was_text = false;
                        }
                        LoopParamToken::Expr(op, val) => {
                            if !last_was_colon {
                                write!(f, ",")?;
                            }
                            write!(f, "{}{}", (*op as u8) as char, val)?;
                            last_was_colon = false;
                            last_was_text = false;
                        }
                        LoopParamToken::Text(bytes) => {
                            // Text strings: only add comma before first text (after numeric params)
                            // Text strings are separated by their @ terminators, no comma between them
                            if !last_was_colon && !last_was_text {
                                write!(f, ",")?;
                            }
                            write!(f, "{}", String::from_utf8_lossy(bytes))?;
                            write!(f, "@")?;
                            last_was_colon = false;
                            last_was_text = true;
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
            IgsCommand::SetRandomRange { range_type } => {
                write!(f, "G#X>2,{}:", range_type)
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
                    write!(
                        f,
                        "G#X>4,{},{},{},{},{},{},{}:",
                        zone_id,
                        x1,
                        y1,
                        x2,
                        y2,
                        length,
                        String::from_utf8_lossy(string)
                    )
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
                write!(f, "G#X>7,{},{}:", pattern, String::from_utf8_lossy(data))
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
            IgsCommand::SetTextColor { layer, color } => {
                let escape_char = match layer {
                    TextColorLayer::Foreground => 'b',
                    TextColorLayer::Background => 'c',
                };
                write!(f, "\x1b{}{}", escape_char, *color as u8 as char)
            }
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
                let dir_val = match direction {
                    crate::Direction::Up => 0,
                    crate::Direction::Down => 1,
                    crate::Direction::Left => 2,
                    crate::Direction::Right => 3,
                };
                // Emit VT52 ESC style for round-trip of ESC m sequences
                write!(f, "\x1bm{},{}", dir_val, count)
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
