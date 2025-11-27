mod loops;
use std::fmt;

pub use loops::*;

mod polymarker_kind;
pub use polymarker_kind::*;

mod pattern_type;
pub use pattern_type::*;

mod extended_commands;
pub use extended_commands::*;

mod text_effect;
pub use text_effect::*;

mod blit_operation;
pub use blit_operation::*;

mod sound_effect;
pub use sound_effect::*;

mod lines;
pub use lines::*;

mod drawing_mode;
pub use drawing_mode::*;

mod pen_type;
pub use pen_type::*;

mod initialization;
pub use initialization::*;

mod cursor_mode;
pub use cursor_mode::*;

mod ask_query;
pub use ask_query::*;

mod screen_clear;
pub use screen_clear::*;

mod stop_type;
pub use stop_type::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgsParameter {
    Value(i32),
    /// 'r' for random value (0-999)
    Random,
    /// 'R' for big random value (0-9999)
    BigRandom,
    /// 'x' - loop step forward variable (only valid in loop context)
    StepForward,
    /// 'y' - loop step reverse variable (only valid in loop context)
    StepReverse,
}

/// Specifies which text color layer to modify.
///
/// IGS command `G#c>layer,color:` uses this to select between
/// foreground (1) and background (0) text colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextColorLayer {
    /// Background text color (layer 0)
    Background,
    /// Foreground text color (layer 1)
    Foreground,
}

impl fmt::Display for TextColorLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextColorLayer::Background => write!(f, "0"),
            TextColorLayer::Foreground => write!(f, "1"),
        }
    }
}

/// Manages random number generation ranges for 'r' and 'R' parameters.
///
/// Default ranges:
/// - 'r' (Small): 0-199
/// - 'R' (Big): 0-199
pub struct ParameterBounds {
    small_range: (i32, i32), // Range for 'r'
    big_range: (i32, i32),   // Range for 'R'
}

impl Default for ParameterBounds {
    fn default() -> Self {
        Self {
            small_range: (0, 199),
            big_range: (0, 199),
        }
    }
}

impl ParameterBounds {
    /// Creates new parameter bounds with default ranges (0-199 for both)
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates the random ranges based on RandomRangeType from SetRandomRange command
    pub fn update(&mut self, range_type: &RandomRangeType) {
        match range_type {
            RandomRangeType::Small { min, max } => {
                self.small_range = (min.value(), max.value());
            }
            RandomRangeType::Big { min, max } => {
                self.big_range = (min.value(), max.value());
            }
        }
    }

    /// Gets the range for 'r' (small random) parameters
    pub fn small_range(&self) -> (i32, i32) {
        self.small_range
    }

    /// Gets the range for 'R' (big random) parameters
    pub fn big_range(&self) -> (i32, i32) {
        self.big_range
    }
}

impl IgsParameter {
    /// Evaluates the parameter to a concrete value.
    /// For fixed values, returns the value directly.
    /// For random values, generates a random number within the configured range.
    /// For loop step values, uses the provided step values (defaults to 0 if not in loop context).
    pub fn evaluate(&self, bounds: &ParameterBounds, loop_step_forward: i32, loop_step_reverse: i32) -> i32 {
        match self {
            IgsParameter::Value(v) => *v,
            IgsParameter::Random => {
                let (min, max) = bounds.small_range();
                fastrand::i32(min..=max)
            }
            IgsParameter::BigRandom => {
                let (min, max) = bounds.big_range();
                fastrand::i32(min..=max)
            }
            IgsParameter::StepForward => loop_step_forward,
            IgsParameter::StepReverse => loop_step_reverse,
        }
    }

    /// Evaluates the parameter with explicit min/max range.
    /// For fixed values, returns the value directly.
    /// For random values (both 'r' and 'R'), uses the provided range.
    /// For loop step values, defaults to 0 (should not be called in loop context).
    pub fn value_with_range(&self, min: i32, max: i32) -> i32 {
        match self {
            IgsParameter::Value(v) => *v,
            IgsParameter::Random | IgsParameter::BigRandom => fastrand::i32(min..=max),
            IgsParameter::StepForward | IgsParameter::StepReverse => 0,
        }
    }

    /// Returns the value if this is a fixed value, panics otherwise.
    /// Use this only when you know the parameter is not random.
    pub fn value(&self) -> i32 {
        match self {
            IgsParameter::Value(v) => *v,
            IgsParameter::Random => panic!("Cannot get fixed value from random parameter 'r'"),
            IgsParameter::BigRandom => panic!("Cannot get fixed value from random parameter 'R'"),
            IgsParameter::StepForward => panic!("Cannot get fixed value from loop step variable 'x'"),
            IgsParameter::StepReverse => panic!("Cannot get fixed value from loop step variable 'y'"),
        }
    }

    /// Returns true if this parameter requires evaluation (random or loop step)
    pub fn is_random(&self) -> bool {
        matches!(
            self,
            IgsParameter::Random | IgsParameter::BigRandom | IgsParameter::StepForward | IgsParameter::StepReverse
        )
    }
}

impl From<i32> for IgsParameter {
    fn from(value: i32) -> Self {
        IgsParameter::Value(value)
    }
}

impl fmt::Display for IgsParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IgsParameter::Value(v) => write!(f, "{}", v),
            IgsParameter::Random => write!(f, "r"),
            IgsParameter::BigRandom => write!(f, "R"),
            IgsParameter::StepForward => write!(f, "x"),
            IgsParameter::StepReverse => write!(f, "y"),
        }
    }
}

/// Random range configuration for 'r' and 'R' parameters
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RandomRangeType {
    /// Range for 'r' (lowercase random): min to max
    Small { min: IgsParameter, max: IgsParameter },
    /// Range for 'R' (uppercase random): min to max  
    Big { min: IgsParameter, max: IgsParameter },
}

impl fmt::Display for RandomRangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RandomRangeType::Small { min, max } => write!(f, "{},{}", min, max),
            RandomRangeType::Big { min, max } => write!(f, "{},{},{}", min, min, max),
        }
    }
}
