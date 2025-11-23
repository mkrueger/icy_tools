6-20-2024
BEST VIEWED with ME.TTP  <ctrl-X> <ctrl-C> EXIT without saving!
		<ctrl-S> Search	
		<ctrl-V> PAGE Forward <esc>V PAGE Backward
		<esc>< MOVE to Beginning <esc>> MOVE to End of Buffer


           Instant Graphics! Online Graphics Script Interpreter
                   Copyright 1988-2024 by Larry Mears
	           Version 2.20 by Larry Mears, Freeware

 
    IGDEV13 with IG220 engine, IG220.ACC, IG220L.ACC and IG220.EMU
	   		for Interlink by Larry Mears, Freeware.
			a few routines in the IGDEV13.PRG and the IGS220.EMU
			were written by Kevin Moody and Anthony Rau,
			used with permission of Kevin Moody.
				

	Note: You may find a list of Atari BBSs here:
	www.sfhqbbs.org/ataribbslist.php
	or maybe here in the future: ataribbs.com

*** IMPORTANT USE IGDEV13.PRG WITH HSMODA07.ZIP set up a 32K serial buffer the
	ST's default serial buffer is only 256 bytes I believe, not near enough!
	IG220.ACC needs it too. Set Interlink's serial port buffer to 31K.
	This allows IG to do it's graphics on the screen and not lose commands
	coming over the serial port at HIGH SPEEDs. :) 19.2K baud!
	Back in 1989  maybe a 4K or 8K serial buffer worked but we only had
	2400 baud modems, 2.4K baud!  The screen is not as fast as the serial
	port.  Without a big serial buffer even ASCII text will drop out a
	little bit. 
 
	Here's a couple of links to get HSMODA07.LZH / .ZIP
		www.scheutzow.de/museum/index.html
	https://github-wiki-see.page/m/z80andrew//SerialDisk/wiki/User-guide

	Also besides HSMODA07 that goes in the AUTO folder you may want to 
	put KX.PRG or KRX.PRGs I included cuts off key click sound and or key
	repeats. These can mess up the TAP-A-TUNE in the IGDEV program.


IG220.ACC needs it too unless it is ran along with Interlink
You set it up in the AUTO folder,
also in the AUTO folder you may want to put (included) KRX.PRG or KX.PRG these
cut off the KEY click sound and KEY repeat when a key is held down these can
effect the IGDEV program's TAP A TUNE function.  RX.PRG just cuts off repeat.

New for IG220
-------------
  This version adds 2 new X commands. See X 11 and X 12 for the details.
X 11 Loads the BitBlit Memory or Wipes with a BYTE value or random numbers.
It can Load ALL 32000 bytes or a 4000 byte section from the serial port or
disk.  This is so you can Load a Bit Map for the screen it takes about 30
seconds for the whole screen or 4 seconds for a 4000 byte horizontal section
at 19.2K baud the ST's max serial port speed.  The X 12 command loads a color
register palette, it loads 4 colors at a time.  With these 2 commands and
IGDEV13 and the IG GRAB commands Screen to Memory / Memory to Screen you can
save out a IG script constructed screen as a stagnate Bit Map  That may load
faster than the script if it is a complex drawing. Also the IGDEV13 program can
load a DEGAS/Elite uncompressed image into the BitBlit Memory .PI1 .PI2 and
.PI3 only. You can save the whole screen or horizontal sections and it's
palette also. See below the X 11 and X 12 commands for more details.
I also added Clip Set to the I 5 command, before it just set resolution.
Any time I refer to the DRAWER it is inside the IGDEV13.PRG by hitting
the F1 key. I also created a IGX1220.TTP it will display multiple IG files just
put a SPACE in between each filename. If the IG files are not in the same
FOLDER as the IGX1220.TTP you will have to give it the PATH  The IGX1220.TTP
just displays the files if you want a pause in between each ADD a G#t 9: at the 
end of the scripts if they don't already have one. 
In the DRAWER I also added support for the color rotation command.


