#![allow(dead_code)]

//
// Constants taken from:
//
//   Z M O D E M . H     Manifest constants for ZMODEM
//    application to application file transfer protocol
//    Copyright 1991 Omen Technology Inc All Rights Reserved
//    04-17-89  Chuck Forsberg Omen Technology Inc
//
// See https://www.rpi.edu/dept/acm/packages/zmodem/3.17/sun4c_41/src/

pub const ZPAD: u8 = b'*'; // 052 Padding character begins frames
pub const ZDLE: u8 = 0x18; // Ctrl-X Zmodem escape - `ala BISYNC DLE
pub const ZDLEE: u8 = 0x58; // Escaped ZDLE as transmitted
pub const ZBIN: u8 = b'A'; // Binary frame indicator (CRC-16)
pub const ZHEX: u8 = b'B'; // HEX frame indicator
pub const ZBIN32: u8 = b'C'; // Binary frame with 32 bit FCS
pub const ZBINR32: u8 = b'D'; // RLE packed Binary frame with 32 bit FCS
pub const ZVBIN: u8 = b'a'; // Binary frame indicator (CRC-16)
pub const ZVHEX: u8 = b'b'; // HEX frame indicator
pub const ZVBIN32: u8 = b'c'; // Binary frame with 32 bit FCS
pub const ZVBINR32: u8 = b'd'; // RLE packed Binary frame with 32 bit FCS
pub const ZRESC: u8 = 0x7e; // RLE flag/escape character
pub const ZMAXHLEN: u8 = 16; // Max header information length  NEVER CHANGE
pub const ZMAXSPLEN: usize = 1024; // Max subpacket length  NEVER CHANGE

pub const CR: u8 = b'\r';
pub const CR_0x80: u8 = CR | 0x80;
pub const DLE: u8 = 0x10;
pub const DLE_0x80: u8 = DLE | 0x80;
pub const XON: u8 = 0x11;
pub const XOFF_0x80: u8 = XOFF | 0x80;
pub const XOFF: u8 = 0x13;
pub const XON_0x80: u8 = XON | 0x80;

/* ZDLE sequences */
/// CRC next, frame ends, header packet follows
pub const ZCRCE: u8 = b'h';
/// CRC next, frame continues nonstop
pub const ZCRCG: u8 = b'i';
/// CRC next, frame continues, ZACK expected
pub const ZCRCQ: u8 = b'j';
/// CRC next, ZACK expected, end of frame
pub const ZCRCW: u8 = b'k';
pub const ZRUB0: u8 = b'l'; /* Translate to rubout 0177 */
pub const ZRUB1: u8 = b'm'; /* Translate to rubout 0377 */

pub const ESC_DLE: u8 = DLE ^ 0x40;
pub const ESC_0X90: u8 = 0x90 ^ 0x40;
pub const ESC_0XON: u8 = 0x11 ^ 0x40;

pub const ESC_0X91: u8 = 0x91 ^ 0x40;
pub const ESC_0XOFF: u8 = 0x13 ^ 0x40;

pub const ESC_0X93: u8 = 0x93 ^ 0x40;
pub const ESC_0X0D: u8 = 0x0D ^ 0x40;
pub const ESC_0X8D: u8 = 0x8D ^ 0x40;

pub const ABORT_SEQ: [u8; 18] = [
    0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, /* 8 CAN */
    0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, /* 10 BS */
];

pub mod zfile_flag {
    pub const ZCBIN: u8 = 1; /* Binary transfer - inhibit conversion */
    pub const ZCNL: u8 = 2; /* Convert NL to local end of line convention */
    pub const ZCRESUM: u8 = 3; /* Resume interrupted file transfer */

    /* Management include options, one of these ored in ZF1 */
    // #define ZMSKNOLOC	0200	/* Skip file if not present at rx */
    /* Management options, one of these ored in ZF1 */
    // #define ZMMASK	037	/* Mask for the choices below */
    pub const ZMNEWL: u8 = 1; /* Transfer if source newer or longer */
    pub const ZMCRC: u8 = 2; /* Transfer if different file CRC or length */
    pub const ZMAPND: u8 = 3; /* Append contents to existing file (if any) */
    pub const ZMCLOB: u8 = 4; /* Replace existing file */
    pub const ZMNEW: u8 = 5; /* Transfer if source newer */

