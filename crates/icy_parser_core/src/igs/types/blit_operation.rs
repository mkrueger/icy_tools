/// BitBlit operation type for GrabScreen command
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlitOperation {
    /// Screen to screen: copy from one screen area to another
    ScreenToScreen {
        /// Upper left corner X of source
        src_x1: i32,
        /// Upper left corner Y of source
        src_y1: i32,
        /// Lower right corner X of source
        src_x2: i32,
        /// Lower right corner Y of source
        src_y2: i32,
        /// Upper left corner X of destination
        dest_x: i32,
        /// Upper left corner Y of destination
        dest_y: i32,
    },
    /// Screen to memory: save screen area to memory
    ScreenToMemory {
        /// Upper left corner X of source
        src_x1: i32,
        /// Upper left corner Y of source
        src_y1: i32,
        /// Lower right corner X of source
        src_x2: i32,
        /// Lower right corner Y of source
        src_y2: i32,
    },
    /// Memory to screen: restore entire memory buffer to screen
    MemoryToScreen {
        /// Upper left corner X of destination
        dest_x: i32,
        /// Upper left corner Y of destination
        dest_y: i32,
    },
    /// Piece of memory to screen: restore part of memory buffer to screen
    PieceOfMemoryToScreen {
        /// Upper left corner X of source in memory
        src_x1: i32,
        /// Upper left corner Y of source in memory
        src_y1: i32,
        /// Lower right corner X of source in memory
        src_x2: i32,
        /// Lower right corner Y of source in memory
        src_y2: i32,
        /// Upper left corner X of destination
        dest_x: i32,
        /// Upper left corner Y of destination
        dest_y: i32,
    },
    /// Memory to memory: copy within memory buffer
    MemoryToMemory {
        /// Upper left corner X of source in memory
        src_x1: i32,
        /// Upper left corner Y of source in memory
        src_y1: i32,
        /// Lower right corner X of source in memory
        src_x2: i32,
        /// Lower right corner Y of source in memory
        src_y2: i32,
        /// Upper left corner X of destination in memory
        dest_x: i32,
        /// Upper left corner Y of destination in memory
        dest_y: i32,
    },
}

/// BitBlit logical operation mode for GrabScreen command
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlitMode {
    /// Mode 0: Clear destination block
    Clear = 0,
    /// Mode 1: S AND D
    And = 1,
    /// Mode 2: S AND (NOT D)
    AndNot = 2,
    /// Mode 3: Replace mode (S)
    Replace = 3,
    /// Mode 4: (NOT S) AND D - Erase mode
    Erase = 4,
    /// Mode 5: Destination unchanged (D)
    Unchanged = 5,
    /// Mode 6: S XOR D - XOR mode
    Xor = 6,
    /// Mode 7: S OR D - Transparent mode
    Transparent = 7,
    /// Mode 8: NOT (S OR D)
    NotOr = 8,
    /// Mode 9: NOT (S XOR D)
    NotXor = 9,
    /// Mode 10: NOT D
    NotD = 10,
    /// Mode 11: S OR (NOT D)
    OrNot = 11,
    /// Mode 12: NOT S
    NotS = 12,
    /// Mode 13: (NOT S) OR D - Reverse Transparent mode
    ReverseTransparent = 13,
    /// Mode 14: NOT (S AND D)
    NotAnd = 14,
    /// Mode 15: Set all bits to 1
    Fill = 15,
}

impl From<i32> for BlitMode {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Clear,
            1 => Self::And,
            2 => Self::AndNot,
            3 => Self::Replace,
            4 => Self::Erase,
            5 => Self::Unchanged,
            6 => Self::Xor,
            7 => Self::Transparent,
            8 => Self::NotOr,
            9 => Self::NotXor,
            10 => Self::NotD,
            11 => Self::OrNot,
            12 => Self::NotS,
            13 => Self::ReverseTransparent,
            14 => Self::NotAnd,
            15 => Self::Fill,
            _ => Self::Replace,
        }
    }
}
