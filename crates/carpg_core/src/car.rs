use crate::balance::BalanceConfig;
use crate::loot::{LootEffect, LootItem, LootRarity, UpgradeEffect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drivetrain {
    Fwd,
    Rwd,
    Awd,
    FourWd,
}

impl Drivetrain {
    pub fn handles_rough_terrain(self) -> bool {
        matches!(self, Self::Awd | Self::FourWd)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveStats {
    pub stealth: u8,
    pub drivetrain: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSpec {
    pub layout: String,
    pub displacement_liters: String,
    pub aspiration: String,
    pub fuel: String,
    pub horsepower: u16,
    pub horsepower_rpm: u16,
    pub torque_lb_ft: u16,
    pub torque_rpm: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleSpec {
    pub make: String,
    pub model: String,
    pub trim: String,
    pub body_style: String,
    pub transmission: String,
    pub curb_weight_lbs: u16,
    pub cargo_volume_cu_ft: String,
    pub fuel_capacity_gal: String,
    pub engine: EngineSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledUpgrade {
    pub name: String,
    pub rarity: LootRarity,
    pub black_market_value: i32,
    pub install_cost: i32,
    pub stealth_modifier: i16,
    pub drivetrain_modifier: i16,
    pub repair_discount_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarMarketOffer {
    pub car: Car,
    pub asking_price: i32,
}

#[derive(Debug, Clone, Copy)]
struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u8(&mut self, max_exclusive: u8) -> u8 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.state >> 32) as u8) % max_exclusive
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartCondition {
    pub engine: u8,
    pub suspension: u8,
    pub tires: u8,
    pub body: u8,
}

impl PartCondition {
    pub fn new() -> Self {
        Self {
            engine: 100,
            suspension: 100,
            tires: 100,
            body: 100,
        }
    }

    pub fn average(self) -> u8 {
        let total =
            self.engine as u16 + self.suspension as u16 + self.tires as u16 + self.body as u16;
        (total / 4) as u8
    }

    pub fn apply_damage(&mut self, damage: PartDamage) {
        self.engine = self.engine.saturating_sub(damage.engine);
        self.suspension = self.suspension.saturating_sub(damage.suspension);
        self.tires = self.tires.saturating_sub(damage.tires);
        self.body = self.body.saturating_sub(damage.body);
    }
}

impl Default for PartCondition {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PartDamage {
    pub engine: u8,
    pub suspension: u8,
    pub tires: u8,
    pub body: u8,
}

impl PartDamage {
    pub fn any(self) -> bool {
        self.engine > 0 || self.suspension > 0 || self.tires > 0 || self.body > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Car {
    pub name: String,
    pub rarity: LootRarity,
    pub model_year: u16,
    pub value: i32,
    pub cargo_capacity: u8,
    pub stealth: u8,
    pub drivetrain: Drivetrain,
    pub spec: VehicleSpec,
    pub condition: PartCondition,
    pub installed_upgrades: Vec<InstalledUpgrade>,
}

impl Car {
    pub fn new(
        name: impl Into<String>,
        rarity: LootRarity,
        model_year: u16,
        value: i32,
        cargo_capacity: u8,
        stealth: u8,
        drivetrain: Drivetrain,
        spec: VehicleSpec,
    ) -> Self {
        Self {
            name: name.into(),
            rarity,
            model_year,
            value,
            cargo_capacity,
            stealth,
            drivetrain,
            spec,
            condition: PartCondition::new(),
            installed_upgrades: Vec::new(),
        }
    }

    pub fn honda_civic_1998_dx_sedan() -> Self {
        crate::generated_tables::car_honda_civic_1998_dx_sedan()
    }

    pub fn volvo_v70_1999_turbo_awd() -> Self {
        crate::generated_tables::car_volvo_v70_1999_turbo_awd()
    }

    pub fn repair_cost(&self) -> i32 {
        let missing = 400
            - self.condition.engine as i32
            - self.condition.suspension as i32
            - self.condition.tires as i32
            - self.condition.body as i32;
        let base_cost = missing * 3;
        let discount = self.total_repair_discount_percent().min(80) as i32;
        base_cost * (100 - discount) / 100
    }

    pub fn repair_fully(&mut self) {
        self.condition = PartCondition::new();
    }

    pub fn effective_stats(&self) -> EffectiveStats {
        let body_penalty = missing_condition(self.condition.body) / 8;
        let engine_penalty = missing_condition(self.condition.engine) / 12;
        let stealth_modifier = self.total_stealth_modifier();
        let stealth = (self
            .stealth
            .saturating_sub(body_penalty.saturating_add(engine_penalty))
            as i16
            + stealth_modifier)
            .clamp(0, u8::MAX as i16) as u8;

        let drivetrain = drivetrain_base(self.drivetrain)
            - (missing_condition(self.condition.suspension) as i16 / 4)
            - (missing_condition(self.condition.tires) as i16 / 5)
            + self.total_drivetrain_modifier();

        EffectiveStats {
            stealth,
            drivetrain,
        }
    }

    pub fn install_loot(&mut self, loot: LootItem) -> bool {
        let effect = match loot.effect {
            LootEffect::SellOnly => return false,
            LootEffect::VehicleUpgrade(effect) => effect,
            LootEffect::InstallCostVoucher(_) => return false,
        };

        self.installed_upgrades.push(InstalledUpgrade::from_loot(
            loot.name,
            loot.rarity,
            loot.black_market_value,
            effect,
        ));
        true
    }

    pub fn uninstall_upgrade(&mut self, upgrade_index: usize) -> Option<LootItem> {
        let upgrade = self.installed_upgrades.get(upgrade_index)?.clone();
        self.installed_upgrades.remove(upgrade_index);
        Some(upgrade.into_loot_item())
    }

    fn total_stealth_modifier(&self) -> i16 {
        self.installed_upgrades
            .iter()
            .map(|upgrade| upgrade.stealth_modifier)
            .sum()
    }

    fn total_drivetrain_modifier(&self) -> i16 {
        self.installed_upgrades
            .iter()
            .map(|upgrade| upgrade.drivetrain_modifier)
            .sum()
    }

    fn total_repair_discount_percent(&self) -> u8 {
        self.installed_upgrades
            .iter()
            .map(|upgrade| upgrade.repair_discount_percent)
            .sum()
    }
}

pub fn generate_car_market_offers(seed: u64, config: &BalanceConfig) -> Vec<CarMarketOffer> {
    let catalog = crate::generated_tables::market_car_catalog();
    let mut rng = SeededRng::new(seed);
    let min_cars = config.car_market_min_cars.min(config.car_market_max_cars);
    let max_cars = config.car_market_max_cars.max(min_cars);
    let count_range = max_cars.saturating_sub(min_cars).saturating_add(1);
    let count = min_cars.saturating_add(rng.next_u8(count_range)).max(1) as usize;
    let mut offers = Vec::new();
    let mut attempts = 0;

    while offers.len() < count && attempts < count * 12 {
        attempts += 1;
        let rarity = roll_car_market_rarity(&mut rng, config);
        let candidates: Vec<_> = catalog
            .iter()
            .filter(|car| car.rarity == rarity)
            .cloned()
            .collect();
        if candidates.is_empty() {
            continue;
        }

        let car = candidates[rng.next_u8(candidates.len() as u8) as usize].clone();
        if offers
            .iter()
            .any(|offer: &CarMarketOffer| offer.car.name == car.name)
        {
            continue;
        }

        offers.push(CarMarketOffer {
            asking_price: car_market_price(&car, config),
            car,
        });
    }

    offers
}

fn roll_car_market_rarity(rng: &mut SeededRng, config: &BalanceConfig) -> LootRarity {
    let common = config.car_market_common_weight;
    let uncommon = config.car_market_uncommon_weight;
    let rare = config.car_market_rare_weight;
    let total = common.saturating_add(uncommon).saturating_add(rare).max(1);
    let roll = rng.next_u8(total);

    if roll < common {
        LootRarity::Common
    } else if roll < common.saturating_add(uncommon) {
        LootRarity::Uncommon
    } else {
        LootRarity::Rare
    }
}

fn car_market_price(car: &Car, config: &BalanceConfig) -> i32 {
    let markup = match car.rarity {
        LootRarity::Common => config.car_market_common_markup_percent,
        LootRarity::Uncommon => config.car_market_uncommon_markup_percent,
        LootRarity::Rare => config.car_market_rare_markup_percent,
    };
    let price = car.value * markup as i32 / 100;
    price.max(car.value + 1)
}

impl InstalledUpgrade {
    pub fn effect(&self) -> UpgradeEffect {
        UpgradeEffect {
            stealth_modifier: self.stealth_modifier,
            drivetrain_modifier: self.drivetrain_modifier,
            repair_discount_percent: self.repair_discount_percent,
            install_cost: self.install_cost,
        }
    }

    fn from_loot(
        name: String,
        rarity: LootRarity,
        black_market_value: i32,
        effect: UpgradeEffect,
    ) -> Self {
        Self {
            name,
            rarity,
            black_market_value,
            install_cost: effect.install_cost,
            stealth_modifier: effect.stealth_modifier,
            drivetrain_modifier: effect.drivetrain_modifier,
            repair_discount_percent: effect.repair_discount_percent,
        }
    }

    fn into_loot_item(self) -> LootItem {
        let effect = self.effect();
        LootItem {
            name: self.name,
            rarity: self.rarity,
            black_market_value: self.black_market_value,
            effect: LootEffect::VehicleUpgrade(effect),
        }
    }
}

fn missing_condition(value: u8) -> u8 {
    100_u8.saturating_sub(value)
}

fn drivetrain_base(drivetrain: Drivetrain) -> i16 {
    match drivetrain {
        Drivetrain::Fwd => 0,
        Drivetrain::Rwd => 0,
        Drivetrain::Awd => 12,
        Drivetrain::FourWd => 16,
    }
}