    /* Number 5 is alive ... */
    // #define ZMDIFF	6	/* Transfer if dates or lengths different */
    // #define ZMPROT	7	/* Protect destination file */
    /* Transport options, one of these in ZF2 */
    // #define ZTLZW	1	/* Lempel-Ziv compression */
    // #define ZTCRYPT	2	/* Encryption */
    // #define ZTRLE	3	/* Run Length encoding */
    /* Extended options for ZF3, bit encoded */
    // #define ZXSPARS	64	/* Encoding for sparse file operations */

    /* Parameters for ZCOMMAND frame ZF0 (otherwise 0) */
    // #define ZCACK1	1	/* Acknowledge, then do command */
}
pub mod zsinit_flag {
    pub const TESCCTL: u8 = 0x40;
    pub const TESC8: u8 = 0x80;
}

pub mod zrinit_flag {
    // Bit Masks for ZRINIT flags byte ZF0
    pub const CANFDX: u8 = 0x01; // Rx can send and receive true full duplex
    pub const CANOVIO: u8 = 0x02; // Rx can receive data during disk I/O
    pub const CANBRK: u8 = 0x04; // Rx can send a break signal
    pub const CANCRY: u8 = 0x08; // Receiver can decode RLE
    pub const CANLZW: u8 = 0x10; // Receiver can uncompress
    pub const CANFC32: u8 = 0x20; // Receiver can use 32 bit Frame Check
    pub const ESCCTL: u8 = 0x40; // Receiver expects ctl chars to be escaped
    pub const ESC8: u8 = 0x80; // Receiver expects 8th bit to be escaped

    pub const YOOHOO: u8 = 0xf1;
    pub const TSYNC: u8 = 0xae;

    // Bit Masks for ZRINIT flags byte ZF1
    const CANVHDR: u8 = 0x01; // Variable headers OK
                              /*
                              // Parameters for ZSINIT frame
                              const ZATTNLEN 32	// Max length of attention string
                              const ALTCOFF ZF1	// Offset to alternate canit string, 0 if not used
                              // Bit Masks for ZSINIT flags byte ZF0
                              const TESCCTL 0100	// Transmitter expects ctl chars to be escaped
                              const ESC8   0200	// Transmitter expects 8th bit to be escaped */
}

pub mod frame_types {
    pub const ZRQINIT: u8 = 0; // Request receive init
    pub const ZRINIT: u8 = 1; // Receive init
    pub const ZSINIT: u8 = 2; // Send init sequence (optional)
    pub const ZACK: u8 = 3; // ACK to above
    pub const ZFILE: u8 = 4; // File name from sender
    pub const ZSKIP: u8 = 5; // To sender: skip this file
    pub const ZNAK: u8 = 6; // Last packet was garbled
    pub const ZABORT: u8 = 7; // Abort batch transfers
    pub const ZFIN: u8 = 8; // Finish session
    pub const ZRPOS: u8 = 9; // Resume data trans at this position
    pub const ZDATA: u8 = 10; // Data packet(s) follow
    pub const ZEOF: u8 = 11; // End of file
    pub const ZFERR: u8 = 12; // Fatal Read or Write error Detected
    pub const ZCRC: u8 = 13; // Request for file CRC and response
    pub const ZCHALLENGE: u8 = 14; // Receiver's Challenge
    pub const ZCOMPL: u8 = 15; // Request is complete
    pub const ZCAN: u8 = 16; // Other end canned session with CAN*5
    pub const ZFREECNT: u8 = 17; // Request for free bytes on filesystem
    pub const ZCOMMAND: u8 = 18; // Command from sending program
    pub const ZSTDERR: u8 = 19; // Output to standard error, data follows
}

/*
/* Byte positions within header array */
#define ZF0	3	/* First flags byte */
#define ZF1	2
#define ZF2	1
#define ZF3	0

#define ZP0	0	/* Low order 8 bits of position */
#define ZP1	1
#define ZP2	2
#define ZP3	3	/* High order 8 bits of file position */
*/