Included Files:
--------------

STSELECT.PRG Put in your AUTO folder 1st lets you select witch AUTO folder
			 and DESK ACCessory programs you want to run. written by
			 J. Weaver Jr  a must have utility.
			 Thank you Mr. Weaver

IGDEV13.PRG The IG220 draw program hit F1 to draw, comes up in dumb terminal
			but smart graphics mode.

DRAWTIPS.TXT Limited Help file on using IG Drawing program.

IG220.ACC	The IG220.ACC desk Accessory program use Interlink
			or other termial program you like that sets up a big
			serial buffered port.

IG220L.ACC  The IG220L.ACC LIGHT version desk Accessory progam use
		Interlink or other termial program you like that sets up a big
		serial buffered port.  IG220L.ACC has no file support
		and uses less memory.

IGS220.EMU	This loads into the INTERLINK terminal Hit F5 or select
			Style/Features EMULATION ?????? BUTTON to Load.

IG220.TXT	List of the IG commands and syntax.

GIST2IG2.PRG This utility program takes sound effects made with The GIST.PRG
		 that you save out as FILENAME.C and turns that into a loadable
		 IG format sound effect that can load into any of the 20 sound
		 effect slots 0-19 they can also be played as note commands.
		 GIST may be found on https://forums.atariage.com  
		 the Atari ST Programming forum also if they ever link to it
		 www.atarimania.com  LeeActor.com website the orginal programmer
		 of GIST worth a visit.

MS2IG.PRG  This a program by Walter Holding he released into the Public
	Domain that I modified.  Thank you Mr. Holding  It takes Activision
	Music Studio MIDI files and turns them into streamable BYTE files
	for the N command MIDI buffer.  It only works for MIDI not the
	sound chip.  I may write another for MIDI to sound chip conversion
	in 2023 sometime. Activision Music Studio can be found on
	www.atarimania.com  I also included MS2IG.BAS if you want to try
	making a MIDI to IG note converter yourself.  It's in a older
	version of GFA Basic. There are a couple of .SNG files to test with.


N_UTILB.PRG	This takes song files made with the TAP A TUNE selection in the
		IGDEV10.PRG's DRAW section and turns them into BYTE streamable
		format for the 10K MIDI buffer.

KRX.PRG 	AUTO folder PROGAMS cuts off key click and key repeat
KX.PRG		cuts off key click only
RX.PRG		cuts off repeat only
		The key click and repeat can mess with the TAP A TUNE in the
		IGDEV10's DRAW F1 function.
		May be usful in other programs too.  I usually just leave the
		key click sound off all the time.

The .C files are sound effect files created with GIST.  The GIST2IG2.PRG
will convert them to IG format.  NEW_SNDS.TXT talks about this also.

The .GR1 and .IG files are IG graphic script examples.  They are old examples
and probably mostly created by hand.

This was all written with Laser C,  MegaMax C, and GFA Basic on the original
Atari STf  I bought in the 1980's with TOS 1.0  The only mods it has is a
dual TOS, flip a switch it goes from TOS 1.0 to TOS 1.04 and the TT Touch for
the keyboard.  I also used the UltraSatan II with ICD Pro 655a driver with
TOS 1.0 for developing the software.  For moving the programs to a .ST
simulated floppy TOS 1.04 and Peter Putnik's HD driver.  I have a NETUSB too
which is great for moving files in ZIPs ARCs and LZHs to from a PC.
I tried to address all I could think of to improve IG and keep it
backwards compatible.  I started working on this late 2019 here it is middle
of 2024.  I had some fun writing it, other times it was just hard.  
I hope you enjoy it.  I thought for a month or more about adding more "STUFF"
to IG nothing else panned out.  I think this may be the final version, unless
some bad bug pops up.  Still I might release some utility programs for IG or
come up with some wizz bang idea.  

...Larry
 

