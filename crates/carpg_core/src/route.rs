use crate::balance::BalanceConfig;
use crate::car::{Car, PartDamage};
use crate::job::{Job, Terrain};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatrolAction {
    BlendIn,
    Detour,
    PushSpeed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkChoice {
    Highway,
    ServiceRoad,
    Backtrack,
}

impl ForkChoice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Highway => "Take the highway",
            Self::ServiceRoad => "Take the service road",
            Self::Backtrack => "Backtrack",
        }
    }
}

impl PatrolAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::BlendIn => "Blend in",
            Self::Detour => "Detour through service roads",
            Self::PushSpeed => "Push speed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatrolActionPreview {
    pub action: PatrolAction,
    pub check: &'static str,
    pub chance: u8,
    pub modifiers: Vec<CheckModifier>,
    pub success: &'static str,
    pub failure: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckModifier {
    pub label: &'static str,
    pub value: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatrolResolution {
    pub action: PatrolAction,
    pub chance: u8,
    pub roll: u8,
    pub success: bool,
    pub heat_gained: u8,
    pub damage: PartDamage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkChoicePreview {
    pub choice: ForkChoice,
    pub check: &'static str,
    pub chance: Option<u8>,
    pub modifiers: Vec<CheckModifier>,
    pub success: &'static str,
    pub failure: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkResolution {
    pub choice: ForkChoice,
    pub chance: Option<u8>,
    pub roll: Option<u8>,
    pub success: bool,
    pub heat_gained: u8,
    pub damage: PartDamage,
    pub payout_multiplier_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteEvent {
    SmoothSegment,
    RoughRoad { damage: PartDamage },
    PatrolCloseCall,
    Backtracked,
    CaughtByCops,
    CargoTooLarge,
    Breakdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutcome {
    pub completed: bool,
    pub payout: i32,
    pub heat_gained: u8,
    pub events: Vec<RouteEvent>,
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

    fn d100(&mut self) -> u8 {
        self.next_u8(100) + 1
    }
}

pub fn patrol_action_previews(car: &Car, job: &Job, current_heat: u8) -> Vec<PatrolActionPreview> {
    patrol_action_previews_with_config(car, job, current_heat, &BalanceConfig::default())
}

pub fn patrol_action_previews_with_config(
    car: &Car,
    job: &Job,
    current_heat: u8,
    config: &BalanceConfig,
) -> Vec<PatrolActionPreview> {
    vec![
        preview_blend_in(car, job, current_heat, config),
        preview_detour(car, job, current_heat, config),
        preview_push_speed(car, job, current_heat, config),
    ]
}

pub fn resolve_patrol_action(
    car: &mut Car,
    job: &Job,
    current_heat: u8,
    action: PatrolAction,
    seed: u64,
) -> PatrolResolution {
    resolve_patrol_action_with_config(
        car,
        job,
        current_heat,
        action,
        seed,
        &BalanceConfig::default(),
    )
}

pub fn resolve_patrol_action_with_config(
    car: &mut Car,
    job: &Job,
    current_heat: u8,
    action: PatrolAction,
    seed: u64,
    config: &BalanceConfig,
) -> PatrolResolution {
    let preview = match action {
        PatrolAction::BlendIn => preview_blend_in(car, job, current_heat, config),
        PatrolAction::Detour => preview_detour(car, job, current_heat, config),
        PatrolAction::PushSpeed => preview_push_speed(car, job, current_heat, config),
    };

    let mut rng = SeededRng::new(seed);
    let roll = rng.d100();
    let success = roll <= preview.chance;
    let mut damage = PartDamage::default();
    let mut heat_gained = 0;

    if !success {
        match action {
            PatrolAction::BlendIn => {
                heat_gained = config.patrol_blend_failure_heat;
            }
            PatrolAction::Detour => {
                damage = PartDamage {
                    suspension: job.terrain.roughness()
                        + config.patrol_detour_failure_suspension_extra,
                    tires: job.terrain.roughness() + config.patrol_detour_failure_tires_extra,
                    ..PartDamage::default()
                };
            }
            PatrolAction::PushSpeed => {
                heat_gained = config.patrol_push_failure_heat;
                damage = PartDamage {
                    engine: config.patrol_push_failure_engine_damage,
                    tires: config.patrol_push_failure_tire_damage,
                    ..PartDamage::default()
                };
            }
        }
    }

    car.condition.apply_damage(damage);

    PatrolResolution {
        action,
        chance: preview.chance,
        roll,
        success,
        heat_gained,
        damage,
    }
}

pub fn fork_choice_previews(car: &Car, job: &Job, current_heat: u8) -> Vec<ForkChoicePreview> {
    fork_choice_previews_with_config(car, job, current_heat, &BalanceConfig::default())
}

pub fn fork_choice_previews_with_config(
    car: &Car,
    job: &Job,
    current_heat: u8,
    config: &BalanceConfig,
) -> Vec<ForkChoicePreview> {
    vec![
        preview_highway(car, job, current_heat, config),
        preview_service_road(car, job, current_heat, config),
        preview_backtrack(config),
    ]
}

pub fn resolve_fork_choice(
    car: &mut Car,
    job: &Job,
    current_heat: u8,
    choice: ForkChoice,
    seed: u64,
) -> ForkResolution {
    resolve_fork_choice_with_config(
        car,
        job,
        current_heat,
        choice,
        seed,
        &BalanceConfig::default(),
    )
}

pub fn resolve_fork_choice_with_config(
    car: &mut Car,
    job: &Job,
    current_heat: u8,
    choice: ForkChoice,
    seed: u64,
    config: &BalanceConfig,
) -> ForkResolution {
    let preview = match choice {
        ForkChoice::Highway => preview_highway(car, job, current_heat, config),
        ForkChoice::ServiceRoad => preview_service_road(car, job, current_heat, config),
        ForkChoice::Backtrack => preview_backtrack(config),
    };

    if choice == ForkChoice::Backtrack {
        return ForkResolution {
            choice,
            chance: None,
            roll: None,
            success: true,
            heat_gained: 0,
            damage: PartDamage::default(),
            payout_multiplier_percent: config.fork_backtrack_payout_multiplier_percent,
        };
    }

    let chance = preview.chance.expect("risky fork choices should have odds");
    let mut rng = SeededRng::new(seed);
    let roll = rng.d100();
    let success = roll <= chance;
    let mut damage = PartDamage::default();
    let mut heat_gained = 0;

    if !success {
        match choice {
            ForkChoice::Highway => {
                heat_gained = config.fork_highway_failure_heat;
            }
            ForkChoice::ServiceRoad => {
                damage = PartDamage {
                    suspension: job.terrain.roughness()
                        + config.fork_service_failure_suspension_extra,
                    tires: job.terrain.roughness() + config.fork_service_failure_tires_extra,
                    body: config.fork_service_failure_body_damage,
                    ..PartDamage::default()
                };
            }
            ForkChoice::Backtrack => {}
        }
    }

    car.condition.apply_damage(damage);

    ForkResolution {
        choice,
        chance: Some(chance),
        roll: Some(roll),
        success,
        heat_gained,
        damage,
        payout_multiplier_percent: 100,
    }
}

pub fn resolve_job(car: &mut Car, job: &Job, seed: u64) -> JobOutcome {
    resolve_job_with_config(car, job, seed, &BalanceConfig::default())
}

pub fn resolve_job_with_config(
    car: &mut Car,
    job: &Job,
    seed: u64,
    config: &BalanceConfig,
) -> JobOutcome {
    if job.cargo_size > car.cargo_capacity {
        return JobOutcome {
            completed: false,
            payout: 0,
            heat_gained: config.cargo_too_large_heat,
            events: vec![RouteEvent::CargoTooLarge],
        };
    }

    let mut rng = SeededRng::new(seed);
    let mut events = Vec::new();
    let mut heat_gained = 0;

    for _ in 0..job.distance {
        let stress = segment_stress(car, job, config);
        let roll = rng.next_u8(100);

        if roll < stress {
            let damage = damage_for(job.terrain, roll, config);
            car.condition.apply_damage(damage);
            events.push(RouteEvent::RoughRoad { damage });
        } else {
            events.push(RouteEvent::SmoothSegment);
        }

        let patrol_roll = rng.next_u8(100);
        let detection_risk = job
            .heat
            .saturating_mul(config.patrol_detection_job_heat_multiplier)
            .saturating_sub(car.effective_stats().stealth);
        if patrol_roll < detection_risk {
            heat_gained += config.patrol_close_call_heat;
            events.push(RouteEvent::PatrolCloseCall);
        }

        if car.condition.average() < config.breakdown_condition_threshold {
            events.push(RouteEvent::Breakdown);
            return JobOutcome {
                completed: false,
                payout: job.payout / config.breakdown_payout_divisor,
                heat_gained: heat_gained + config.breakdown_heat,
                events,
            };
        }
    }

    JobOutcome {
        completed: true,
        payout: job.payout,
        heat_gained,
        events,
    }
}

fn preview_blend_in(
    car: &Car,
    job: &Job,
    current_heat: u8,
    config: &BalanceConfig,
) -> PatrolActionPreview {
    let effective = car.effective_stats();
    let modifiers = vec![
        CheckModifier {
            label: "base",
            value: config.patrol_blend_base,
        },
        CheckModifier {
            label: "effective stealth",
            value: effective.stealth as i16 * config.patrol_blend_stealth_multiplier,
        },
        CheckModifier {
            label: "job heat",
            value: -(job.heat as i16 * config.patrol_blend_job_heat_multiplier),
        },
        CheckModifier {
            label: "garage heat",
            value: -(current_heat as i16 * config.patrol_blend_garage_heat_multiplier),
        },
        CheckModifier {
            label: "cargo size",
            value: -(job.cargo_size as i16 * config.patrol_blend_cargo_size_multiplier),
        },
    ];

    PatrolActionPreview {
        action: PatrolAction::BlendIn,
        check: "Stealth",
        chance: chance_from(&modifiers, config),
        modifiers,
        success: "patrol loses interest",
        failure: "+2 heat",
    }
}

fn preview_detour(
    car: &Car,
    job: &Job,
    current_heat: u8,
    config: &BalanceConfig,
) -> PatrolActionPreview {
    let effective = car.effective_stats();
    let modifiers = vec![
        CheckModifier {
            label: "base",
            value: config.patrol_detour_base,
        },
        CheckModifier {
            label: "effective drivetrain",
            value: effective.drivetrain,
        },
        CheckModifier {
            label: "terrain",
            value: -(job.terrain.roughness() as i16 * config.patrol_detour_terrain_multiplier),
        },
        CheckModifier {
            label: "condition",
            value: car.condition.average() as i16 / config.patrol_detour_condition_divisor,
        },
        CheckModifier {
            label: "garage heat",
            value: -(current_heat as i16 * config.patrol_detour_garage_heat_multiplier),
        },
    ];

    PatrolActionPreview {
        action: PatrolAction::Detour,
        check: "Terrain / suspension",
        chance: chance_from(&modifiers, config),
        modifiers,
        success: "avoid patrol through side roads",
        failure: "suspension and tire damage",
    }
}

fn preview_push_speed(
    car: &Car,
    job: &Job,
    current_heat: u8,
    config: &BalanceConfig,
) -> PatrolActionPreview {
    let modifiers = vec![
        CheckModifier {
            label: "base",
            value: config.patrol_push_base,
        },
        CheckModifier {
            label: "horsepower",
            value: car.spec.engine.horsepower as i16 / config.patrol_push_horsepower_divisor,
        },
        CheckModifier {
            label: "condition",
            value: car.condition.average() as i16 / config.patrol_push_condition_divisor,
        },
        CheckModifier {
            label: "terrain",
            value: -(job.terrain.roughness() as i16 * config.patrol_push_terrain_multiplier),
        },
        CheckModifier {
            label: "job heat",
            value: -(job.heat as i16 * config.patrol_push_job_heat_multiplier),
        },
        CheckModifier {
            label: "garage heat",
            value: -(current_heat as i16 * config.patrol_push_garage_heat_multiplier),
        },
    ];

    PatrolActionPreview {
        action: PatrolAction::PushSpeed,
        check: "Power / tires / engine",
        chance: chance_from(&modifiers, config),
        modifiers,
        success: "escape immediately",
        failure: "engine damage, tire damage, +1 heat",
    }
}

fn preview_highway(
    car: &Car,
    job: &Job,
    current_heat: u8,
    config: &BalanceConfig,
) -> ForkChoicePreview {
    let effective = car.effective_stats();
    let modifiers = vec![
        CheckModifier {
            label: "base",
            value: config.fork_highway_base,
        },
        CheckModifier {
            label: "effective stealth",
            value: effective.stealth as i16 * config.fork_highway_stealth_multiplier,
        },
        CheckModifier {
            label: "job heat",
            value: -(job.heat as i16 * config.fork_highway_job_heat_multiplier),
        },
        CheckModifier {
            label: "garage heat",
            value: -(current_heat as i16 * config.fork_highway_garage_heat_multiplier),
        },
        CheckModifier {
            label: "cargo size",
            value: -(job.cargo_size as i16 * config.fork_highway_cargo_size_multiplier),
        },
    ];

    ForkChoicePreview {
        choice: ForkChoice::Highway,
        check: "Stealth / heat",
        chance: Some(chance_from(&modifiers, config)),
        modifiers,
        success: "fast route, no extra wear",
        failure: "+2 heat from highway patrol attention",
    }
}

fn preview_service_road(
    car: &Car,
    job: &Job,
    current_heat: u8,
    config: &BalanceConfig,
) -> ForkChoicePreview {
    let effective = car.effective_stats();
    let modifiers = vec![
        CheckModifier {
            label: "base",
            value: config.fork_service_base,
        },
        CheckModifier {
            label: "effective drivetrain",
            value: effective.drivetrain,
        },
        CheckModifier {
            label: "terrain",
            value: -(job.terrain.roughness() as i16 * config.fork_service_terrain_multiplier),
        },
        CheckModifier {
            label: "condition",
            value: car.condition.average() as i16 / config.fork_service_condition_divisor,
        },
        CheckModifier {
            label: "garage heat",
            value: -(current_heat as i16 * config.fork_service_garage_heat_multiplier),
        },
    ];

    ForkChoicePreview {
        choice: ForkChoice::ServiceRoad,
        check: "Drivetrain / terrain",
        chance: Some(chance_from(&modifiers, config)),
        modifiers,
        success: "avoid patrol pressure through rougher roads",
        failure: "suspension, tire, and body damage",
    }
}

fn preview_backtrack(config: &BalanceConfig) -> ForkChoicePreview {
    ForkChoicePreview {
        choice: ForkChoice::Backtrack,
        check: "No roll",
        chance: None,
        modifiers: Vec::new(),
        success: if config.fork_backtrack_payout_multiplier_percent == 80 {
            "safe route, payout reduced by 20%"
        } else {
            "safe route, payout reduced"
        },
        failure: "none",
    }
}

fn chance_from(modifiers: &[CheckModifier], config: &BalanceConfig) -> u8 {
    modifiers
        .iter()
        .map(|modifier| modifier.value)
        .sum::<i16>()
        .clamp(config.chance_min as i16, config.chance_max as i16) as u8
}

fn segment_stress(car: &Car, job: &Job, config: &BalanceConfig) -> u8 {
    let mut stress = job.terrain.roughness() * config.segment_terrain_stress_multiplier;

    if matches!(job.terrain, Terrain::Rural | Terrain::Mountain)
        && !car.drivetrain.handles_rough_terrain()
    {
        stress += config.segment_rough_terrain_2wd_penalty;
    }

    if car.condition.average() < config.segment_low_condition_threshold {
        stress += config.segment_low_condition_penalty;
    }

    stress.min(config.segment_stress_cap)
}

fn damage_for(terrain: Terrain, roll: u8, config: &BalanceConfig) -> PartDamage {
    let base = terrain.roughness();
    match roll % 4 {
        0 => PartDamage {
            tires: base + config.damage_tire_extra,
            ..PartDamage::default()
        },
        1 => PartDamage {
            suspension: base + config.damage_suspension_extra,
            ..PartDamage::default()
        },
        2 => PartDamage {
            engine: base,
            ..PartDamage::default()
        },
        _ => PartDamage {
            body: base + config.damage_body_extra,
            ..PartDamage::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::car::{Car, Drivetrain};
    use crate::job::{CargoType, Job};

    #[test]
    fn rejects_jobs_that_exceed_cargo_capacity() {
        let mut car = Car::honda_civic_1998_dx_sedan();
        let job = Job::new(
            "Oversized crate",
            CargoType::Parts,
            Terrain::City,
            9,
            500,
            1,
            2,
        );

        let outcome = resolve_job(&mut car, &job, 7);

        assert!(!outcome.completed);
        assert_eq!(outcome.payout, 0);
        assert_eq!(outcome.events, vec![RouteEvent::CargoTooLarge]);
    }

    #[test]
    fn rough_terrain_can_damage_a_two_wheel_drive_car() {
        let mut car = Car::new(
            "Old Sedan",
            crate::loot::LootRarity::Common,
            1996,
            700,
            3,
            5,
            Drivetrain::Rwd,
            crate::car::VehicleSpec {
                make: "Generic".to_string(),
                model: "Sedan".to_string(),
                trim: "Base".to_string(),
                body_style: "4-door sedan".to_string(),
                transmission: "5-speed manual".to_string(),
                curb_weight_lbs: 2800,
                cargo_volume_cu_ft: "13.0".to_string(),
                fuel_capacity_gal: "14.0".to_string(),
                engine: crate::car::EngineSpec {
                    layout: "inline-4".to_string(),
                    displacement_liters: "2.0".to_string(),
                    aspiration: "naturally aspirated".to_string(),
                    fuel: "gasoline".to_string(),
                    horsepower: 120,
                    horsepower_rpm: 5600,
                    torque_lb_ft: 125,
                    torque_rpm: 4200,
                },
            },
        );
        let job = Job::new(
            "Logging road drop",
            CargoType::Contraband,
            Terrain::Mountain,
            2,
            900,
            4,
            5,
        );

        let starting_condition = car.condition;
        let outcome = resolve_job(&mut car, &job, 11);

        assert!(outcome.completed || outcome.payout > 0);
        assert_ne!(car.condition, starting_condition);
    }

    #[test]
    fn successful_jobs_pay_full_payout() {
        let mut car = Car::new(
            "Old 4x4",
            crate::loot::LootRarity::Common,
            1994,
            1200,
            4,
            4,
            Drivetrain::FourWd,
            crate::car::VehicleSpec {
                make: "Generic".to_string(),
                model: "4x4".to_string(),
                trim: "Base".to_string(),
                body_style: "2-door SUV".to_string(),
                transmission: "5-speed manual".to_string(),
                curb_weight_lbs: 3600,
                cargo_volume_cu_ft: "30.0".to_string(),
                fuel_capacity_gal: "18.0".to_string(),
                engine: crate::car::EngineSpec {
                    layout: "inline-6".to_string(),
                    displacement_liters: "4.0".to_string(),
                    aspiration: "naturally aspirated".to_string(),
                    fuel: "gasoline".to_string(),
                    horsepower: 180,
                    horsepower_rpm: 4750,
                    torque_lb_ft: 220,
                    torque_rpm: 3000,
                },
            },
        );
        let job = Job::new(
            "Rural night drop",
            CargoType::Documents,
            Terrain::Rural,
            1,
            420,
            1,
            2,
        );

        let outcome = resolve_job(&mut car, &job, 3);

        assert!(outcome.completed);
        assert_eq!(outcome.payout, 420);
    }

    #[test]
    fn patrol_previews_show_visible_odds() {
        let car = Car::honda_civic_1998_dx_sedan();
        let job = Job::new(
            "Industrial parts delivery",
            CargoType::Parts,
            Terrain::Industrial,
            2,
            420,
            3,
            4,
        );

        let previews = patrol_action_previews(&car, &job, 0);

        assert_eq!(previews.len(), 3);
        assert!(previews.iter().all(|preview| preview.chance >= 10));
        assert!(previews.iter().all(|preview| preview.chance <= 90));
    }

    #[test]
    fn failed_push_speed_applies_damage_and_heat() {
        let mut car = Car::honda_civic_1998_dx_sedan();
        let job = Job::new(
            "Industrial parts delivery",
            CargoType::Parts,
            Terrain::Industrial,
            2,
            420,
            3,
            4,
        );

        let result = resolve_patrol_action(&mut car, &job, 0, PatrolAction::PushSpeed, 1);

        assert!(!result.success);
        assert_eq!(result.heat_gained, 1);
        assert_eq!(car.condition.engine, 90);
        assert_eq!(car.condition.tires, 88);
    }

    #[test]
    fn damage_and_heat_lower_patrol_odds() {
        let mut car = Car::honda_civic_1998_dx_sedan();
        let job = Job::new(
            "Industrial parts delivery",
            CargoType::Parts,
            Terrain::Industrial,
            2,
            420,
            3,
            4,
        );

        let clean = patrol_action_previews(&car, &job, 0);

        car.condition.apply_damage(PartDamage {
            engine: 30,
            suspension: 35,
            tires: 30,
            body: 40,
        });
        let damaged_hot = patrol_action_previews(&car, &job, 4);

        assert!(damaged_hot[0].chance < clean[0].chance);
        assert!(damaged_hot[1].chance < clean[1].chance);
        assert!(damaged_hot[2].chance < clean[2].chance);
    }

    #[test]
    fn fork_previews_include_two_risks_and_a_safe_backtrack() {
        let car = Car::honda_civic_1998_dx_sedan();
        let job = Job::new(
            "Industrial parts delivery",
            CargoType::Parts,
            Terrain::Industrial,
            2,
            420,
            3,
            4,
        );

        let previews = fork_choice_previews(&car, &job, 0);

        assert_eq!(previews.len(), 3);
        assert_eq!(previews[0].choice, ForkChoice::Highway);
        assert_eq!(previews[1].choice, ForkChoice::ServiceRoad);
        assert_eq!(previews[2].choice, ForkChoice::Backtrack);
        assert!(previews[0].chance.is_some());
        assert!(previews[1].chance.is_some());
        assert_eq!(previews[2].chance, None);
    }

    #[test]
    fn backtrack_reduces_payout_without_damage_or_heat() {
        let mut car = Car::honda_civic_1998_dx_sedan();
        let job = Job::new(
            "Industrial parts delivery",
            CargoType::Parts,
            Terrain::Industrial,
            2,
            420,
            3,
            4,
        );

        let result = resolve_fork_choice(&mut car, &job, 0, ForkChoice::Backtrack, 1);

        assert!(result.success);
        assert_eq!(result.roll, None);
        assert_eq!(result.heat_gained, 0);
        assert_eq!(result.damage, PartDamage::default());
        assert_eq!(result.payout_multiplier_percent, 80);
    }
}
