{$A+,B-,D+,E+,F-,G-,I+,L+,N-,O-,P-,Q+,R+,S+,T-,V-,X+,Y+}
{$M 4096,0,655360}
PROGRAM ADF_TO_XBIN_Converter;
(*****************************************************************************

 ADF to XBIN conversion program.

 ADF2XBIN converts an ADF file to a fully compliant compressed XBIN file.

 Compression routine is identical to the BIN2XBIN compression.  ADF is always
 80 characters wide, which makes memory management somewhat simpler compared
 to the BIN2XBIN utility.

 Any SAUCE info is stripped from the ADF.  As such SAUCEd ADF files should
 properly convert to XBIN.

 Alternate palette and Font information is copied into the XBIN as-is.  No
 checking is performed to see if the used palette and/or font match the
 default.  If default font and/or palette are indeed used, they could be
 stripped out of the XBIN.

*****************************************************************************)

USES  CRT,
      DOS,
      STM;

TYPE  Char2    = ARRAY [0..1]  OF Char;
      Char4    = ARRAY [0..3]  OF Char;
      Char5    = ARRAY [0..4]  OF Char;
      Char8    = ARRAY [0..7]  OF Char;
      Char20   = ARRAY [0..19] OF Char;
      Char35   = ARRAY [0..34] OF Char;
      Char64   = ARRAY [0..63] OF Char;


{ ================================= SAUCE ================================= }

Const SAUCE_ID      : Char5 = 'SAUCE';
      CMT_ID        : Char5 = 'COMNT';

TYPE  SAUCERec = RECORD                { ÚÄÄ Implemented in Version ?        }
                   ID       : Char5;   { 00  'SAUCE'                         }
                   Version  : Char2;   { 00  '00'                            }
                   Title    : Char35;  { 00  Title of the file               }
                   Author   : Char20;  { 00  Creator of the file             }
                   Group    : Char20;  { 00  Group creator belongs to        }
                   Date     : Char8;   { 00  CCYYMMDD                        }
                   FileSize : Longint; { 00  Original FileSize               }
                   DataType : Byte;    { 00  Type of Data                    }
                   FileType : Byte;    { 00  What type of file is it ?       }
                   TInfo1   : Word;    { 00  \                               }
                   TInfo2   : Word;    { 00   \ Type Info Zone               }
                   TInfo3   : Word;    { 00   /                              }
                   TInfo4   : Word;    { 00  /                               }
                   Comments : Byte;    { 00  Number of Comment lines         }
                   Flags    : Byte;    { 00* Bit flags                       }
                   Filler   : Array[1..22] of Char;
                 END;

{ ================================== XBIN ================================= }

CONST XB_ID     : Char4 = 'XBIN';
TYPE  XB_Header = RECORD
                    ID      : Char4;
                    EofChar : Byte;
                    Width   : Word;
                    Height  : Word;
                    FontSize: Byte;
                    Flags   : Byte;
                  END;
      BINChr    = RECORD               { BIN Character/Attribute pair. }
                    CASE Boolean OF
                    TRUE  : (
                             CharAttr : Word;
                            );
                    FALSE : (
                             Character : Byte;
                             Attribute : Byte;
                            );
                  END;
      BINChrAry = ARRAY[0..79] OF BINChr;  { This size is different from BIN2XBIN }

      { Conversion table for converting an EGA to VGA palette }
CONST STD_EGA_TO_VGA_PAL : ARRAY [0..15] OF BYTE =
                                (0,1,2,3,4,5,20,7,56,57,58,59,60,61,62,63);

VAR   XBHdr     : XB_Header;
      SAUCE     : SauceRec;
      CMT       : Char5;
      ErrCode   : Integer;
      XB, ADF   : STREAM;              { File stream, see STM unit }
      Lines     : Word;
      ADFSize   : Longint;
      ADFFont   : ARRAY[0..4095] OF CHAR;
      ADFPal    : ARRAY[0..191] OF CHAR;
      BIN       : BinChrAry;
      Counter   : Integer;


{ ABORT Execution and display error message }
PROCEDURE Abort (Str: String);
BEGIN
   WriteLn;
   WriteLn('ADF2XBIN V1.00.  Execution aborted.');
   WriteLn;
   WriteLn(Str);
   WriteLn;
   Halt(2);
