program IDV;

{ very early version of the iCEDraw Viewer }

uses Crt, Dos, Font, Strings;

{ here are some constants that are useful later }

const PC : array[0..15] of byte = (0,1,2,3,4,5,20,7,56,57,58,59,60,61,62,63);

const DefPal : array[0..15,0..2] of byte =  { default ANSI colors in RGB }

             ((0,0,0),(0,0,42),(0,42,0),(0,42,42),
              (42,0,0),(42,0,42),(42,21,0),(42,42,42),
              (21,21,21),(21,21,63),(21,63,21),(21,63,63),
              (63,21,21),(63,21,63),(63,63,21),(63,63,63));

type IDVPicture = record


{  New .IDF format:  * Version Number   : 4 bytes  (i.e.  '1.03');

                    * ScreenBoundaries : 4 words
                      (usually $00,$4F,$00,$15 for X1,Y1,X2,Y2.. you can prob
                      ignore this for now though)

                    * Screen Data      : compressed
                      (see above for screen data compression.. the max length
                       will be 200 lines when uncompressed... data is in
                       memory format (low byte = ASCII, hi byte = color atr)

                    * Font Data        : 4096 bytes
                      (256 chars * 16 bytes/char)

                    * Palette Data     : 48 bytes
                      (only covers colors 0-15 VGA palettes... IGNORE THIS
                       for now, b/c it's not supported and will be all 0's)}


      MAIN_BOUND_X1 : word;       { screen boundaries }
      MAIN_BOUND_Y1 : word;
      MAIN_BOUND_X2 : word;
      MAIN_BOUND_Y2 : word;
      VERSION       : string[4];  { version of loaded file }
      TOP_LINE      : word;
      ScreenData    : array[0..79,0..201+1] of word; { high byte = color, low = char }
      PaletteData   : array[0..15, 0..2] of byte;

      end;

var IDPic : IDVPicture; { this is the actual picture }

procedure SetUpVGACard; assembler;

            { turn off cursor }
            asm

                mov ax, 0100h
                mov cx, 0800h
                int 10h

            { put da thingz in 8 clock mode so the chars don't have that
               extra line on the right side }

                mov     dx,03c4h
                mov     ax,0100h
                out     dx,ax

                mov     dx,03c4h
                mov     ax,0301h
                out     dx,ax

                mov     dx,03c2h
                mov     al,063h
                out     dx,al

                mov     dx,03c4h
                mov     ax,0300h
                out     dx,ax

                mov     dx,03d4h
                mov     ax,4f09h
                out     dx,ax

            end;

procedure  InitVGA;

           begin

           ClrScr;
           SetupVGACard;
           LoadFontToMem; { from the FONT.TPU unit }

           end;



procedure LoadPic(var F : file);

{ this takes a filename, searches for it, decompresses, and saves the
  raw .BIN type data to ScreenData[] }

          var  Count, SubCount, PrevChar, MemoryPosition : word;

          begin

               { first read header crap }

               BlockRead(F, IDPic.VERSION,4);
               BlockRead(F, IDPic.MAIN_BOUND_X1,2);
               BlockRead(F, IDPic.MAIN_BOUND_Y1,2);
               BlockRead(F, IDPic.MAIN_BOUND_X2,2);
               BlockRead(F, IDPic.MAIN_BOUND_Y2,2);

               { Use Compression to Load Data }
               { Compression Format:

                     2-65535 : regular data (since we use a word for a char + it's color)

                     1 (x) (y) : do x repeats of y }

                 MemoryPosition := 0;
                 PrevChar := 0;
                 Count := 0;
                 repeat
                  BlockRead(F,SubCount,2);
                  If SubCount = 1 then begin
                     BlockRead(F,Count,2);
                     BlockRead(F,PrevChar,2);
                     for SubCount := 1 to Count do
                       IDPic.ScreenData[(MemoryPosition+SubCount-1) mod 80, (MemoryPosition+SubCount-1) div 80] := PrevChar;
                     MemoryPosition := MemoryPosition + Count - 1;
                     Count := 0;
                     end
                  else
                  IDPic.ScreenData[MemoryPosition mod 80, MemoryPosition div 80] := SubCount;
                  MemoryPosition := MemoryPosition + 1;
                 until MemoryPosition >= ((201-2) * 80 - 1);

               { end compression, now read in font and pal data }

               BlockRead(F, Font.FontData,SizeOf(Font.FontData));
               BlockRead(F, IDPic.PaletteData, SizeOf(IDPic.PaletteData));


               If Ord(IDPic.Version[3]) >= Ord('3') then

                 { if the version is greater than 1.3, then we make sure
                   to change the VGA palette to the one specified in the
                   file. to do this we use VGA sequencer ports directly:

                   Port[$3C8] := PC[Color to change]

                   (Note! We must use the PC[] color table because after
                    ansi color 7, the port we write to does NOT correspond
                    to the color we want to change. don't quite know why,
                    probably just because VGA chips suck.)

                   Port[$3C9] := Red   Value of the New Color (from 0-63)
                   Port[$3C9] := Green Value of the New Color (from 0-63)
                   Port[$3C9] := Blue  Value of the New Color (from 0-63)
                 }

                For Count := 0 to 15 do begin
                    Port[$3c8] := PC[Count];
                    Port[$3c9] := IDPic.paletteData[Count,0];
                    Port[$3c9] := IDPic.paletteData[Count,1];
                    Port[$3c9] := IDPic.paletteData[Count,2];
                 end
                 else for Count := 0 to 15 do begin

                    Port[$3c8] := PC[Count];
                    Port[$3c9] := defpal[Count,0];
                    Port[$3c9] := defpal[Count,1];
                    Port[$3c9] := defpal[Count,2];
                 end;

          end;

function GetKeyPress : word;

         var Ch : char;

         begin
         If KeyPressed then begin

         Ch := ReadKey;
         GetKeyPress := Ord(Ch);
         If Ch = #0 then begin
            Ch := ReadKey;
            GetKeyPress := Ord(Ch) + 256;
            end;
         end;
         end;

procedure DisplayPicture;

          var X,Y,StartLine : byte;
              Ch : word; { keypress }

          begin

          StartLine := 0; { current top line of pic }
          TextColor(15); TextBackground(0); ClrScr; GotoXY(1,25);
          Write(' IDV v1.0 (C) Necros/iCE  -  Use Cursor Keys to Scroll  -  iCE Productions 1995');

          repeat

          { display pic, this is really shitty and slow but i'm sure
            after you see how it works you could write a nice fast ASM
            one ;) }

          for X := 0 to 23 do for Y := 0 to 79  { $b800 is start of text segment }
           do MemW[$b800:Y*2+X*160] := IDPic.ScreenData[Y,X+StartLine];

          { now read a key              }

            Ch := GetKeyPress;
            Case Ch of

            $0148 : If StartLine > 0 then StartLine := StartLine - 1;
            $0150 : If StartLine < 200 then StartLine := StartLine + 1;

            end;
          until Ch = $001B; { ESC key }

          end;



var FN : string;
    F  : file;

begin

writeln('IDF Viewer Version 1.0 by Necros / iCE');

{ check for the right parameters }

  if ParamCount <> 1 then begin
     Writeln('Syntax: IDV {filename.idf}');
     Halt(1);
     end;

{ now set up the new file }

  FN := ParamStr(1);
  Assign(F, FN);
  {$I-}
  Reset(F,1);
  {$I+}
  If IOResult = 0 then
     begin
      Writeln('Decompressing .IDF file...');
      LoadPic(F);
     end
     else begin
     Write('Error: File not Found');
     Halt(1);
     end;

InitVGA;
DisplayPicture;

{ now close up everything }
  asm
   mov ax, 0003 { pop back into 80x25 normal mode }
   int 10h
  end;

TextColor(15);
TextBackground(0);
end.