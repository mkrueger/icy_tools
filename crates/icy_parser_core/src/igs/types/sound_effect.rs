/// Sound effects for BellsAndWhistles command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundEffect {
    /// Alien Invasion
    AlienInvasion = 0,
    /// Red Alert
    RedAlert = 1,
    /// Gunshot
    Gunshot = 2,
    /// Laser 1
    Laser1 = 3,
    /// Jackhammer
    Jackhammer = 4,
    /// Teleport
    Teleport = 5,
    /// Explosion
    Explosion = 6,
    /// Laser 2
    Laser2 = 7,
    /// Longbell
    Longbell = 8,
    /// Surprise
    Surprise = 9,
    /// Radio Broadcast
    RadioBroadcast = 10,
    /// Bounce Ball
    BounceBall = 11,
    /// Eerie Sound
    EerieSound = 12,
    /// Harley Motorcycle
    HarleyMotorcycle = 13,
    /// Helicopter
    Helicopter = 14,
    /// Steam Locomotive
    SteamLocomotive = 15,
    /// Wave
    Wave = 16,
    /// Robot Walk
    RobotWalk = 17,
    /// Passing Plane
    PassingPlane = 18,
    /// Landing
    Landing = 19,
}

impl From<i32> for SoundEffect {
    fn from(value: i32) -> Self {
        match value {
            0 => SoundEffect::AlienInvasion,
            1 => SoundEffect::RedAlert,
            2 => SoundEffect::Gunshot,
            3 => SoundEffect::Laser1,
            4 => SoundEffect::Jackhammer,
            5 => SoundEffect::Teleport,
            6 => SoundEffect::Explosion,
            7 => SoundEffect::Laser2,
            8 => SoundEffect::Longbell,
            9 => SoundEffect::Surprise,
            10 => SoundEffect::RadioBroadcast,
            11 => SoundEffect::BounceBall,
            12 => SoundEffect::EerieSound,
            13 => SoundEffect::HarleyMotorcycle,
            14 => SoundEffect::Helicopter,
            15 => SoundEffect::SteamLocomotive,
            16 => SoundEffect::Wave,
            17 => SoundEffect::RobotWalk,
            18 => SoundEffect::PassingPlane,
            19 => SoundEffect::Landing,
            _ => SoundEffect::AlienInvasion, // Default fallback for invalid values
        }
    }
}