END;


{ Display command syntax and abort }
PROCEDURE HelpText;
BEGIN
   WriteLn('ADF2XBIN converts ADF files to XBIN.');
   WriteLn;
   WriteLn('Correct Syntax:  ADF2XBIN <ADFFILE> <XBINFILE>');
   WriteLn;
   Halt(1);
END;


{ Return size of File in Bytes or -1 if it does not exist or can't determine }
{ size                                                                       }
FUNCTION FileExist (FName:String) : LongInt;
VAR F      : FILE;
BEGIN
  {$i-}
  ASSIGN(F,FName);
  RESET(F,1);
  IF (IOResult=0) THEN BEGIN
     FileExist := FileSize(F);          { Return Size of file        }
     IF (IOResult<>0) THEN
        FileExist:=-1;                  { Return -1 : File not Found }
     Close(F);
  END
  ELSE
     FileExist:=-1;                     { Return -1 : File not Found }
  {$i+}
END;


{ÛÛÛ XBIN Compression START ÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛ}

{
  Introductory note.

  The XBIN compression used here is a single step compression algorythm.
  What this means is that we will compress the data one character/attribute
  pair at a time letting that char/attr pass through all the necessary
  conditions until it has been decided what has to be done with it.
  While not being the fastest or most compact algorythm available, it does
  make the algorythm a lot easier to understand.

  This XBIN compression routine uses a temporary buffer (an array) to hold
  the current run-count and compressed data.  Since the maximum run-count is
  64, this buffer only needs to be 129 bytes in size (1 byte for the
  run-count, and 64 times a char/attr pair when no compression is taking
  place.

  The overall idea behind this routine is pretty simple..  here's the rough
  outline:

  WHILE (Still_characters_to_process)
     IF (A_run_is_busy)
        IF (Stop_this_run_for_whatever_reason)
           Write_run_to_disk;
        ENDIF
     ENDIF
     IF (Run_is_still_busy)
        add_current_char/attr_pair_to_run;
     ELSE
        start_a_new_run_with_char/attr_pair;
     ENDIF
  ENDWHILE
  IF (A_run_is_busy)
     Write_run_to_disk;
  ENDIF

  It looks simple, but implementing it effectively is tricky.  The most
  involving part will be the "Stop_this_run_for_whatever_reason" routine.
  There are several reasons for wishing to stop the run.
    1) The current run is 64 characters wide, thus, another char/attr pair
       can't be added.
    2) The current compression can no longer be maintained as the new
       char/attr pair does not match.
    3) Aborting the run prematurely offers a possibility to restart using a
       better compression method.
  Reasons 1 and 2, are easy enough to deal with, the third provides the path
  to optimal compression.  The better the conditions are made for aborting in
  favour of a better compression method, the better compression will be.

  Enough about theory, on to the actual code.
}

PROCEDURE XBIN_Compress (VAR BIN:BINChrAry; BIN_Width : WORD);

CONST NO_COMP       = $00;
      CHAR_COMP     = $40;
      ATTR_COMP     = $80;
      CHARATTR_COMP = $C0;

VAR   CompressBuf   : Array[0..2*64] of Byte;
      RunCount      : Word;
      RunMode       : Byte;
      RunChar       : BINChr;
      CB_Index      : Word;            { Index into CompressBuf               }
      BIN_Index     : Word;            { Index into BIN_Line                  }
      EndRun        : Boolean;

