//! Stable SCREEN machine IDs: C64=1, C128 VIC=2, VIC-20=3, PET=4, C16=5,
//! PET 80=6, C128 VDC=7, Atari ST=8, Atari 8-bit=9, Viewdata=10, Mode 7=11.
//! PETSCII modes are upper/graphics=0 and lower/upper=1 (VDC attributes remain per cell).
//! ST modes are low=0, medium=1, high=2; Atari 8-bit modes are ANTIC=0, XEP80=1.
//! Viewdata and Mode 7 currently have mode 0. IDs must not be renumbered or reused.

use crate::{PetsciiCase, PetsciiMachine, TerminalResolution};

/// Hardware interpretation of a text screen, independent of its character encoding and fonts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MachineMode {
    Petscii { machine: PetsciiMachine, charset: PetsciiCase },
    AtariSt { resolution: TerminalResolution },
    Atari8Bit { xep80: bool },
    Viewdata,
    Mode7,
}

impl serde::Serialize for MachineMode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.to_screen_ids(), serializer)
    }
}

impl<'de> serde::Deserialize<'de> for MachineMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (machine, mode) = <(u16, u16) as serde::Deserialize>::deserialize(deserializer)?;
        Self::from_screen_ids(machine, mode).ok_or_else(|| serde::de::Error::custom(format!("unsupported machine/mode {machine}/{mode}")))
    }
}

impl MachineMode {
    pub(crate) fn to_screen_ids(self) -> (u16, u16) {
        match self {
            Self::Petscii { machine, charset } => {
                let machine = match machine {
                    PetsciiMachine::C64 => 1,
                    PetsciiMachine::C128 => 2,
                    PetsciiMachine::Vic20 => 3,
                    PetsciiMachine::Pet => 4,
                    PetsciiMachine::C16 => 5,
                    PetsciiMachine::Pet80 => 6,
                    PetsciiMachine::C128Vdc => 7,
                };
                (machine, u16::from(charset == PetsciiCase::Lower))
            }
            Self::AtariSt { resolution } => (
                8,
                match resolution {
                    TerminalResolution::Low => 0,
                    TerminalResolution::Medium => 1,
                    TerminalResolution::High => 2,
                },
            ),
            Self::Atari8Bit { xep80 } => (9, u16::from(xep80)),
            Self::Viewdata => (10, 0),
            Self::Mode7 => (11, 0),
        }
    }

    pub(crate) fn from_screen_ids(machine: u16, mode: u16) -> Option<Self> {
        let petscii = match machine {
            1 => Some(PetsciiMachine::C64),
            2 => Some(PetsciiMachine::C128),
            3 => Some(PetsciiMachine::Vic20),
            4 => Some(PetsciiMachine::Pet),
            5 => Some(PetsciiMachine::C16),
            6 => Some(PetsciiMachine::Pet80),
            7 => Some(PetsciiMachine::C128Vdc),
            _ => None,
        };
        if let Some(machine) = petscii {
            return match mode {
                0 => Some(Self::Petscii {
                    machine,
                    charset: PetsciiCase::Upper,
                }),
                1 => Some(Self::Petscii {
                    machine,
                    charset: PetsciiCase::Lower,
                }),
                _ => None,
            };
        }
        match (machine, mode) {
            (8, 0) => Some(Self::AtariSt {
                resolution: TerminalResolution::Low,
            }),
            (8, 1) => Some(Self::AtariSt {
                resolution: TerminalResolution::Medium,
            }),
            (8, 2) => Some(Self::AtariSt {
                resolution: TerminalResolution::High,
            }),
            (9, 0) => Some(Self::Atari8Bit { xep80: false }),
            (9, 1) => Some(Self::Atari8Bit { xep80: true }),
            (10, 0) => Some(Self::Viewdata),
            (11, 0) => Some(Self::Mode7),
            _ => None,
        }
    }
}
