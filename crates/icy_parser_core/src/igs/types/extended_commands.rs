/// Extended command identifiers for IGS X commands.
///
/// Extended commands are invoked by `G#X` followed by a command number (0-9999).
/// This provides a namespace for 10,000 additional commands beyond the basic
/// single-letter command set.
///
/// # Examples
/// ```text
/// G#X 0,400,50,200,145,200:  // Spray paint
/// G#X 1,4,0:                 // Set color register
/// G#X 8,1,15,150,0:          // Rotate color registers
/// ```
pub struct ExtendedCommandId;

impl ExtendedCommandId {
    /// Spray Paint - Plots polymarkers at random in a rectangular area
    ///
    /// # Format
    /// `G#X 0,x,y,width,height,concentration:`
    ///
    /// # Parameters
    /// - `x, y`: Upper left corner coordinates
    /// - `width`: X range from upper left (max 255)
    /// - `height`: Y range from upper left (max 255)
    /// - `concentration`: Number of points (max 9999)
    ///
    /// # Special Usage
    /// Can also control color rotation:
    /// - `G#X 0,1,0,0,0,0:` - Enable rotation from pen 1 to max
    /// - `G#X 0,0,0,0,0,0:` - Disable rotation
    pub const SPRAY_PAINT: i32 = 0;

    /// Set Color Register - Sets a color register directly via Xbios 7
    ///
    /// # Format
    /// `G#X 1,register,value:`
    ///
    /// # Parameters
    /// - `register`: Color register 0-15
    /// - `value`: Color value 0-9999 (ST uses up to 1911, STE may use higher)
    ///
    /// # Note
    /// Unlike the `S` command which sets a pen's register, this sets
    /// a specific hardware color register directly.
    pub const SET_COLOR_REGISTER: i32 = 1;

    /// Set Random Function Range - Defines range for 'r' and 'R' parameters
    ///
    /// # Format
    /// - Small range (r): `G#X 2,min,max:`
    /// - Big range (R): `G#X 2,min,min,max:` (note: min appears twice)
    ///
    /// # Parameters
    /// - `min`: Minimum value (0-9999)
    /// - `max`: Maximum value (0-9999)
    ///
    /// # Default
    /// Both 'r' and 'R' default to 0-199 range at startup
    ///
    /// # Example
    /// ```text
    /// G#X 2,0,639:           // Set 'r' to 0-639
    /// G#X 2,50,50,150:       // Set 'R' to 50-150
    /// L 0,R,400,r:           // Use in line command
    /// ```
    pub const SET_RANDOM_RANGE: i32 = 2;

    /// Right Mouse Button Macro - Associates string with right mouse button
    ///
    /// # Format
    /// - Deactivate: `G#X 3,0:`
    /// - Reactivate: `G#X 3,1,send_cr:`
    /// - Load: `G#X 3,2,active,send_cr,length,string:`
    ///
    /// # Parameters
    /// - `active`: 0=off, 1=on
    /// - `send_cr`: 0=no CR, 1=send CR at end
    /// - `length`: String length (1-80 chars, don't count separator)
    /// - `string`: Text to transmit (ends with separator like `:`)
    ///
    /// # Examples
    /// ```text
    /// G#X 3,0:                    // Turn off
    /// G#X 3,1,1:                  // Reactivate with CR
    /// G#X 3,2,1,1,3,m/a:          // Load "m/a" with CR
    /// G#X 3,2,1,1,30,C'mon Baby:  // Load longer string
    /// ```
    pub const RIGHT_MOUSE_BUTTON_MACRO: i32 = 3;