BEGIN
  RunCount := 0;                       { There's no run busy                  }
  BIN_Index:= 0;

  WHILE (BIN_Index<BIN_Width) DO BEGIN { Still characters to process ?        }
     IF (RunCount>0) THEN BEGIN        { A run is busy                        }
        EndRun := FALSE;               { Assume we won't need to end the run  }

        IF (RunCount=64) THEN BEGIN    { We reached the longest possible run? }
           EndRun:=TRUE;               { Yes, end the current run             }
        END
        ELSE BEGIN
           { A run is currently busy.  Check to see if we can/will continue...}
           CASE RunMode OF
              NO_COMP       : BEGIN
                { No compression can always continue, since it does not       }
                { require on the character and/or attribute to match its      }
                { predecessor                                                 }

                { === No compression run.  Aborting this will only have       }
                {     benefit if we can start a run of at least 3 character   }
                {     or attribute compression. OR a run of at least 2        }
                {     char/attr compression                                   }
                {     The required run of 3 (2) takes into account the fact   }
                {     that a run must be re-issued if no more than 3 (2)      }
                {     BIN pairs can be compressed                             }
                IF (BIN_Width-BIN_Index>=2) AND
                   (BIN[BIN_Index].CharAttr=BIN[BIN_Index+1].CharAttr) THEN BEGIN
                   EndRun:=TRUE;
                END
                ELSE IF (BIN_Width-BIN_Index>=3) AND
                        (BIN[BIN_Index].Character=BIN[BIN_Index+1].Character) AND
                        (BIN[BIN_Index].Character=BIN[BIN_Index+2].Character) THEN BEGIN
                   EndRun:=TRUE;
                END
                ELSE IF (BIN_Width-BIN_Index>=3) AND
                        (BIN[BIN_Index].Attribute=BIN[BIN_Index+1].Attribute) AND
                        (BIN[BIN_Index].Attribute=BIN[BIN_Index+2].Attribute) THEN BEGIN
                   EndRun:=TRUE;
                END
              END;

              CHAR_COMP     : BEGIN
                { Character compression needs to be ended when the new        }
                { character no longer matches the run-character               }
                IF (BIN[BIN_Index].Character<>RunChar.Character) THEN BEGIN
                   EndRun:=TRUE;
                END
                { === Aborting an character compression run will only have    }
                {     benefit if we can start a run of at least 3 char/attr   }
                {     pairs.                                                  }
                ELSE IF (BIN_Width-BIN_Index>=3) AND
                        (BIN[BIN_Index].CharAttr=BIN[BIN_Index+1].CharAttr) AND
                        (BIN[BIN_Index].CharAttr=BIN[BIN_Index+2].CharAttr) THEN BEGIN
                   EndRun:=TRUE;
                END
              END;

              ATTR_COMP     : BEGIN
                { Attribute compression needs to be ended when the new        }
                { attribute no longer matches the run-attribute               }
                IF (BIN[BIN_Index].Attribute<>RunChar.Attribute) THEN BEGIN
                   EndRun:=TRUE;
                END
                { === Aborting an attribute compression run will only have    }
                {     benefit if we can start a run of at least 3 char/attr   }
                {     pairs.                                                  }
                ELSE IF (BIN_Width-BIN_Index>=3) AND
                        (BIN[BIN_Index].CharAttr=BIN[BIN_Index+1].CharAttr) AND
                        (BIN[BIN_Index].CharAttr=BIN[BIN_Index+2].CharAttr) THEN BEGIN
                   EndRun:=TRUE;
                END
              END;

              CHARATTR_COMP : BEGIN
                { Character/Attribute compression needs to be ended when the  }
                { new char/attr no longer matches the run-char/attr           }
                IF (BIN[BIN_Index].CharAttr<>RunChar.CharAttr) THEN BEGIN
                   EndRun:=TRUE;
                END
                { === Aborting a char/attr compression will never yield any   }
                {     benefit                                                 }
              END;
           END; { CASE }
        END; { IF }

        IF EndRun THEN BEGIN
           CompressBuf[0] := RunMode + (RunCount-1);
           STM_Write(XB,CompressBuf,CB_Index);
           IF (XB.LastErr<>STM_OK) THEN Abort('Error Writing File');

           RunCount:=0;                { Run no longer busy                   }
        END; { IF }
     END; { IF }

     IF (RunCount>0) THEN BEGIN        { Run is still busy ?                  }
         { === Add new char/attr to current run as appropriate for compression}
         {     method in use                                                  }
         CASE RunMode OF
            NO_COMP       : BEGIN
               { Store Char/Attr pair                                         }
               CompressBuf[CB_Index]:=BIN[BIN_Index].Character;
               CompressBuf[CB_Index+1]:=BIN[BIN_Index].Attribute;
               Inc(CB_Index,2);
            END;

            CHAR_COMP     : BEGIN
               { Store Attribute                                              }
               CompressBuf[CB_Index]:=BIN[BIN_Index].Attribute;
               Inc(CB_Index);
            END;

            ATTR_COMP     : BEGIN
               { Store character                                              }
               CompressBuf[CB_Index]:=BIN[BIN_Index].Character;
               Inc(CB_Index);
            END;

            CHARATTR_COMP : BEGIN
               { Nothing to change, only RunCount ever changes                }
            END;
         END;
     END
     ELSE BEGIN                        { Run not busy, Start a new one        }
         CB_Index := 1;                { Skip index 0 (for run-count byte)    }

         IF (BIN_Width-BIN_Index>=2) THEN BEGIN { At least 2 more to do       }
            IF (BIN[BIN_Index].CharAttr=BIN[BIN_Index+1].CharAttr) THEN
               { === We can use char/attr compression                         }
               RunMode:=CHARATTR_COMP
            ELSE IF (BIN[BIN_Index].Character=BIN[BIN_Index+1].Character) THEN
               { === We can use character compression                         }
               RunMode:=CHAR_COMP
            ELSE IF (BIN[BIN_Index].Attribute=BIN[BIN_Index+1].Attribute) THEN
               { === We can use attribute compression                         }
               RunMode:=ATTR_COMP
            ELSE
               { === We can't use any compression                             }
               RunMode:=NO_COMP;
         END
         ELSE                          { Last character, use no-compression   }
            RunMode:=NO_COMP;

         IF (RunMode=ATTR_COMP) THEN BEGIN
                                       { Attr compression has Attr first !!   }
            CompressBuf[CB_Index]:=BIN[BIN_Index].Attribute;
            CompressBuf[CB_Index+1]:=BIN[BIN_Index].Character;
         END
         ELSE BEGIN
            CompressBuf[CB_Index]:=BIN[BIN_Index].Character;
            CompressBuf[CB_Index+1]:=BIN[BIN_Index].Attribute;
         END;

         Inc(CB_Index,2);
         RunChar.CharAttr:=BIN[BIN_Index].CharAttr;
     END; { IF }

     Inc(RunCount);                    { RunCount is now one more             }
     Inc(BIN_Index);                   { One char/attr pair processed         }
  END;

  IF (RunCount>0) THEN BEGIN
     CompressBuf[0] := RunMode + (RunCount-1);
     STM_Write(XB,CompressBuf,CB_Index);
     IF (XB.LastErr<>STM_OK) THEN Abort('Error Writing File');
  END;
END;

{ÛÛÛ XBIN Compression END ÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛÛ}


BEGIN { *** MAIN *** }
  WriteLn ('ADF TO XBIN Converter V1.00.');
  WriteLn ('Coded by Tasmaniac / ACiD.');
  WriteLn ('Sourcecode placed into the public domain, use and modify freely');
  WriteLn;

  { --- Check passed parameter ------------------------------------------- }
  IF (ParamCount<>2) THEN HelpText;

  { --- Open ADF --------------------------------------------------------- }
  WriteLn ('Opening ADF...');
  STM_Open(ADF, ParamStr(1), NOCREATE);
  IF (ADF.LastErr<>STM_OK) THEN Abort('Error opening ADF file '+ParamStr(1));

  { --- Determine size of unSAUCED ADF ----------------------------------- }
  WriteLn ('Determining actual size of ADF...');
  ADFSize := STM_GetSize(ADF);
  IF (ADF.LastErr<>STM_OK) THEN Abort('Error determinig size of ADF file');

  STM_Goto(ADF,ADFSize-Sizeof(SAUCE));
  IF (ADF.LastErr<>STM_OK) THEN Abort('Error seeking SAUCE info in ADF file');

  STM_Read(ADF,SAUCE,sizeof(SAUCE));
  IF (ADF.LastErr<>STM_OK) THEN Abort('Error reading SAUCE info from ADF file');

  IF (SAUCE.ID=SAUCE_ID) THEN BEGIN
     Dec(ADFSize,sizeof(SAUCE));{ Reduce ADF size, accounting for SAUCE }

     IF (SAUCE.Comments>0) THEN BEGIN
                        { Commentblock added to Sauce, check if it's valid }
        STM_Goto(ADF,ADFSize-(SAUCE.Comments*64)-5);
        IF (ADF.LastErr<>STM_OK) THEN Abort('Error seeking SAUCE COMMENT info in ADF file');

        STM_Read(ADF,CMT,sizeof(CMT));
        IF (ADF.LastErr<>STM_OK) THEN Abort('Error reading SAUCE info from ADF file');

        IF (CMT<>CMT_ID) THEN
           Abort('Invalid SAUCE COMMENT block in ADF');
        DEC(ADFSize,(SAUCE.Comments*64)+5); { Adjust to account for comments }
     END;

     Dec(ADFSize);   { Account for EOF character preceding Sauce & comment }
     IF (SAUCE.FileSize<>ADFSize) THEN
        Abort('Calculated size of ADF and size according to SAUCE don''t match');
  END;

  Lines := (ADFSize - 1 - 192 - 4000) DIV 160; { Number lines in ADF }

  STM_Goto(ADF,1);                     { Start of ADF, skip version byte }
  IF (ADF.LastErr<>STM_OK) THEN Abort('Error seeking to start of ADF');

  { ===========================  CREATE XBIN  ============================ }
  WriteLn ('Creating XBIN...');
  STM_Create(XB,Paramstr(2));
  IF (XB.LastErr<>STM_OK) THEN Abort('Error creating XBIN file '+ParamStr(2));

  { --- Write Header ----------------------------------------------------- }
  WriteLn ('Writing XBIN header...');
  XBHdr.ID      := XB_ID; { 'XBIN' ID                       }
  XBHdr.EofChar := 26;    { Mark EOF when TYPEing XBIN      }
  XBHDr.Width   := 80;    { ADF is always 80 wide           }
  XBHdr.Height  := Lines; { This is what we just calculated }
  XBHdr.FontSize:= 16;    { Fonts in ADF are 16 pixels high }
  XBHdr.Flags   := $0F;   { Palette present, Font present, Compresed, }
                          { Non-Blinking (ADF doesn't have blinking), }
                          { 256 character font                        }
  STM_Write(XB,XBHdr,Sizeof(XBHdr));
  IF (XB.LastErr<>STM_OK) THEN Abort('Error writing XBIN file');

  { --- Copy Palette ----------------------------------------------------- }
  WriteLn ('Copying palette from ADF to XBIN...');
  STM_Read (ADF,ADFPal,sizeof(ADFPal));
  IF (ADF.LastErr<>STM_OK) THEN Abort('Error reading palette from ADF file');
  { For some reason ADF stores 64 palette values while only 16 colors can  }
  { be active at any one time.  Copy the relevant portion of this palette  }
  {  to the XBIN                                                           }
  FOR Counter:=0 TO 15 DO BEGIN
     STM_Write(XB,ADFPal[STD_EGA_TO_VGA_PAL[Counter]*3],3);
     IF (XB.LastErr<>STM_OK) THEN Abort('Error writing palette in XBIN file');
  END;

  { --- Copy Font -------------------------------------------------------- }
  WriteLn ('Copying font from ADF to XBIN...');
  STM_Read (ADF,ADFFont,sizeof(ADFFont));
  IF (ADF.LastErr<>STM_OK) THEN Abort('Error reading font from ADF file');

  STM_Write(XB,ADFFont,sizeof(ADFFont));
  IF (XB.LastErr<>STM_OK) THEN Abort('Error writing font in XBIN file');

  { --- Write image data ------------------------------------------------- }
  WriteLn('Converting image data from ADF to XBIN...');
  FOR Lines:=1 to XBHdr.Height DO BEGIN
     STM_Read (ADF,BIN,160); { Read one screen line }
     IF (ADF.LastErr<>STM_OK) THEN Abort('Error reading image date from ADF file');

     Write(Lines,'/',XBHdr.Height,#13);
     XBIN_Compress(BIN,XBHdr.Width);
  END;
  Write('':79,#13);

  STM_Close(ADF);
  STM_Close(XB);

  WriteLn ('Conversion complete.');
END.





