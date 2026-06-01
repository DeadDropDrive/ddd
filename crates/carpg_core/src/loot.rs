use crate::balance::BalanceConfig;
use crate::job::Job;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootRarity {
    Common,
    Uncommon,
    Rare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootEffect {
    SellOnly,
    VehicleUpgrade(UpgradeEffect),
    InstallCostVoucher(InstallCostVoucherEffect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeEffect {
    pub stealth_modifier: i16,
    pub drivetrain_modifier: i16,
    pub repair_discount_percent: u8,
    pub install_cost: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallCostVoucherEffect {
    pub discount_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootItem {
    pub name: String,
    pub rarity: LootRarity,
    pub black_market_value: i32,
    pub effect: LootEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlackMarketOffer {
    pub item: LootItem,
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

pub fn generate_loot(job: &Job, seed: u64, config: &BalanceConfig) -> Vec<LootItem> {
    let mut rng = SeededRng::new(seed);
    let drop_chance = config
        .loot_base_drop_chance
        .saturating_add(job.heat.saturating_mul(config.loot_job_heat_multiplier))
        .saturating_add(
            job.cargo_size
                .saturating_mul(config.loot_cargo_size_multiplier),
        )
        .min(config.loot_max_drop_chance);

    if rng.next_u8(100) >= drop_chance {
        return Vec::new();
    }

    let mut items = vec![primary_loot(job, rng.next_u8(100))];
    if rng.next_u8(100) < config.loot_bonus_drop_chance {
        items.push(bonus_loot(job, rng.next_u8(100)));
    }

    items
}

pub fn generate_black_market_offers(seed: u64, config: &BalanceConfig) -> Vec<BlackMarketOffer> {
    let catalog = black_market_upgrade_catalog();
    let mut rng = SeededRng::new(seed);
    let min_parts = config
        .black_market_min_parts
        .min(config.black_market_max_parts);
    let max_parts = config.black_market_max_parts.max(min_parts);
    let count_range = max_parts.saturating_sub(min_parts).saturating_add(1);
    let count = min_parts.saturating_add(rng.next_u8(count_range)).max(1) as usize;
    let mut offers = Vec::new();
    let mut attempts = 0;

    while offers.len() < count && attempts < count * 12 {
        attempts += 1;
        let rarity = roll_black_market_rarity(&mut rng, config);
        let candidates: Vec<_> = catalog
            .iter()
            .filter(|item| item.rarity == rarity)
            .cloned()
            .collect();
        if candidates.is_empty() {
            continue;
        }

        let item = candidates[rng.next_u8(candidates.len() as u8) as usize].clone();
        if offers
            .iter()
            .any(|offer: &BlackMarketOffer| offer.item.name == item.name)
        {
            continue;
        }

        offers.push(BlackMarketOffer {
            asking_price: black_market_price(&item, config),
            item,
        });
    }

    offers
}

pub fn black_market_upgrade_catalog() -> Vec<LootItem> {
    crate::generated_tables::black_market_upgrade_catalog()
}

fn primary_loot(job: &Job, roll: u8) -> LootItem {
    crate::generated_tables::primary_loot(job, roll)
}

fn bonus_loot(job: &Job, roll: u8) -> LootItem {
    crate::generated_tables::bonus_loot(job, roll)
}

fn roll_black_market_rarity(rng: &mut SeededRng, config: &BalanceConfig) -> LootRarity {
    let common = config.black_market_common_weight;
    let uncommon = config.black_market_uncommon_weight;
    let rare = config.black_market_rare_weight;
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

fn black_market_price(item: &LootItem, config: &BalanceConfig) -> i32 {
    let markup = match item.rarity {
        LootRarity::Common => config.black_market_common_markup_percent,
        LootRarity::Uncommon => config.black_market_uncommon_markup_percent,
        LootRarity::Rare => config.black_market_rare_markup_percent,
    };
    let price = item.black_market_value * markup as i32 / 100;
    price.max(item.black_market_value + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{CargoType, Terrain};

    #[test]
    fn generates_deterministic_loot_for_same_seed() {
        let job = Job::new(
            "Rural night drop",
            CargoType::Contraband,
            Terrain::Rural,
            2,
            650,
            5,
            5,
        );
        let config = BalanceConfig::default();

        let first = generate_loot(&job, 4, &config);
        let second = generate_loot(&job, 4, &config);

        assert_eq!(first, second);
    }

    #[test]
    fn can_generate_sellable_loot() {
        let job = Job::new(
            "Rural night drop",
            CargoType::Contraband,
            Terrain::Rural,
            2,
            650,
            5,
            5,
        );
        let config = BalanceConfig::default();

        let loot = generate_loot(&job, 4, &config);

        assert!(!loot.is_empty());
        assert!(loot.iter().all(|item| item.black_market_value > 0));
    }

    #[test]
    fn black_market_generates_two_or_three_vehicle_upgrades_above_sell_value() {
        let config = BalanceConfig::default();

        let offers = generate_black_market_offers(42, &config);

        assert!((2..=3).contains(&offers.len()));
        assert!(offers
            .iter()
            .all(|offer| matches!(offer.item.effect, LootEffect::VehicleUpgrade(_))));
        assert!(offers
            .iter()
            .all(|offer| offer.asking_price > offer.item.black_market_value));
    }

    #[test]
    fn black_market_generation_is_deterministic() {
        let config = BalanceConfig::default();

        let first = generate_black_market_offers(99, &config);
        let second = generate_black_market_offers(99, &config);

        assert_eq!(first, second);
    }
}