    /// Define and Load Zone Data - Creates clickable rectangular zones
    ///
    /// # Format
    /// - Clear all: `G#X 4,9999:`
    /// - Loopback on: `G#X 4,9998:`
    /// - Loopback off: `G#X 4,9997:`
    /// - Define zone: `G#X 4,id,x1,y1,x2,y2,length,string:`
    ///
    /// # Parameters
    /// - `id`: Zone number 0-47 (or 9999-9997 for special functions)
    /// - `x1, y1`: Upper left corner
    /// - `x2, y2`: Lower right corner
    /// - `length`: String length (max 80, don't count separator)
    /// - `string`: Text to transmit when zone clicked
    ///
    /// # Special IDs
    /// - 9999: Clear all zones (sets rectangles to -1,-1,-1,-1)
    /// - 9998: Enable zone loopback (force valid zone selection)
    /// - 9997: Disable zone loopback (default)
    ///
    /// # Note
    /// Zone loopback makes the terminal beep and wait for a valid
    /// zone click if an undefined zone is selected.
    pub const DEFINE_ZONE: i32 = 4;

    /// Flow Control Shutdown - Controls XON/XOFF flow control
    ///
    /// # Format
    /// - Simple: `G#X 5,mode:`
    /// - Custom XON: `G#X 5,2,ascii,reps:`
    /// - Custom XOFF: `G#X 5,3,ascii,reps:`
    /// - Reset: `G#X 5,4:`
    ///
    /// # Parameters
    /// - `mode`: 0=off, 1=on, 2=set XON, 3=set XOFF, 4=reset defaults
    /// - `ascii`: ASCII value for XON/XOFF character
    /// - `reps`: Number of repetitions to send
    ///
    /// # Default
    /// IG defaults to ^S (19) once for XOFF, ^Q (17) once for XON
    ///
    /// # Warning
    /// Use with caution - disabling flow control may cause data loss.
    /// Always reset to defaults (mode 4) in logoff scripts.
    pub const FLOW_CONTROL: i32 = 5;

    /// Left Mouse Button CR/LF - Makes left button act as Enter
    ///
    /// # Format
    /// `G#X 6,mode:`
    ///
    /// # Parameters
    /// - `mode`: 0=off (default), 1=CR only, 2=CR+LF
    ///
    /// # Note
    /// Does not affect the `<` input command's mouse zone option.
    /// Useful for hands-free message reading by clicking instead
    /// of using keyboard.
    pub const LEFT_MOUSE_BUTTON_CR: i32 = 6;

    /// Load Fill Pattern - Defines custom 16×16 bit patterns
    ///
    /// # Format
    /// `G#X 7,slot,pattern_data:`
    ///
    /// # Parameters
    /// - `slot`: Pattern number 0-7 (6 and 7 also serve as line patterns)
    /// - `pattern_data`: 16 strings of 17 chars each
    ///   - Character 1-16: 'X' or 'x' = bit set, anything else = bit clear
    ///   - Character 17: '@' terminator
    ///
    /// # Example
    /// ```text
    /// G#X 7,1,
    /// ----------------@
    /// --------XX------@
    /// -------XXXX-----@
    /// ...14 more rows...
    /// ```
    ///
    /// # Note
    /// The Draw program has a Fill Pattern Editor for creating these.
    pub const LOAD_FILL_PATTERN: i32 = 7;

    /// Rotate Color Registers - Animates colors by shifting registers
    ///
    /// # Format
    /// `G#X 8,start,end,count,delay:`
    ///
    /// # Parameters
    /// - `start`: Starting color register
    /// - `end`: Ending color register
    /// - `count`: Number of shifts (0=reset to original)
    /// - `delay`: Time between shifts in 1/200ths of a second (0-9999)
    ///
    /// # Direction
    /// - If start < end: shift right
    /// - If start > end: shift left
    ///
    /// # Reset
    /// `G#X 8,1,1,0,1:` resets all registers to their original values
    ///
    /// # Use Cases
    /// - Animated waterfalls (cycling blue shades)
    /// - Steam effects, flames, lightning
    /// - Scrolling rainbow text
    /// - Works best in low-res (16 colors), limited in med-res, useless in hi-res
    pub const ROTATE_COLOR_REGISTERS: i32 = 8;

