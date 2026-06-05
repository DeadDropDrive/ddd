#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoType {
    Documents,
    Parts,
    Contraband,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terrain {
    City,
    Industrial,
    Rural,
    Mountain,
}

impl Terrain {
    pub fn roughness(self) -> u8 {
        match self {
            Self::City => 1,
            Self::Industrial => 3,
            Self::Rural => 5,
            Self::Mountain => 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub name: String,
    pub cargo: CargoType,
    pub terrain: Terrain,
    pub cargo_size: u8,
    pub payout: i32,
    pub heat: u8,
    pub distance: u8,
}

impl Job {
    pub fn new(
        name: impl Into<String>,
        cargo: CargoType,
        terrain: Terrain,
        cargo_size: u8,
        payout: i32,
        heat: u8,
        distance: u8,
    ) -> Self {
        Self {
            name: name.into(),
            cargo,
            terrain,
            cargo_size,
            payout,
            heat,
            distance,
        }
    }
}
