use crate::EngineResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgsCommands {
    /// A  =  command identifier
    /// Sets attributes for fills and sets
    /// border option.
    ///
    /// 1st parameter selects pattern type:
    /// 0=hollow, 1=solid, 2=pattern, 3=hatch 4=user defined
    ///
    /// 2nd parameter selects pattern index number it
    /// ranges 1-24 for type pattern and 1-12 for type hatch.
    /// These patterns are illustrated in the ST BASIC manual,
    /// page 95 in mine.  The IG Drawer will display examples.
    ///
    /// 0-9 for user defined, 8 sets pattern 0 as RANDOM, looks
    /// like dirt or sand mostly, 9 or greater sets pattern 0 as
    /// it's stored default, looks like Star Trek symbol.
    /// Patterns 6 and 7 are also used for user defined LINES.
    /// SEE X 7 command for defining these patterns.
    ///
    /// 3rd parameter specifies if a border is to be drawn
    /// around the filled area.  1=yes, 0=no
    AttributeForFills,

    /// b = command identifer.
    /// Allows special sound effects to be played
    /// using the ST's Sound Chip.
    ///
    /// number Description
    /// --------------------
    /// 0      Alien Invasion
    /// 1      Red Alert
    /// 2      Gunshot
    /// 3      Laser 1
    /// 4      Jackhammer
    /// 5      Teleport
    /// 6      Explosion
    /// 7      Laser 2
    /// 8      Longbell
    /// 9      Surprise
    /// 10     Radio Broadcast
    /// 11     Bounce Ball
    /// 12     Eerie Sound
    /// 13     Harley Motorcycle
    /// 14     Helicopter
    /// 15     Steam Locomotive
    /// 16     Wave
    /// 17     Robot Walk
    /// 18     Passing Plane
    /// 19     Landing
    BellsAndWhistles,

    /// B = command identifier (5 parameters)
    ///
    /// General purpose command for drawing rectangles.<br/>
    /// All attributes effect this command.
    ///
    /// 1st parameter = upper left corner  X coordinate<br/>
    /// 2nd parameter = upper left corner  Y coordinate<br/>
    /// 3rd parameter = lower right corner X coordinate<br/>
    /// 4th parameter = lower right corner Y coordinate<br/>
    /// 5th parameter = rounded corners flag  0=no  1=yes<br/>
    Box,

    /// C  =  command identifier (2 parameters)
    ///
    /// Selects the Pen number to perform the screen<br/>
    /// operation with.
    ///
    /// 1st parameter selects screen operation pen to change.<br/>
    /// 0 = Polymarker color, used for the (P  plot command).<br/>
    /// 1 = line color<br/>
    /// 2 = fill color<br/>
    /// 3 = text color,  used with the ( W command ).<br/>
    /// <br/>
    /// 2nd parameter selects the Pen number 0 thru 15.<br/>
    ColorSet,

    ///  D = command identifier (2 parameters)
    ///
    ///  draws a line from the last polymarker plot,<br/>
    ///  draw LINE or DRAWTO command.  You should use<br/>
    ///  the P or L command to first establish a point<br/>
    ///  for DRAWTO to begin at.<br/>
    ///  See also the entry for T and C commands.
    ///
    ///  1st parameter = X coordinate<br/>
    ///  2nd parameter = Y coordinate<br/>
    LineDrawTo,

    /// E = command identifier ( 3 parameters)
    ///
    /// Sets VDI text effects for text put on the screen<br/>
    /// with the  W  command.<br/>
    /// 1st parameter selects font effect they can be combined<br/>
    ///         0 = normal      1 = thickened (bold)<br/>
    ///         2 = ghosted     4 = skewed<br/>
    ///         8 = undelined   16 = outlined<br/>
    ///
    /// 2nd parameter sets text size in points 1/72 of a inch.<br/>
    /// Values the default system font may be printed in:<br/>
    ///                 8  9  10  16  18  20<br/>
    ///
    /// 3rd parameter sets the text rotation.<br/>
    ///         0 = 0 degrees           1 = 90 degrees<br/>
    ///         2 = 180 degrees         3 = 270 degrees<br/>
    ///         4 = 360 degrees<br/>
    TextEffects,

    ///   F = command identifier<br/>
    ///
    /// Fills a area by replacing the color found<br/>
    /// at specified X Y coordinates till it hits<br/>
    /// another color or edge of screen.<br/>
    /// <br/>
    /// 1st parameter = X coordinate<br/>
    /// 2nd paraneter = Y coordinate<br/>
    FloodFill,

    /// f = command identifer
    /// Fills a area defined by the X and Y points.
    /// The enclosed area will be filled with the current fill
    /// pattern, fill color, border set with the A command.
    ///
    /// 1st parameter = number of paired X,Y points
    /// so the 3 means 6 parameters following.  It also
    /// will be the number of sides the area will have.
    /// passing a 1 or 2 here draws a point or line.
    ///
    /// remaining parameters = X,Y pairs forming the points
    /// the last pair of points will be connected to the
    /// beginning points automatically by the routine.
    PolyFill,

    /// g = command identifier
    ///
    /// Turn graphic scaling on or off.
    /// When on all X Y coordinates are plotted to
    /// an imaginary screen 10,000 by 10,000.
    /// 0,0 is in the upper left corner, while the
    /// lower right corner is 9999,9999 This means
    /// graphics plotted with graphics scaling should
    /// look approximately the same in all resolutions!
    /// In practice though it's only really good for
    /// general positioning.  The new Y coordinate *2
    /// will save you time when converting graphics.  Make
    /// your graphics for medium resolution and then
    /// when you do the mono version  just add a
    /// G#g 2: at the top of your script and a
    /// G#g 0: at the end if you want to cut it back off.
    /// this should fix the medium res graphics to run in
    /// 640x400 mono mode with only the adjustment of
    /// the 'S'etcolors, 'C'olor, and maybe the 'c' command
    /// Keep in mind that none of these 'g' options work
    /// within a "& LOOP".
    ///
    ///
    /// Parameter:
    ///  0 = off
    ///  1 = on
    ///  2 = Y coordinate *2 if in monchrome 640 x 400 mode
    ///      will not change value if in low or medium
    ///      resolution.
    GraphicScaling,

    ///  G#G 0,3,0,0,100,100,100,50:  screen to screen
    /// --------------                  G#G 1,3,0,0,100,100:         screen to memory
    /// G#G 2,3,200,50:              memory to screen
    /// G#G 3,3,50,50,75,75,150,100: piece of memory to
    ///                              screen
    ///
    /// G = command identifier
    /// Screen grab,  "Bit-Blit".
    /// Grabs a rectangular portion of the screen
    /// copies it to another portion of the screen
    /// or to memory, or copies memory to screen,
    /// depending on 1st parameter.  The whole screen
    /// can be blitted to memory and back!
    ///
    /// 1st parameter sets type of blit to do:
    /// 0 = screen to screen
    /// 1 = screen to memory
    /// 2 = memory to screen
    /// 3 = piece of memory to screen
    ///
    ///
    /// 2nd parameter sets writing mode for the blit operation.
    /// mode  logic...............Description
    /// 0    dest=0..............Clear destination block
    /// 1    dest=S AND D
    /// 2    dest=S AND (NOT D)
    /// 3    dest=S.............Replace mode
    /// 4    dest=(NOT S) AND D...Erase mode
    /// 5    dest=D...............Destination unchanged
    /// 6    dest=S XOR D.........XOR mode
    /// 7    dest=S OR D..........Transparent mode
    /// 8    dest=NOT (S OR D)
    /// 9    dest=NOT (S XOR D)
    /// 10   dest=NOT D
    /// 11   dest=S OR(NOT D)
    /// 12   dest=NOT S
    /// 13   dest=(NOT S) OR D....Reverse Transparent mode
    /// 14   dest=NOT (S AND D)
    /// 15   dest=1...............Fill destination block
    ///
    /// The rest of the parameters depend on the
    /// 1st parameters setting:
    ///
    /// IF 1st PARAMETER = 0   "screen to screen"
    /// 3rd = X source, upper left corner
    /// 4th = Y source, upper left corner
    /// 5th = X source, lower right corner
    /// 6th = Y source, lower right corner
    /// 7th = X destination, upper left corner
    /// 8th = Y destination, upper left corner
    ///
    /// IF 1st PARAMETER = 1  "screen to memory"
    /// 3rd = X source, upper left corner
    /// 4th = Y source, upper left corner
    /// 5th = X source, lower right corner
    /// 6th = Y source, lower right corner
    ///
    /// IF 1st PARAMETER = 2  "memory to screen"
    /// 3rd = X destination, upper left corner
    /// 4th = Y destination, upper left corner
    ///
    /// IF 1st PARAMETER = 3   "piece of memory to screen"
    /// 3rd = X source, upper left corner
    /// 4th = Y source, upper left corner
    /// 5th = X source, lower right corner
    /// 6th = Y source, lower right corner
    /// 7th = X destination, upper left corner
    /// 8th = Y destination, upper left corner
    GrabScreen,

    /// q Quick Pause with Vsync or set Internal Double Stepping
    /// NOTE: Vsync() waits until the screen's next vertical retrace occurs.
    ///
    /// Parameter = number of Vsync()s to pause, 60ths of a second max 180.  No flow control with this delay.
    /// IF Parameter = 9995 double step the G command internally use 3 Vsync()s
    ///                 
    /// 9996 double step the G command internally use 2 Vsync()s
    /// 9997 double step the G command internally use 1 Vsync()s
    /// 9998 double step the G command internally use 0 Vsync()s
    /// 9999 Turn double step OFF
    QuickPause,

    /// H = command identifier (1 parameter)
    ///
    /// When on non solids are drawn, a circle will be
    /// drawn instead of a disk.
    ///
    /// Parameter  1=on  0=off
    HollowSet,

    /// I = command identifier
    ///
    /// Initializes color pallet and most<br/>
    /// attributes to what ever they were before<br/>
    /// the Instant Graphics ACC was executed.<br/>
    /// Issue this command at the start of each  graphic<br/>
    /// sequence and you'll have a common starting point.<br/>
    ///<br/>
    /// Parameter:<br/>
    ///     0 = Set resolution, pallet, and attributes<br/>
    ///     1 = Set resolution and pallet<br/>
    ///     2 = Set attributes<br/>
    ///     3 = Set Instant Graphics! default pallet<br/>
    Initialize,

    ///  J = command identifier
    /// Draws a elliptical ARC, which is part of a oval.
    ///
    /// 1st parameter = X coordinate for the oval center
    /// 2nd parameter = Y coordinate for the oval center
    /// 3rd parameter = X radius of the oval
    /// 4th parameter = Y radius of the oval
    /// 5th parameter = begining angle to start drawing at
    /// 6th parameter = ending angle to to stop drawing at
    EllipticalArc,

    /// k = command identifier
    /// Turns text cursor on or off and
    /// sets BACKSPACE as destructive or nondestructive.
    ///
    /// Parameter:
    ///     0 = cursor off
    ///     1 = cursor on
    ///     2 = destructive backspace
    ///     3 = nondestructive backspace
    Cursor,

    /// K = command identifier
    /// Draws a ARC, which is part of a circle.
    ///
    /// 1st parameter = X coordinate for the circle center
    /// 2nd parameter = Y coordinate for the circle center
    /// 3rd parameter = radius of the circle
    /// 4th parameter = begining angle to start drawing at
    /// 5th parameter = ending angle to to stop drawing at
    Arc,

    /// L = command identifier
    /// Draws a line between specified points.
    /// See also the entry for T and C commands.
    ///
    /// 1st parameter = begining X coordinate
    /// 2nd parameter = begining Y coordinate
    /// 3rd parameter = ending   X coordiante
    /// 4th parameter = ending   Y coordinate
    DrawLine,

    /// z = command identifer
    /// Draws a connected line defined by the X and Y points.
    /// Line color, type and line end points apply.
    ///
    /// 1st parameter = number of paired X,Y points
    /// so the 3 means 6 parameters following.
    /// A minimum of 2 here required!  I forced this
    /// because 1 here would crash the system! The maximum
    /// number of points is 128.
    ///
    /// remaining parameters = X,Y pairs forming the points
    /// of the Line.
    PolyLine,

    /// M = command identifier
    ///
    /// Parameter sets drawing mode.
    /// 1 = replace     2 = transparent
    /// 3 = XOR         4 = reverse transparent
    DrawingMode,

    /// n = command identifer
    /// Treats IG's sound effects as musical notes.
    /// There is no flow control with this command.
    ///
    /// Note:
    /// Long tunes should use this command within a
    /// "& LOOP" or use the 'N' command. The values
    /// listed below apply to the 'N' command except
    /// 'N' takes it's parameters as a ASCII string of
    /// characters one BYTE per parameter a 255 value
    /// is the max that can be passed to 'N'.
    /// Values are passed as characters with the proper
    /// ASCII value to 'N'. Sound effects for this command
    /// may be altered with "b 20".
    /// PARAMETERS for 'n' command
    /// . . . . . . . . . . . . . .
    ///
    /// 1st parameter is the effect number to use
    /// from 0-19.  These are the same effects as the
    /// 'b' command uses exception that n doesn't loop
    /// the first 5 effects internally.
    ///
    /// 2nd parameter is the voice to use 0-2.
    ///
    /// 3rd parameter is the Volume 0-15.
    ///
    /// 4th parameter is pitch 0-255.  A 0 will play no
    /// effect but allow TIMIMG and STOP type to be executed.
    ///
    /// 5th parameter is timing 0-9999.  This stops IG in it's
    /// tracks for a specified time in 200ths of a second,
    /// unless the user taps a key.
    ///
    /// 6th parameter is stop note type 0-4.
    ///     0=no effect, sound remains on.
    ///     1=move selected voice into release phase.
    ///     2=Stop select voice immediately.
    ///     3=move all voices into release phase.
    ///     4=Stop all voices immediately. ( same as "b 21" )
    ChipMusic,

    /// N = command identifier
    /// The N command is for handling sound.
    /// MIDI, and sound chip.
    ///
    /// 1st parameter = N operation to perform.
    /// If 1st parameter = 0, 1, 3 or 4  a 2nd parameter
    /// is required, which is number of MIDI data bytes
    /// to read into the MIDI buffer, MAX of 9999.
    ///
    /// Load only midi:      N 0,9998,datadelaydatadelaydatadelay....
    /// sound chip       N 3,9996,DataDataDataDataDelayData....
    ///
    /// Load and execute:    N 1,9998,datadelaydatadelaydatadelay....
    /// sound chip       N 4,9996,DataDataDataDataDelayData....
    ///
    /// The 0 means load the MIDI buffer only,
    /// then the number of bytes to load followed
    /// by a comma, then the MIDI data in the form
    /// data byte, delay byte, back to back with the
    /// data byte always first.  The delay is in 200ths
    /// of a second so a delay of about 1.25 seconds
    /// between each data byte is the max. After
    /// the MIDI buffer is loaded with a N 0 or 1 command
    /// a N 2: (midi) or N 5: (sound chip) issued later will
    /// replay the buffer without reloading, like
    /// G#N 2:  If a user CONTROL C's or CONTROL X's
    /// the MIDI data while it is being loaded the MIDI
    /// buffer will be set to 0 and N 2: will play
    /// nothing.  However it a user aborts while MIDI is
    /// being played the MIDI buffer will remain intact.
    /// If a user has the MIDI option off (F4 function key
    /// on the ACC, control+shift+m for the EMU) MIDI data
    /// will be still loaded but not executed. If line noise
    /// creeps into the MIDI buffer when it's loaded it will
    /// garble the sound, the user might be able to recover
    /// by pressing the + key to try to get the MIDI flow
    /// out to the ports in proper sync datadelaydatadelay,
    /// line noise can get it in reverse order.
    ///
    ///  NEW for IG218...
    /// Once the buffer is LOADED you can:
    ///
    /// EXE Buffer N>6,x=From(0-1664), y=To(1-1665)chip notes
    /// example N>6,300,701:
    ///
    /// Each note takes six bytes so 1666 notes possible in
    /// the 10001 byte buffer note 0 to 1665.  I wrote a GFA
    /// Basic program that takes the IG Draw program's
    /// Tap A Tune notes and auto generates the N chip note
    /// byte format it's N_UTILB.PRG  Keep in mind this buffer
    /// doesn't play in the background no multi tasking and
    /// you use it for one thing at a time MIDI or Chip Notes
    /// or with the New X>9 command that stores and Executes
    /// IG commands in it.
    ///
    /// DATA FORMAT FOR SOUND CHIP
    /// The data to the sound chip  routine is in
    /// ASCII values ie " a capital A represents 65 ".
    /// the format is:
    ///
    /// Effect_numberVoiceVolumePitchTimingStop_effect
    /// 0-19 0-2 0-15 0-255 0-255 0-4
    ///
    /// When a 0 is passed as a pitch value timing and
    /// stop effects can be issued without executing
    /// a note.  For more details see the "n" command.
    ///
    /// *** Download MS2IG.ARC for a Music Studio to IG MIDI file converter.
    /// *** See "n" command also and look for N_UTILB.PRG in arc.
    Noise,

    /// O = command identifier
    /// Draws a disc or circle depending if the
    /// H command is active or not.
    ///
    /// 1st parameter = X coordinate of circle center
    /// 2nd parameter = Y coordinate of circle center
    /// 3rd parameter = radius of circle
    Circle,

    /// P = command identifier
    /// Plot a point or polymarker shape on the screen.
    /// See also the entry for the  T and C  commands.
    ///
    /// 1st parameter = X coordinate
    /// 2nd parameter = Y coordinate
    PolymarkerPlot,

    /// Q = command identifier
    /// Draws an ellipse, which is a OVAL.
    /// See H and A commands also.
    /// 1st parameter = X coordinate of oval center
    /// 2nd parameter = Y coordinate of oval center
    /// 3rd parameter = X radius of oval
    /// 4th parameter = Y radius of oval
    Ellipse,

    /// R = command identifier
    /// Allows to switch between low and medium resolution.
    /// Low resolution allows the use of 16 VDI colors!
    /// Medium resolution only allows 4 colors.
    /// If the resolution selected is the one the system is
    /// currently in, IG ignores it. This is so you
    /// can set the set the color palette and not do a
    /// resolution switch.  Resolution switching might cause
    /// havoc for some commercial terminals you may be using
    /// IG with, although it shouldn't  with FLASH 1.60 and
    /// Interlink.  With Interlink IG.EMU the CLR HOME key will
    /// restore Medium resolution, with the ACC it's automatic.
    ///
    ///
    /// 1st Parameter selects resolution to switch to:
    /// 0 = low resolution      1 = medium resolution
    ///
    /// 2nd Parameter is the system palette flag:
    /// 0 = no change           1 = default system colors.
    ///                         2 = IG default color palette
    SetResolution,

    /// s = command identifier
    /// Clears whole screen or portions of it.
    ///
    /// Parameter:
    ///     0 = Clear screen home cursor.
    ///     1 = Clear from home to cursor.
    ///     2 = Clear from cursor to bottom of screen.
    ///     3 = Clear WHOLE screen with VDI.
    ///     4 = Clear WHOLE screen with VDI and VT52
    ///         cursor will be set to home.
    ///     5 = Clear,Home,ReverseOff,Text Background to reg 0,
    ///         Text Color to register 3.  All done with VT52,
    ///         a VT52 quick reset of sorts.
    ScreenClear,

    /// S = command identifier
    /// 1st parameter selects pen color to change 0 thru 15.
    /// 2nd parameter selects red   color level 0 thru 7.
    /// 3rd parameter selects green color level 0 thru 7.
    /// 4th parameter selects blue  color level 0 thru 7.
    SetPenColor,

    /// t = command identifier
    /// Tells IG to send ^S, times it for X seconds
    /// and then tells IG to send ^Q.  Any key
    /// will abort the pause prematurly.
    /// MAX time is 30 seconds if more of a pause is needed
    /// chain a few together.  G#t>30:t 5:
    /// I'm hoping this will eliminate the BBS from timing
    /// out and logging a user off.
    ///
    /// Parameter = number of seconds to pause, 30 max.
    TimeAPause,

    /// T = command identifier
    ///
    /// 1st parameter selects lines or polymarkers to change.
    /// 1 = polymarkers  ( effects output of the P command )
    /// 2 = lines        ( effects D and L commands )
    ///
    /// 2nd parameter picks type of line or polymarker
    /// depending value of 1st parameter.
    ///
    /// for polymarkers:
    ///     1 = point            2 = plus sign
    ///     3 = star             4 = square
    ///     5 = diagonal cross   6 = diamond
    ///
    /// for lines:
    ///     1 = solid            2 = long dash
    ///     3 = dotted line      4 = dash-dot
    ///     5 = dashed line      6 = dash-dot-dot
    ///     7 = user defined ( see X 7 command )
    ///
    /// 3rd parameter selects size and line end styles,
    ///     and which user defined line
    ///
    /// -size-
    /// for polymarkers: 1 thru 8
    /// for solid only lines: 1 thru 41
    ///
    /// for user defined lines:
    /// pattern numbers 1 thru 32
    ///
    ///
    /// -line end styles-
    /// 0  = both ends square
    /// 50 = arrows on both ends
    /// 51 = arrow on left,  squared on right
    /// 52 = arrow on right, squared on left
    /// 53 = arrow on left,  rounded on right
    /// 54 = arrow on right, rounded on left
    /// 60 = rounded on both ends
    /// 61 = rounded on left,  squared on right
    /// 62 = rounded on right, squared on left
    /// 63 = rounded on left,  arrow on right
    /// 64 = rounded on right, arrow on left
    LineMarkerTypes,

    /// U = command identifier
    /// Draws a rounded rectangle.
    ///
    /// 1st parameter = upper left corner  X coordinate
    /// 2nd parameter = upper left corner  Y coordinate
    /// 3rd parameter = lower right corner X coordinate
    /// 4th parameter = lower right corner Y coordinate
    /// 5th parameter = 0 selects filled rounded rectangle
    ///                   with no borders.
    ///                 1 selects rounded rectangle
    ///                   affected by all attributes and
    ///                   H command as well as line patterns
    ///                   set with the T command.
    RoundedRectangles,

    /// V = command identifier
    /// Draws a pieslice, which is part of a circle.
    ///
    /// 1st parameter = X coordinate for the circle center
    /// 2nd parameter = Y coordinate for the circle center
    /// 3rd parameter = radius of the circle
    /// 4th parameter = begining angle to start drawing at
    /// 5th parameter = ending angle to to stop drawing at
    Pieslice,

    /// W = command identifier
    /// Writes text on screen at any X Y coordinate.
    /// Carriage Return and Linefeed are ignored (IG214) so
    /// you can split the text to be written across two lines,
    /// the maximum length is 128 characters.
    /// The @ symbol ends the text to be written.
    /// See also the E and C commands.
    ///
    /// 1st parameter = X coordinate
    /// 2nd parameter = Y coordinate
    /// 3rd parameter = text ended with @
    ///
    /// Chain example:
    /// G#W>20,50,Chain@L 0,0,300,190
    WriteText,

    /// Y = command identifier
    /// Draws a elliptical pieslice. which is part of a OVAL.
    ///
    /// 1st parameter = X coordinate for the oval center
    /// 2nd parameter = Y coordinate for the oval center
    /// 3rd parameter = X radius of the oval
    /// 4th parameter = Y radius of the oval
    /// 5th parameter = begining angle to start drawing at
    /// 6th parameter = ending angle to to stop drawing at
    EllipticalPieslice,

    /// Z = command identifer
    /// Fills a area.  The A commands
    /// border set has no effect on this
    /// fill.
    ///
    /// 1st parameter = upper left corner  X coordinate
    /// 2nd parameter = upper left corner  Y coordinate
    /// 3rd parameter = lower right corner X coordinate
    /// 4th parameter = lower right corner Y coordinate
    FilledRectangle,

    /// Gets input from user's keyboard and transmits it as soon
    /// as the chain " > " from the last G#  is broke.  Should
    /// be used near the end of a MENU, as the BBS will continue
    /// to send to the terminal while the INPUT command is
    /// waiting on the user.  This is so the BBS will be waiting
    /// for INPUT when IG sends the user's response at '>' exit.
    /// The INPUT command is good for letting you use
    /// any 4 colors you want for a BBS MENU and then to issue
    /// some reset commands. ie G#<>1,0,1:I>0:k>1:s>0:   Also
    /// optionally INVOKES MOUSE routine for the X 4  command
    /// so ZONES can be pointed to and clicked on, you must
    /// use X 4 to define and load the zone strings first and
    /// you should use IG to draw some borders around the
    /// zones so the user will know where and what he is
    /// selecting when he clicks on a ZONE, that way you
    /// have the job of cosmetics, that's half the fun anyway.
    /// The selected Zone's associated data string is
    /// transmitted to the BBS as soon as IG exits
    /// the chain from the last G#  .  ZONE 47 is the default
    /// ZONE, it's associated data string will be sent if no
    /// ZONES match where the user clicked.  You should always
    /// define ZONE 47. ( Check out X 4,9998: LOOPBACK also!)
    /// You may find the X 3 and the X 6 command useful too.
    ///
    /// 1st parameter = Transmitt carriage return at the end of the string INPUTted?  1 = YES  0 = NO
    /// 2nd parameter = INPUT type
    /// 0 = One key,  (hot key input for FoReM)
    /// 1 = String, with a return to
    /// end input from user,
    /// max string length = 128
    ///
    /// 2 = MOUSE ZONE, activate a POLYMARKER
    /// mouse pointer, use the " T " command
    /// to select mouse type and size and the
    /// " C " command to set mouse color.
    /// 3 to 10 = MOUSE ZONE activates a GEM mouse pointer
    /// 3=Arrow 4=Hour Glass 5=Bumble Bee
    /// 6=Pointing Finger 7=Flat Hand
    /// 8=Thin Cross Hair 9=Thick Cross Hair
    /// 10=Outlined Cross Hair   
    /// User moves mouse and clicks on a
    /// "ZONE". Selection is processed
    /// when button is released.
    /// The associated ZONE string
    /// is copied into INPUT's
    /// string to be transmitted to BBS
    /// at the end of the IG script chain.
    ///
    ///
    /// 3rd parameter = Output options
    /// 0 = Don't show input typed from user
    ///     on his screen. Has no effect on
    ///     Mouse ZONES.
    /// 1 = Show input typed from user on screen.
    ///     Has no effect on Mouse ZONES.
    /// 2 = Show input but throw it away, don't
    ///     transmit it at the end of the chain.
    ///     Does effect Mouse ZONE.
    /// 3 = Don't show input from user, and throw it
    ///     away too.  Does effect Mouse ZONE.
    ///
    /// Note:  If 2 ZONES areas are over lapping on the screen the
    /// ZONE with the lower value ID number will get selected when
    /// the mouse is clicked on both ZONEs at the same time.
    ///
    /// +----+--------------------+
    /// user clicks in here ---> |ID=1|                    |
    /// ZONE 1 gets selected     +----+   ID=10            |
    /// |                         |
    /// +-------------------------+
    ///
    /// +----+--------------------+
    /// user clicks in here ---> |ID=7|                    |
    /// ZONE 2 gets selected     +----+   ID=2             |
    /// |                         |
    /// +-------------------------+
    InputCommand,

    /// ? = command identifier
    /// Asks the Instant Graphics terminal
    /// questions.  Transmit it to the
    /// BBS  ( Host system ).
    /// 1st parameter selects the question to ask.
    ///         0 = Version number, IG will transmit in
    ///             ASCII to the host system the version
    ///             number it is.
    ///         1 = Ask IG where the cursor is and the
    ///             mouse button state.  When this question
    ///             is asked a 2nd parameter is passed also,
    ///             like G#? 1,0  the zero means just check
    ///             the cursor and mouse buttons and send it to
    ///             host system immediatly.  If the second
    ///             parameter is a 1 then the user can move the
    ///             cursor with the mouse until a button is
    ///             pressed then the cursor location and button
    ///             state is transmitted.  In other words a
    ///             point and click cursor!!!  The cursor and
    ///             and mouse button state is sent in three
    ///             characters, subtract 32 from the ASCII
    ///             value of these characters to arrive at
    ///             COLUMN number 0-79    ROW 0-24   BUTTON 0-3
    ///             With this command the cursor should be
    ///             enabled with the G#k 1 command.
    ///         2 = Ask IG where the mouse is and button state.
    ///             A second parameter is required when this
    ///             question is asked, like G#? 2,0 the zero
    ///             indicates that IG is to send the BBS the
    ///             mouse coordinates immediatly.
    ///             A 1 or 2 activates a polymarker for a mouse
    ///             pointer that you select with IG's "T" command
    ///             A 3 to 10 activates the GEM mouse pointer
    ///                 3=Arrow 4=Hour Glass 5=Bumble Bee
    ///                 6=Pointing Finger 7=Flat Hand
    ///                 8=Thin Cross Hair 9=Thick Cross Hair
    ///                 10=Outlined Cross Hair   
    ///             The user can move the pointer around till
    ///                he clicks a button then the host system is
    ///             sent the X,Y,Button in a ASCII string just
    ///             like this  420,150,1:   It's up to the
    ///             host system to convert the ASCII string
    ///             into actual numbers.  The "g" command has
    ///             no effect on this command in version 2.12+
    ///     3 = Asks IG what resolution the terminal is in
    ///                 0:    low resolution     320x200
    ///                 1:    medium resolution  640x200
    ///                 2:    high resolution    640x400    
    AskIG,

    /// XOR stepping example:
    /// G#G 1,3,0,0,50,50:
    /// G#&>198,0,2,0,G|4,2,6,x,x:
    ///
    ///
    /// & = command identifier
    /// Loops a operation specified number of times with
    /// stepping, special options for XOR ing and the
    /// 'W'rite text command. The CHAIN character > only
    /// works directly after the &>   You can loop a
    /// chain of commands, see parameter 5,  but you can't
    /// loop a loop.  Still this command is very powerful
    /// and worth the effort to learn.
    ///
    /// 1st parameter = FROM value
    ///     if from value bigger than TO value
    ///     loop will detect and step backwards.
    ///
    /// 2nd parameter = TO value
    /// 3rd parameter = step value, positive number only.
    /// 4th parameter = DELAY in 200 hundredths of a between
    ///         each step of the loop.
    /// 5th parameter = command Identifier to loop.
    ///         optional specification character after 5th
    ///         parameter instead of comma:
    ///            | = XOR stepping
    ///            @ = get text for W command everytime
    ///                otherwise text written from loop
    ///                with the W command is last text
    ///                written with W command before the
    ///                loop was executed.  W command now
    ///                ignores CR and LF so loop command
    ///                can be used for easy Written text
    ///                placement with the  loop's stepping.
    /// NOTE: (Chain Gang) If a > symbol is given here as a command
    /// identifer chain gang option is invoked.
    /// This allows multiple commands to LOOPed.
    /// Instead of one command specified for this
    /// parameter a string of command identifers
    /// are passed.
    /// The > to get IG's attention to chain gang
    /// and ending with the @ FOLLOWED by a comma.
    /// Like this >CL@,
    ///   C is at command position 0
    ///   L is at command position 1
    /// The position of the command is the key
    /// to which command will be executed.
    /// There can be up to 128 (0-127) commands in
    /// this command string in any order you like.
    ///
    /// Example: switching line color and drawing lines too
    /// G#I>0:k>0:s>4:S>3,0,0,6:
    /// G#&>0,636,4,0,>CL@,16,0)1,3:1)319,99,x,0:0)1,2:1)319,99,+2,0:
    /// You can replace the  )  above with the commands  themself and  it will  work
    /// G#&>0,636,4,0,>CL@,16,0C1,1:1L319,99,x,199:0C1,2:1L319,99,+2,199:
    /// G#t>6:I>0:s>4:b>7:k>1:
    ///
    ///
    /// 6th parameter = number of parameters command that
    /// to be looped requires.  You should at
    /// least specify the number the command requires
    /// ie L command requires 4 , ie W command 2.
    /// You can specify multiples of the required number
    /// such as 8 or 12 for the L command Max up to
    /// 2048. It's just a total of all the parameters
    /// required that follows it.
    /// This will work like BASIC's READ DATA
    /// statements between each loop step.  Also note
    /// a _ underscore may be used to split parameters
    /// across lines if it is used in place of the first
    /// digit of value, this will make huge detailed
    /// files smaller (DEGAS conversions).
    ///
    /// REMAINING parameters = whatever the command being looped
    /// requires.  If you use a "x" as a parameter
    /// it will be stepped in the direction of the
    /// FROM TO values, if you use a "y" the loop
    /// will step the value in a reverse direction.
    /// You can use both "x" and "y" at the same time.
    /// If you use a number it will remain as a constant
    /// for the command being looped through out the
    /// loop execution.  Adding a + before constant
    /// will add the "x step value" to the constant.
    /// Adding a - before the constant will subtract
    /// the constant value from the current "x step"
    /// value.  Adding a ! before the constant will
    /// subtract the "x step" value from the constant.
    /// Like so :
    /// G#&>10,30,2,0,L,4,100,+10,-10,+600,!99:
    ///
    ///
    ///
    /// loop Written text option example:
    ///
    /// G#E>0,18,0:C>3,2:s>0:
    /// G#&>20,140,20,0,W@2,0,x,A. Item 1@
    /// B. Item 2@
    /// C. Item 3@
    /// D. Item 4@
    /// E. Item 5@
    /// F. Item 6@
    /// G. Item 7@
    /// G#W>200,140,Power Graphics with IG!!!@
    /// G#& 140,20,20,0,W,2,200,x:
    /// G#W>10,180,That's so DEVO!!!@
    ///
    /// Example of loop used to READ DATA and step within at the same time in
    /// both directions, once you get used to the "& loop" you will use it
    /// a lot!!
    ///
    /// G#I>0:s>0:k>0:L>300,10,340,10:S>2,7,4,5:S>1,0,0,0:
    /// G#&>85,300,5,0,D,24,340,10:340,60:420,60:420,85:_
    /// 340,85:340,180:85,180:x,85:220,85:220,60:x,60:x,10:
    /// G#L>300,180,300,85:A>1,1,1:C>2,2:F>320,20:E>0,10,0:M>2:
    /// G#C>3,1:W>210,141,Because of@
    /// G#W>210,156,God's Love@
    /// G#W>210,171,we are.@M>1:
    /// G#t>3:G>1,3,80,8,421,181:s>0:
    /// G#&>0,220,4,0,G,16,2,3,x,x:2,3,y,y:2,3,x,y:2,3,y,x:t>2:s>0:
    /// G#&>0,638,4,0,G,8,2,3,x,9:2,3,y,9:
    /// G#t>3:s>0:k>1:G>2,3,80,8:p 0,20:
    LoopCommand,

    /// c = command identifier
    /// Sets text and background color
    ///
    /// 1st parameter selects text or background
    ///  0 = background         1 = text
    ///
    /// 2nd parameter = color register 0 thru 15
    ///
    ///  Note: color registers can be changed with the
    ///  S command but the ST's VDI pen numbers do not
    ///  corespond with color register numbers here a
    ///  reference chart:
    ///
    /// register        pen             register        pen
    ///    0             0                 8             9
    ///    1             2                 9             10
    ///    2             3                 10            11
    ///    3             6                 11            14
    ///    4             4                 12            12
    ///    5             7                 13            15
    ///    6             5                 14            13
    ///    7             8                 15            1
    VTColor,

    /// d = command identifier
    /// Deletes specified number of text lines, the bottom
    /// line on the screen is scrolled upward.
    ///
    /// Parameter =  number of lines to delete.
    VTDeleteLine,

    /// i = command identifier
    /// Inserts lines at cursor position or top of screen.
    ///
    /// 1st parameter selects type of insert.
    ///         0 = move cursor up a line until it hits
    ///             the top of the screen, then insert
    ///             blank lines.
    ///         1 = Insert line at cursor, bottom line is
    ///             scrolled off.
    ///
    /// 2nd parameter = number of times to perform
    ///                 this operation.
    ///
    VTLineInsert,

    /// l = command identifier
    /// Clears text lines.
    ///
    /// Parameter:
    ///     0 = Clear whole line and carriage return.
    ///     1 = Clear line from begining to cursor inclusive.
    ///     2 = Clear line at cursor to end of line.
    VTLineClear,

    /// m = command identifier
    /// Homes or moves cursor a line at a time
    /// or a column at a time, from current position.
    ///
    /// 1st parameter selects direction.
    ///         0 = Home cursor.
    ///         1 = up
    ///         2 = down
    ///         3 = right
    ///         4 = left
    ///
    /// 2nd parameter sets number of times to do this
    ///     operation.
    ///         
    VTCursorMotion,

    /// p = command identifier
    /// Positions cursor at  column, line.
    /// Like X Y only with characters.
    ///
    /// 1st parameter = column   0 thru 79
    ///
    /// 2nd parameter = line     0 thru 24
    VTPosition,

    /// r = command identifier
    /// Remembers or recalls cursor position.
    ///
    /// Parameter:
    ///         0 = remember cursor position
    ///         1 = recall cursor position, and put it there
    VTRemember,

    /// v = command identifier
    /// Turn inverse video on or off.
    /// Parameter:
    ///     0 = off
    ///     1 = on
    VTInverseVideo,

    /// w = command identifier
    /// Turns line wrap on or off.
    ///
    /// Parameter:
    ///     0 = off
    ///     1 = on
    VTLineWrap,

    /// Extended commands are invoked by a captial
    /// X and a number ranging from 0 to 9999, this is
    /// sort of like the old 8 bit XIO thing.
    /// Opens a door for 10,000 new commands!!!
    ExtendedCommands,
}

