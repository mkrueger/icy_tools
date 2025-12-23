//! Avatar and status icons for collaboration users
//!
//! SVG icons are embedded at compile time.

use crate::fl;
use iced::widget::Svg;
use iced::Length;

// ============================================================================
// Avatar Icons (13 total)
// ============================================================================

/// Available avatar types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Avatar {
    #[default]
    Face = 0,
    Face2 = 1,
    Face3 = 2,
    Face4 = 3,
    Face5 = 4,
    Face6 = 5,
    AccountCircle = 6,
    Android = 7,
    FlutterDash = 8,
    Mood = 9,
    Person = 10,
    Psychology = 11,
    SentimentSatisfied = 12,
}

impl Avatar {
    /// Total number of avatars
    pub const COUNT: usize = 13;

    /// Get avatar from index (wraps around)
    pub fn from_index(index: usize) -> Self {
        match index % Self::COUNT {
            0 => Avatar::Face,
            1 => Avatar::Face2,
            2 => Avatar::Face3,
            3 => Avatar::Face4,
            4 => Avatar::Face5,
            5 => Avatar::Face6,
            6 => Avatar::AccountCircle,
            7 => Avatar::Android,
            8 => Avatar::FlutterDash,
            9 => Avatar::Mood,
            10 => Avatar::Person,
            11 => Avatar::Psychology,
            12 => Avatar::SentimentSatisfied,
            _ => Avatar::Face,
        }
    }

    /// Get avatar from user ID (deterministic assignment)
    pub fn from_user_id(user_id: u32) -> Self {
        // Use a simple hash to get variety
        let hash = user_id.wrapping_mul(2654435761); // Knuth multiplicative hash
        Self::from_index(hash as usize)
    }

    /// Get the SVG data for this avatar
    pub fn svg_data(&self) -> &'static [u8] {
        match self {
            Avatar::Face => AVATAR_FACE,
            Avatar::Face2 => AVATAR_FACE_2,
            Avatar::Face3 => AVATAR_FACE_3,
            Avatar::Face4 => AVATAR_FACE_4,
            Avatar::Face5 => AVATAR_FACE_5,
            Avatar::Face6 => AVATAR_FACE_6,
            Avatar::AccountCircle => AVATAR_ACCOUNT_CIRCLE,
            Avatar::Android => AVATAR_ANDROID,
            Avatar::FlutterDash => AVATAR_FLUTTER_DASH,
            Avatar::Mood => AVATAR_MOOD,
            Avatar::Person => AVATAR_PERSON,
            Avatar::Psychology => AVATAR_PSYCHOLOGY,
            Avatar::SentimentSatisfied => AVATAR_SENTIMENT_SATISFIED,
        }
    }

    /// Create an iced Svg widget for this avatar
    pub fn svg(&self, size: f32) -> Svg<'static> {
        let handle = iced::widget::svg::Handle::from_memory(self.svg_data());
        Svg::new(handle).width(Length::Fixed(size)).height(Length::Fixed(size))
    }
}

// ============================================================================
// User Status
// ============================================================================

/// User status (matches Moebius protocol)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum UserStatus {
    #[default]
    Active = 0,
    Idle = 1,
    Away = 2,
    Web = 3,
}

impl UserStatus {
    /// Create from protocol status byte
    pub fn from_byte(status: u8) -> Self {
        match status {
            0 => UserStatus::Active,
            1 => UserStatus::Idle,
            2 => UserStatus::Away,
            3 => UserStatus::Web,
            _ => UserStatus::Active,
        }
    }

    /// Get the SVG data for this status badge
    pub fn svg_data(&self) -> &'static [u8] {
        match self {
            UserStatus::Active => STATUS_CIRCLE_FILLED,
            UserStatus::Idle => STATUS_SCHEDULE,
            UserStatus::Away => STATUS_BEDTIME,
            UserStatus::Web => STATUS_PUBLIC,
        }
    }

    /// Get the color for this status
    pub fn color(&self) -> iced::Color {
        match self {
            UserStatus::Active => iced::Color::from_rgb(0.3, 0.8, 0.3), // Green
            UserStatus::Idle => iced::Color::from_rgb(0.9, 0.7, 0.2),   // Yellow/Orange
            UserStatus::Away => iced::Color::from_rgb(0.8, 0.3, 0.3),   // Red
            UserStatus::Web => iced::Color::from_rgb(0.3, 0.5, 0.9),    // Blue
        }
    }

    /// Create an iced Svg widget for this status badge
    pub fn svg(&self, size: f32) -> Svg<'static> {
        let handle = iced::widget::svg::Handle::from_memory(self.svg_data());
        Svg::new(handle).width(Length::Fixed(size)).height(Length::Fixed(size))
    }

    /// Display name for this status
    #[allow(dead_code)]
    pub fn name(&self) -> String {
        match self {
            UserStatus::Active => fl!("collab-status-active"),
            UserStatus::Idle => fl!("collab-status-idle"),
            UserStatus::Away => fl!("collab-status-away"),
            UserStatus::Web => fl!("collab-status-web"),
        }
    }
}

// ============================================================================
// Embedded SVG Data - Avatars
// ============================================================================

static AVATAR_FACE: &[u8] = include_bytes!("icons/avatars/face.svg");
static AVATAR_FACE_2: &[u8] = include_bytes!("icons/avatars/face_2.svg");
static AVATAR_FACE_3: &[u8] = include_bytes!("icons/avatars/face_3.svg");
static AVATAR_FACE_4: &[u8] = include_bytes!("icons/avatars/face_4.svg");
static AVATAR_FACE_5: &[u8] = include_bytes!("icons/avatars/face_5.svg");
static AVATAR_FACE_6: &[u8] = include_bytes!("icons/avatars/face_6.svg");
static AVATAR_ACCOUNT_CIRCLE: &[u8] = include_bytes!("icons/avatars/account_circle.svg");
static AVATAR_ANDROID: &[u8] = include_bytes!("icons/avatars/android.svg");
static AVATAR_FLUTTER_DASH: &[u8] = include_bytes!("icons/avatars/flutter_dash.svg");
static AVATAR_MOOD: &[u8] = include_bytes!("icons/avatars/mood.svg");
static AVATAR_PERSON: &[u8] = include_bytes!("icons/avatars/person.svg");
static AVATAR_PSYCHOLOGY: &[u8] = include_bytes!("icons/avatars/psychology.svg");
static AVATAR_SENTIMENT_SATISFIED: &[u8] = include_bytes!("icons/avatars/sentiment_satisfied.svg");

// ============================================================================
// Embedded SVG Data - Status Icons
// ============================================================================

static STATUS_CIRCLE_FILLED: &[u8] = include_bytes!("icons/circle_filled.svg");
static STATUS_SCHEDULE: &[u8] = include_bytes!("icons/schedule.svg");
static STATUS_BEDTIME: &[u8] = include_bytes!("icons/bedtime.svg");
static STATUS_PUBLIC: &[u8] = include_bytes!("icons/public.svg");
