use crate::balance::BalanceConfig;
use crate::car::{generate_car_market_offers, Car, CarMarketOffer};
use crate::job::Job;
use crate::loot::{generate_black_market_offers, generate_loot, BlackMarketOffer, LootItem};
use crate::route::{
    fork_choice_previews_with_config, patrol_action_previews_with_config,
    resolve_fork_choice_with_config, resolve_job_with_config, resolve_patrol_action_with_config,
    ForkChoice, ForkChoicePreview, ForkResolution, JobOutcome, PatrolAction, PatrolActionPreview,
    PatrolResolution,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Garage {
    pub cash: i32,
    pub heat: u8,
    pub cars: Vec<Car>,
    pub inventory: Vec<LootItem>,
    pub black_market: Vec<BlackMarketOffer>,
    pub car_market: Vec<CarMarketOffer>,
    black_market_seed: u64,
    car_market_seed: u64,
    pub balance: BalanceConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobReport {
    pub car_name: String,
    pub job_name: String,
    pub outcome: JobOutcome,
    pub loot: Vec<LootItem>,
}

impl Garage {
    pub fn new(cash: i32, cars: Vec<Car>) -> Self {
        let balance = BalanceConfig::default();
        Self {
            cash,
            heat: 0,
            cars,
            inventory: Vec::new(),
            black_market: generate_black_market_offers(101, &balance),
            car_market: generate_car_market_offers(303, &balance),
            black_market_seed: 154,
            car_market_seed: 356,
            balance,
        }
    }

    pub fn with_balance(cash: i32, cars: Vec<Car>, balance: BalanceConfig) -> Self {
        Self {
            cash,
            heat: 0,
            cars,
            inventory: Vec::new(),
            black_market: generate_black_market_offers(101, &balance),
            car_market: generate_car_market_offers(303, &balance),
            black_market_seed: 154,
            car_market_seed: 356,
            balance,
        }
    }

    pub fn starter() -> Self {
        Self::new(750, crate::generated_tables::starter_cars())
    }

    pub fn run_job(&mut self, car_index: usize, job: &Job, seed: u64) -> Option<JobReport> {
        let car = self.cars.get_mut(car_index)?;
        let car_name = car.name.clone();
        let outcome = resolve_job_with_config(car, job, seed, &self.balance);
        let loot = if outcome.completed {
            generate_loot(job, seed.wrapping_add(211), &self.balance)
        } else {
            Vec::new()
        };

        self.cash += outcome.payout;
        self.heat = self.heat.saturating_add(outcome.heat_gained);
        self.inventory.extend(loot.clone());

        Some(JobReport {
            car_name,
            job_name: job.name.clone(),
            outcome,
            loot,
        })
    }

    pub fn resolve_patrol_encounter(
        &mut self,
        car_index: usize,
        job: &Job,
        action: PatrolAction,
        seed: u64,
    ) -> Option<PatrolResolution> {
        let car = self.cars.get_mut(car_index)?;
        let resolution =
            resolve_patrol_action_with_config(car, job, self.heat, action, seed, &self.balance);
        self.heat = self.heat.saturating_add(resolution.heat_gained);
        Some(resolution)
    }

    pub fn patrol_previews(&self, car_index: usize, job: &Job) -> Option<Vec<PatrolActionPreview>> {
        let car = self.cars.get(car_index)?;
        Some(patrol_action_previews_with_config(
            car,
            job,
            self.heat,
            &self.balance,
        ))
    }

    pub fn resolve_fork_encounter(
        &mut self,
        car_index: usize,
        job: &Job,
        choice: ForkChoice,
        seed: u64,
    ) -> Option<ForkResolution> {
        let car = self.cars.get_mut(car_index)?;
        let resolution =
            resolve_fork_choice_with_config(car, job, self.heat, choice, seed, &self.balance);
        self.heat = self.heat.saturating_add(resolution.heat_gained);
        Some(resolution)
    }

    pub fn fork_previews(&self, car_index: usize, job: &Job) -> Option<Vec<ForkChoicePreview>> {
        let car = self.cars.get(car_index)?;
        Some(fork_choice_previews_with_config(
            car,
            job,
            self.heat,
            &self.balance,
        ))
    }

    pub fn repair_car(&mut self, car_index: usize) -> bool {
        let Some(car) = self.cars.get_mut(car_index) else {
            return false;
        };

        let cost = car.repair_cost();
        if cost == 0 || self.cash < cost {
            return false;
        }

        self.cash -= cost;
        car.repair_fully();
        true
    }

    pub fn sell_inventory(&mut self) -> i32 {
        let total = self
            .inventory
            .iter()
            .map(|item| item.black_market_value)
            .sum();
        self.inventory.clear();
        self.cash += total;
        total
    }

    pub fn sell_inventory_item(&mut self, item_index: usize) -> Option<i32> {
        if item_index >= self.inventory.len() {
            return None;
        }

        let item = self.inventory.remove(item_index);
        self.cash += item.black_market_value;
        Some(item.black_market_value)
    }

    pub fn install_inventory_item(&mut self, item_index: usize, car_index: usize) -> bool {
        if item_index >= self.inventory.len() {
            return false;
        }

        let base_install_cost = match self.inventory[item_index].effect {
            crate::loot::LootEffect::SellOnly => return false,
            crate::loot::LootEffect::VehicleUpgrade(effect) => effect.install_cost,
            crate::loot::LootEffect::InstallCostVoucher(_) => return false,
        };
        let voucher_index = self.install_cost_voucher_index(item_index);
        let install_cost = if let Some(voucher_index) = voucher_index {
            let crate::loot::LootEffect::InstallCostVoucher(voucher) =
                self.inventory[voucher_index].effect
            else {
                unreachable!();
            };
            discounted_install_cost(base_install_cost, voucher.discount_percent)
        } else {
            base_install_cost
        };
        if self.cash < install_cost {
            return false;
        }

        let Some(car) = self.cars.get_mut(car_index) else {
            return false;
        };

        let item = self.inventory.remove(item_index);
        if car.install_loot(item.clone()) {
            self.cash -= install_cost;
            if let Some(voucher_index) = voucher_index {
                let adjusted_index = if voucher_index > item_index {
                    voucher_index - 1
                } else {
                    voucher_index
                };
                self.inventory.remove(adjusted_index);
            }
            true
        } else {
            self.inventory.insert(item_index, item);
            false
        }
    }

    pub fn uninstall_upgrade(&mut self, car_index: usize, upgrade_index: usize) -> bool {
        let Some(car) = self.cars.get_mut(car_index) else {
            return false;
        };
        let Some(item) = car.uninstall_upgrade(upgrade_index) else {
            return false;
        };
        self.inventory.push(item);
        true
    }

    pub fn buy_black_market_part(&mut self, offer_index: usize) -> bool {
        let Some(offer) = self.black_market.get(offer_index) else {
            return false;
        };
        if self.cash < offer.asking_price {
            return false;
        }

        let offer = self.black_market.remove(offer_index);
        self.cash -= offer.asking_price;
        self.inventory.push(offer.item);
        true
    }

    pub fn buy_market_car(&mut self, offer_index: usize) -> bool {
        let Some(offer) = self.car_market.get(offer_index) else {
            return false;
        };
        if self.cash < offer.asking_price {
            return false;
        }

        let offer = self.car_market.remove(offer_index);
        self.cash -= offer.asking_price;
        self.cars.push(offer.car);
        true
    }

    pub fn sell_car(&mut self, car_index: usize) -> Option<i32> {
        if self.cars.len() <= 1 || car_index >= self.cars.len() {
            return None;
        }

        let sale_value = self.car_sale_value(car_index)?;
        self.cars.remove(car_index);
        self.cash += sale_value;
        Some(sale_value)
    }

    pub fn car_sale_value(&self, car_index: usize) -> Option<i32> {
        self.cars
            .get(car_index)
            .map(|car| self.estimated_car_sale_value(car))
    }

    pub fn estimated_car_sale_value(&self, car: &Car) -> i32 {
        let upgrade_value = car
            .installed_upgrades
            .iter()
            .map(|upgrade| upgrade.black_market_value + upgrade.install_cost)
            .sum::<i32>()
            / 2;
        let pre_discount_value = car.value + upgrade_value;
        let condition_percent = car.condition.average().max(10) as i32;
        let heat_percent = (100 - self.heat as i32 * 10).clamp(25, 100);

        (pre_discount_value * condition_percent * heat_percent / 10_000).max(1)
    }

    pub fn refresh_black_market(&mut self) {
        self.black_market = generate_black_market_offers(self.black_market_seed, &self.balance);
        self.black_market_seed = self.black_market_seed.wrapping_add(53);
    }

    pub fn refresh_car_market(&mut self) {
        self.car_market = generate_car_market_offers(self.car_market_seed, &self.balance);
        self.car_market_seed = self.car_market_seed.wrapping_add(71);
    }

    fn install_cost_voucher_index(&self, installing_item_index: usize) -> Option<usize> {
        self.inventory
            .iter()
            .enumerate()
            .find(|(index, item)| {
                *index != installing_item_index
                    && matches!(item.effect, crate::loot::LootEffect::InstallCostVoucher(_))
            })
            .map(|(index, _)| index)
    }
}

fn discounted_install_cost(base_cost: i32, discount_percent: u8) -> i32 {
    base_cost * (100 - discount_percent.min(100) as i32) / 100
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::car::PartDamage;

    #[test]
    fn repair_car_spends_cash_and_restores_condition() {
        let mut garage = Garage::starter();
        garage.cars[0].condition.apply_damage(PartDamage {
            suspension: 10,
            ..PartDamage::default()
        });

        let repaired = garage.repair_car(0);

        assert!(repaired);
        assert_eq!(garage.cash, 720);
        assert_eq!(garage.cars[0].condition.suspension, 100);
    }

    #[test]
    fn sell_inventory_adds_black_market_value_to_cash() {
        let mut garage = Garage::starter();
        garage.inventory.push(LootItem {
            name: "Test loot".to_string(),
            rarity: crate::loot::LootRarity::Common,
            black_market_value: 123,
            effect: crate::loot::LootEffect::SellOnly,
        });

        let sold_for = garage.sell_inventory();

        assert_eq!(sold_for, 123);
        assert_eq!(garage.cash, 873);
        assert!(garage.inventory.is_empty());
    }

    #[test]
    fn sell_inventory_item_sells_one_item() {
        let mut garage = Garage::starter();
        garage.inventory.push(LootItem {
            name: "First loot".to_string(),
            rarity: crate::loot::LootRarity::Common,
            black_market_value: 123,
            effect: crate::loot::LootEffect::SellOnly,
        });
        garage.inventory.push(LootItem {
            name: "Second loot".to_string(),
            rarity: crate::loot::LootRarity::Common,
            black_market_value: 77,
            effect: crate::loot::LootEffect::SellOnly,
        });

        let sold_for = garage.sell_inventory_item(0);

        assert_eq!(sold_for, Some(123));
        assert_eq!(garage.cash, 873);
        assert_eq!(garage.inventory.len(), 1);
        assert_eq!(garage.inventory[0].name, "Second loot");
    }

    #[test]
    fn install_inventory_item_applies_upgrade_and_removes_item() {
        let mut garage = Garage::starter();
        garage.inventory.push(LootItem {
            name: "Forged plate kit".to_string(),
            rarity: crate::loot::LootRarity::Uncommon,
            black_market_value: 280,
            effect: crate::loot::LootEffect::VehicleUpgrade(crate::loot::UpgradeEffect {
                stealth_modifier: 2,
                drivetrain_modifier: -1,
                repair_discount_percent: 0,
                install_cost: 170,
            }),
        });

        let installed = garage.install_inventory_item(0, 0);

        assert!(installed);
        assert!(garage.inventory.is_empty());
        assert_eq!(garage.cash, 580);
        assert_eq!(garage.cars[0].effective_stats().stealth, 10);
        assert_eq!(garage.cars[0].effective_stats().drivetrain, -1);
    }

    #[test]
    fn install_inventory_item_requires_cash_for_install_cost() {
        let mut garage = Garage::starter();
        garage.cash = 100;
        garage.inventory.push(LootItem {
            name: "Hidden compartment liner".to_string(),
            rarity: crate::loot::LootRarity::Rare,
            black_market_value: 520,
            effect: crate::loot::LootEffect::VehicleUpgrade(crate::loot::UpgradeEffect {
                stealth_modifier: 3,
                drivetrain_modifier: -2,
                repair_discount_percent: 0,
                install_cost: 360,
            }),
        });

        let installed = garage.install_inventory_item(0, 0);

        assert!(!installed);
        assert_eq!(garage.cash, 100);
        assert_eq!(garage.inventory.len(), 1);
        assert!(garage.cars[0].installed_upgrades.is_empty());
    }

    #[test]
    fn install_cost_voucher_is_consumed_to_discount_upgrade_install() {
        let mut garage = Garage::starter();
        garage.inventory.push(LootItem {
            name: "Contact favor voucher".to_string(),
            rarity: crate::loot::LootRarity::Uncommon,
            black_market_value: 160,
            effect: crate::loot::LootEffect::InstallCostVoucher(
                crate::loot::InstallCostVoucherEffect {
                    discount_percent: 100,
                },
            ),
        });
        garage.inventory.push(LootItem {
            name: "Forged ECU module".to_string(),
            rarity: crate::loot::LootRarity::Rare,
            black_market_value: 420,
            effect: crate::loot::LootEffect::VehicleUpgrade(crate::loot::UpgradeEffect {
                stealth_modifier: -2,
                drivetrain_modifier: 4,
                repair_discount_percent: 0,
                install_cost: 320,
            }),
        });

        let installed = garage.install_inventory_item(1, 0);

        assert!(installed);
        assert_eq!(garage.cash, 750);
        assert!(garage.inventory.is_empty());
        assert_eq!(garage.cars[0].installed_upgrades.len(), 1);
        assert_eq!(garage.cars[0].effective_stats().drivetrain, 4);
    }

    #[test]
    fn uninstall_upgrade_moves_it_back_to_inventory_without_refund() {
        let mut garage = Garage::starter();
        garage.inventory.push(LootItem {
            name: "Salvaged all-terrain tires".to_string(),
            rarity: crate::loot::LootRarity::Uncommon,
            black_market_value: 180,
            effect: crate::loot::LootEffect::VehicleUpgrade(crate::loot::UpgradeEffect {
                stealth_modifier: -1,
                drivetrain_modifier: 2,
                repair_discount_percent: 0,
                install_cost: 150,
            }),
        });
        assert!(garage.install_inventory_item(0, 0));

        let uninstalled = garage.uninstall_upgrade(0, 0);

        assert!(uninstalled);
        assert_eq!(garage.cash, 600);
        assert_eq!(garage.inventory.len(), 1);
        assert!(garage.cars[0].installed_upgrades.is_empty());
    }

    #[test]
    fn sell_only_inventory_item_cannot_be_installed() {
        let mut garage = Garage::starter();
        garage.inventory.push(LootItem {
            name: "Unmarked packet".to_string(),
            rarity: crate::loot::LootRarity::Common,
            black_market_value: 90,
            effect: crate::loot::LootEffect::SellOnly,
        });

        let installed = garage.install_inventory_item(0, 0);

        assert!(!installed);
        assert_eq!(garage.inventory.len(), 1);
        assert!(garage.cars[0].installed_upgrades.is_empty());
    }

    #[test]
    fn starter_garage_has_limited_black_market_stock() {
        let garage = Garage::starter();

        assert!((2..=3).contains(&garage.black_market.len()));
        assert!(garage
            .black_market
            .iter()
            .all(|offer| offer.asking_price > offer.item.black_market_value));
    }

    #[test]
    fn car_catalog_flags_drive_starter_and_market_lists() {
        let starters = crate::starter_car_catalog();
        let market = crate::market_car_catalog();
        let garage = Garage::starter();

        assert!(!starters.is_empty());
        assert!(!market.is_empty());
        assert_eq!(garage.cars, starters);
    }

    #[test]
    fn buying_black_market_part_spends_cash_and_adds_inventory() {
        let mut garage = Garage::starter();
        let offer = garage.black_market[0].clone();
        let starting_stock = garage.black_market.len();

        let bought = garage.buy_black_market_part(0);

        assert!(bought);
        assert_eq!(garage.cash, 750 - offer.asking_price);
        assert_eq!(garage.inventory.last(), Some(&offer.item));
        assert_eq!(garage.black_market.len(), starting_stock - 1);
    }

    #[test]
    fn starter_garage_has_car_market_stock_above_sell_value() {
        let garage = Garage::starter();

        assert!((1..=3).contains(&garage.car_market.len()));
        assert!(garage
            .car_market
            .iter()
            .all(|offer| offer.asking_price > offer.car.value));
    }

    #[test]
    fn buying_market_car_spends_cash_and_adds_car() {
        let mut garage = Garage::starter();
        garage.cash = 10_000;
        let offer = garage.car_market[0].clone();
        let starting_stock = garage.car_market.len();
        let starting_cars = garage.cars.len();

        let bought = garage.buy_market_car(0);

        assert!(bought);
        assert_eq!(garage.cash, 10_000 - offer.asking_price);
        assert_eq!(garage.cars.last(), Some(&offer.car));
        assert_eq!(garage.car_market.len(), starting_stock - 1);
        assert_eq!(garage.cars.len(), starting_cars + 1);
    }

    #[test]
    fn selling_car_requires_a_spare_car() {
        let mut garage = Garage::new(750, vec![Car::honda_civic_1998_dx_sedan()]);

        let sold_for = garage.sell_car(0);

        assert_eq!(sold_for, None);
        assert_eq!(garage.cars.len(), 1);
    }

    #[test]
    fn car_sale_value_drops_with_damage_and_heat() {
        let clean_garage = Garage::new(
            750,
            vec![
                Car::honda_civic_1998_dx_sedan(),
                Car::volvo_v70_1999_turbo_awd(),
            ],
        );
        let clean_value = clean_garage.car_sale_value(1).unwrap();
        let mut hot_damaged_garage = clean_garage.clone();
        hot_damaged_garage.heat = 6;
        hot_damaged_garage.cars[1]
            .condition
            .apply_damage(PartDamage {
                engine: 40,
                suspension: 40,
                tires: 40,
                body: 40,
            });

        let hot_damaged_value = hot_damaged_garage.car_sale_value(1).unwrap();

        assert!(hot_damaged_value < clean_value / 2);
    }

    #[test]
    fn car_sale_value_includes_half_upgrade_value_and_install_cost() {
        let mut garage = Garage::new(
            750,
            vec![
                Car::honda_civic_1998_dx_sedan(),
                Car::volvo_v70_1999_turbo_awd(),
            ],
        );
        garage.inventory.push(LootItem {
            name: "Hidden compartment liner".to_string(),
            rarity: crate::loot::LootRarity::Rare,
            black_market_value: 520,
            effect: crate::loot::LootEffect::VehicleUpgrade(crate::loot::UpgradeEffect {
                stealth_modifier: 3,
                drivetrain_modifier: -2,
                repair_discount_percent: 0,
                install_cost: 360,
            }),
        });
        let base_value = garage.car_sale_value(1).unwrap();

        assert!(garage.install_inventory_item(0, 1));

        assert_eq!(garage.car_sale_value(1), Some(base_value + 440));
    }

    #[test]
    fn black_market_purchase_requires_cash() {
        let mut garage = Garage::starter();
        garage.cash = 0;
        let starting_stock = garage.black_market.len();

        let bought = garage.buy_black_market_part(0);

        assert!(!bought);
        assert_eq!(garage.cash, 0);
        assert!(garage.inventory.is_empty());
        assert_eq!(garage.black_market.len(), starting_stock);
    }
}