impl IgsCommands {
    /// Read all commands except loop.
    pub fn from_char(ch: char) -> EngineResult<Self> {
        let result = match ch {
            'A' => IgsCommands::AttributeForFills,
            'b' => IgsCommands::BellsAndWhistles,
            'B' => IgsCommands::Box,
            'C' => IgsCommands::ColorSet,
            'D' => IgsCommands::LineDrawTo,
            'E' => IgsCommands::TextEffects,
            'F' => IgsCommands::FloodFill,
            'f' => IgsCommands::PolyFill,
            'g' => IgsCommands::GraphicScaling,
            'G' => IgsCommands::GrabScreen,
            'q' => IgsCommands::QuickPause,
            'H' => IgsCommands::HollowSet,
            'I' => IgsCommands::Initialize,
            'J' => IgsCommands::EllipticalArc,
            'k' => IgsCommands::Cursor,
            'K' => IgsCommands::Arc,
            'L' => IgsCommands::DrawLine,
            'z' => IgsCommands::PolyLine,
            'M' => IgsCommands::DrawingMode,
            'n' => IgsCommands::ChipMusic,
            'N' => IgsCommands::Noise,
            'O' => IgsCommands::Circle,
            'P' => IgsCommands::PolymarkerPlot,
            'Q' => IgsCommands::Ellipse,
            'R' => IgsCommands::SetResolution,
            's' => IgsCommands::ScreenClear,
            'S' => IgsCommands::SetPenColor,
            't' => IgsCommands::TimeAPause,
            'T' => IgsCommands::LineMarkerTypes,
            'U' => IgsCommands::RoundedRectangles,
            'V' => IgsCommands::Pieslice,
            'W' => IgsCommands::WriteText,
            'Y' => IgsCommands::EllipticalPieslice,
            'Z' => IgsCommands::FilledRectangle,
            '<' => IgsCommands::InputCommand,
            '?' => IgsCommands::AskIG,
            // Modified VT-52 Commands
            'c' => IgsCommands::VTColor,
            'd' => IgsCommands::VTDeleteLine,
            'i' => IgsCommands::VTLineInsert,
            'l' => IgsCommands::VTLineClear,
            'm' => IgsCommands::VTCursorMotion,
            'p' => IgsCommands::VTPosition,
            'r' => IgsCommands::VTRemember,
            'v' => IgsCommands::VTInverseVideo,
            'w' => IgsCommands::VTLineWrap,
            'X' => IgsCommands::ExtendedCommands,
            _ => {
                return Err(anyhow::anyhow!("Unknown IGS command: {ch}"));
            }
        };
        Ok(result)
    }
}
