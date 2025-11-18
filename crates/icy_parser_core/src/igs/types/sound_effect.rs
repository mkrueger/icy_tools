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

impl Default for SoundEffect {
    fn default() -> Self {
        Self::AlienInvasion
    }
}

impl TryFrom<i32> for SoundEffect {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SoundEffect::AlienInvasion),
            1 => Ok(SoundEffect::RedAlert),
            2 => Ok(SoundEffect::Gunshot),
            3 => Ok(SoundEffect::Laser1),
            4 => Ok(SoundEffect::Jackhammer),
            5 => Ok(SoundEffect::Teleport),
            6 => Ok(SoundEffect::Explosion),
            7 => Ok(SoundEffect::Laser2),
            8 => Ok(SoundEffect::Longbell),
            9 => Ok(SoundEffect::Surprise),
            10 => Ok(SoundEffect::RadioBroadcast),
            11 => Ok(SoundEffect::BounceBall),
            12 => Ok(SoundEffect::EerieSound),
            13 => Ok(SoundEffect::HarleyMotorcycle),
            14 => Ok(SoundEffect::Helicopter),
            15 => Ok(SoundEffect::SteamLocomotive),
            16 => Ok(SoundEffect::Wave),
            17 => Ok(SoundEffect::RobotWalk),
            18 => Ok(SoundEffect::PassingPlane),
            19 => Ok(SoundEffect::Landing),
            _ => Err(format!("Invalid SoundEffect value: {}", value)),
        }
    }
}