    /// IG MIDI Buffer - Load or execute IG commands from a buffer
    ///
    /// # Format
    /// - Load: `G#X 9,0,commands until ||}`
    /// - Execute: `G#X 9,1:`
    /// - Clear: `G#X 9,2:`
    ///
    /// # Parameters
    /// - `mode`: 0=load, 1=execute, 2=clear
    /// - `commands`: Any IG commands (when loading)
    ///
    /// # Buffer Size
    /// 10,001 bytes (~121-140 lines depending on usage)
    ///
    /// # Note
    /// - Shared with `N` (music) command - choose one purpose
    /// - Cannot execute `X 9,1:` from within the buffer (infinite loop trap)
    /// - Load terminates on `||}` sequence
    ///
    /// # Example
    /// ```text
    /// G#X 9,0,G#b>1:L>0,0,100,100:||}  // Load
    /// G#X 9,1:                         // Execute
    /// ```
    ///
    /// # Version
    /// Added in IG218
    pub const MIDI_BUFFER: i32 = 9;

    /// Set Begin Point for DrawTo - Sets starting point for `D` command
    ///
    /// # Format
    /// `G#X 10,x,y:`
    ///
    /// # Parameters
    /// - `x, y`: Starting coordinates for next DrawTo command
    ///
    /// # Note
    /// Similar to `L` or `P` commands but doesn't draw anything.
    /// Just sets the pen position for subsequent `D` commands.
    ///
    /// # Example
    /// ```text
    /// G#X 10,100,50:  // Set position
    /// D>75,100:       // Draw line from (100,50) to (75,100)
    /// ```
    ///
    /// # Version
    /// Added in IG219
    pub const SET_DRAWTO_BEGIN: i32 = 10;

    /// Load or Wipe Screen BitBlit Memory - Manages 32KB screen buffer
    ///
    /// # Format
    /// - Wipe all: `G#X 11,0,0,value:`
    /// - Wipe section: `G#X 11,0,section,value:`
    /// - Load & show all: `G#X 11,1,0:`
    /// - Load & show section: `G#X 11,1,section:`
    /// - Load only (all): `G#X 11,2,0:`
    /// - Load only (section): `G#X 11,2,section:`
    ///
    /// # Parameters
    /// - `mode`: 0=wipe, 1=load & show, 2=load only
    /// - `section`: 0=all 32KB, 1-8=4KB horizontal section (1=top, 8=bottom)
    /// - `value`: Fill value 0-255, 256=random per byte, r/R=one random value
    ///
    /// # Timing
    /// - Full screen: ~30 seconds at 19.2K baud
    /// - One section: ~4 seconds at 19.2K baud
    /// - Section height: 50 pixels (mono), 25 pixels (med/low res)
    ///
    /// # Note
    /// - BitBlit data must be transmitted correctly (full 0-255 byte range)
    /// - Use IGDEV13.PRG to create proper BitBlit memory files
    /// - Load & show always uses REPLACE mode
    /// - Does not handle color palette or resolution automatically
    ///
    /// # Version
    /// Added in IG220
    pub const BITBLIT_MEMORY: i32 = 11;

    /// Load Color Hardware Register Palette - Batch set color registers
    ///
    /// # Format
    /// `G#X 12,group,c0,c1,c2,c3:`
    ///
    /// # Parameters
    /// - `group`: Which set of 4 registers (0-3)
    ///   - 0: registers 0-3
    ///   - 1: registers 4-7
    ///   - 2: registers 8-11
    ///   - 3: registers 12-15
    /// - `c0-c3`: Color values for the 4 registers (0-9999)
    ///
    /// # Resolution Requirements
    /// - Medium res: Call once (4 colors)
    /// - Low res: Call 4 times for all 16 colors
    ///
    /// # Example
    /// ```text
    /// G#X 12,0,0,1911,1792,112:  // Set registers 0-3
    /// G#X 12,1,256,512,768,1024: // Set registers 4-7
    /// ```
    ///
    /// # Note
    /// IGDEV13 program generates proper palette values when using
    /// "SET Palette/Save whole Palette" or when grabbing screen with ALT-L.
    pub const LOAD_COLOR_PALETTE: i32 = 12;
}
