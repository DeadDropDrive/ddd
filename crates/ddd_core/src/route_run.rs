use crate::garage::Garage;
use crate::job::Job;
use crate::loot::{generate_loot, LootItem};
use crate::route::{
    fork_choice_previews_with_config, patrol_action_previews_with_config,
    resolve_fork_choice_with_config, resolve_patrol_action_with_config, ForkChoice,
    ForkChoicePreview, ForkResolution, JobOutcome, PatrolAction, PatrolActionPreview,
    PatrolResolution, RouteEvent,
};
use crate::{car::PartDamage, BalanceConfig, Car};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutePhase {
    Segment,
    Patrol,
    Fork,
    Backtrack,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteEncounterKind {
    Patrol,
    Fork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteEncounterPreview {
    Patrol(Vec<PatrolActionPreview>),
    Fork(Vec<ForkChoicePreview>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAction {
    Patrol(PatrolAction),
    Fork(ForkChoice),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteResolution {
    Patrol(PatrolResolution),
    Fork(ForkResolution),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentOutcome {
    Clear,
    Encounter(RouteEncounterKind),
    Crash,
    CaughtByCops,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPreview {
    pub segment_index: u8,
    pub segment_count: u8,
    pub encounter_chance: u8,
    pub crash_chance: u8,
    pub caught_chance: u8,
    pub route_marker: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentResolution {
    pub segment_index: u8,
    pub segment_count: u8,
    pub encounter_chance: u8,
    pub crash_chance: u8,
    pub caught_chance: u8,
    pub roll: u8,
    pub outcome: SegmentOutcome,
    pub events: Vec<RouteEvent>,
    pub heat_gained: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteRun {
    pub car_index: usize,
    pub job: Job,
    pub phase: RoutePhase,
    pub payout_multiplier_percent: u8,
    pub current_segment: u8,
    pub events: Vec<RouteEvent>,
    pub heat_gained: u8,
    pub car_wrecked: bool,
    pub caught_by_cops: bool,
    seed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteRunReport {
    pub outcome: JobOutcome,
    pub loot: Vec<LootItem>,
    pub payout_multiplier_percent: u8,
    pub backtracked: bool,
    pub car_wrecked: bool,
    pub caught_by_cops: bool,
}

impl RouteRun {
    pub fn new(car_index: usize, job: Job, seed: u64) -> Self {
        Self {
            car_index,
            job,
            phase: RoutePhase::Segment,
            payout_multiplier_percent: 100,
            current_segment: 0,
            events: Vec::new(),
            heat_gained: 0,
            car_wrecked: false,
            caught_by_cops: false,
            seed,
        }
    }

    pub fn next_encounter(&self) -> Option<RouteEncounterKind> {
        match self.phase {
            RoutePhase::Segment => None,
            RoutePhase::Patrol => Some(RouteEncounterKind::Patrol),
            RoutePhase::Fork => Some(RouteEncounterKind::Fork),
            RoutePhase::Backtrack | RoutePhase::Complete => None,
        }
    }

    pub fn encounter_preview(&self, garage: &Garage) -> Option<RouteEncounterPreview> {
        match self.phase {
            RoutePhase::Segment => None,
            RoutePhase::Patrol => self
                .patrol_previews(garage)
                .map(RouteEncounterPreview::Patrol),
            RoutePhase::Fork => self.fork_previews(garage).map(RouteEncounterPreview::Fork),
            RoutePhase::Backtrack | RoutePhase::Complete => None,
        }
    }

    pub fn segment_preview(&self, garage: &Garage) -> Option<SegmentPreview> {
        if self.phase != RoutePhase::Segment || self.current_segment >= self.job.distance {
            return None;
        }
        let car = garage.cars.get(self.car_index)?;
        Some(SegmentPreview {
            segment_index: self.current_segment,
            segment_count: self.job.distance,
            encounter_chance: segment_encounter_chance(&self.job, &garage.balance),
            crash_chance: segment_crash_chance(car, &self.job, garage.heat),
            caught_chance: segment_caught_chance(&self.job, garage.heat),
            route_marker: route_marker(self.current_segment, self.job.distance),
        })
    }

    pub fn resolve_segment(&mut self, garage: &mut Garage) -> Option<SegmentResolution> {
        let preview = self.segment_preview(garage)?;
        let mut rng = SeededRng::new(self.seed);
        let roll = rng.d100();
        let mut events = Vec::new();
        let mut heat_gained = 0;

        if roll <= preview.crash_chance {
            self.car_wrecked = true;
            self.phase = RoutePhase::Complete;
            self.events.push(RouteEvent::Breakdown);
            garage.cars.remove(self.car_index);
            return Some(SegmentResolution {
                roll,
                outcome: SegmentOutcome::Crash,
                events,
                heat_gained,
                ..preview.into_resolution_parts()
            });
        }

        let caught_threshold = preview
            .crash_chance
            .saturating_add(preview.caught_chance)
            .min(100);
        if roll <= caught_threshold {
            self.caught_by_cops = true;
            self.phase = RoutePhase::Complete;
            heat_gained = caught_heat_gained(&self.job, garage.heat);
            garage.heat = garage.heat.saturating_add(heat_gained);
            self.heat_gained = self.heat_gained.saturating_add(heat_gained);
            events.push(RouteEvent::CaughtByCops);
            self.events.extend(events.clone());
            garage.cars.remove(self.car_index);
            return Some(SegmentResolution {
                roll,
                outcome: SegmentOutcome::CaughtByCops,
                events,
                heat_gained,
                ..preview.into_resolution_parts()
            });
        }

        let encounter_threshold = preview
            .crash_chance
            .saturating_add(preview.caught_chance)
            .saturating_add(preview.encounter_chance)
            .min(100);
        let outcome = if roll <= encounter_threshold {
            let encounter = if (self.current_segment + roll) % 2 == 0 {
                RouteEncounterKind::Patrol
            } else {
                RouteEncounterKind::Fork
            };
            self.phase = match encounter {
                RouteEncounterKind::Patrol => RoutePhase::Patrol,
                RouteEncounterKind::Fork => RoutePhase::Fork,
            };
            SegmentOutcome::Encounter(encounter)
        } else {
            let car = garage.cars.get_mut(self.car_index)?;
            events = resolve_clear_segment(car, &self.job, &mut rng, &garage.balance);
            heat_gained = segment_heat_gained(&events, &garage.balance);
            garage.heat = garage.heat.saturating_add(heat_gained);
            self.heat_gained = self.heat_gained.saturating_add(heat_gained);
            self.events.extend(events.clone());
            self.advance_segment();
            SegmentOutcome::Clear
        };

        self.seed = self.seed.wrapping_add(17);
        Some(SegmentResolution {
            roll,
            outcome,
            events,
            heat_gained,
            ..preview.into_resolution_parts()
        })
    }

    pub fn resolve_action(
        &mut self,
        garage: &mut Garage,
        action: RouteAction,
    ) -> Option<RouteResolution> {
        match action {
            RouteAction::Patrol(action) => self
                .resolve_patrol(garage, action)
                .map(RouteResolution::Patrol),
            RouteAction::Fork(choice) => {
                self.resolve_fork(garage, choice).map(RouteResolution::Fork)
            }
        }
    }

    pub fn patrol_previews(&self, garage: &Garage) -> Option<Vec<PatrolActionPreview>> {
        if self.phase != RoutePhase::Patrol {
            return None;
        }

        let car = garage.cars.get(self.car_index)?;
        Some(patrol_action_previews_with_config(
            car,
            &self.job,
            garage.heat,
            &garage.balance,
        ))
    }

    pub fn resolve_patrol(
        &mut self,
        garage: &mut Garage,
        action: PatrolAction,
    ) -> Option<PatrolResolution> {
        if self.phase != RoutePhase::Patrol {
            return None;
        }

        let car = garage.cars.get_mut(self.car_index)?;
        let resolution = resolve_patrol_action_with_config(
            car,
            &self.job,
            garage.heat,
            action,
            self.seed,
            &garage.balance,
        );
        garage.heat = garage.heat.saturating_add(resolution.heat_gained);
        self.heat_gained = self.heat_gained.saturating_add(resolution.heat_gained);
        self.seed = self.seed.wrapping_add(17);
        self.advance_segment();

        Some(resolution)
    }

    pub fn fork_previews(&self, garage: &Garage) -> Option<Vec<ForkChoicePreview>> {
        if self.phase != RoutePhase::Fork {
            return None;
        }

        let car = garage.cars.get(self.car_index)?;
        Some(fork_choice_previews_with_config(
            car,
            &self.job,
            garage.heat,
            &garage.balance,
        ))
    }

    pub fn resolve_fork(
        &mut self,
        garage: &mut Garage,
        choice: ForkChoice,
    ) -> Option<ForkResolution> {
        if self.phase != RoutePhase::Fork {
            return None;
        }

        let car = garage.cars.get_mut(self.car_index)?;
        let mut resolution = resolve_fork_choice_with_config(
            car,
            &self.job,
            garage.heat,
            choice,
            self.seed,
            &garage.balance,
        );
        garage.heat = garage.heat.saturating_add(resolution.heat_gained);
        self.heat_gained = self.heat_gained.saturating_add(resolution.heat_gained);
        self.seed = self.seed.wrapping_add(17);
        if choice == ForkChoice::Backtrack {
            resolution.payout_multiplier_percent = backtrack_payout_multiplier_percent(
                garage.balance.fork_backtrack_payout_multiplier_percent,
                self.current_segment,
                self.job.distance,
            );
        }
        self.payout_multiplier_percent = resolution.payout_multiplier_percent;
        self.phase = if choice == ForkChoice::Backtrack {
            RoutePhase::Backtrack
        } else {
            self.advance_segment();
            self.phase
        };

        Some(resolution)
    }

    pub fn complete(&mut self, garage: &mut Garage) -> Option<RouteRunReport> {
        match self.phase {
            RoutePhase::Segment if self.current_segment >= self.job.distance => {
                self.complete_travel(garage)
            }
            RoutePhase::Backtrack => Some(self.complete_backtrack(garage)),
            RoutePhase::Complete if self.car_wrecked => Some(self.complete_wreck()),
            RoutePhase::Complete if self.caught_by_cops => Some(self.complete_caught()),
            _ => None,
        }
    }

    fn complete_travel(&mut self, garage: &mut Garage) -> Option<RouteRunReport> {
        let car = garage.cars.get(self.car_index)?;
        let completed = self.job.cargo_size <= car.cargo_capacity;
        let payout = if completed {
            apply_payout_multiplier(self.job.payout, self.payout_multiplier_percent)
        } else {
            0
        };
        let heat_gained = if completed {
            self.heat_gained
        } else {
            self.heat_gained
                .saturating_add(garage.balance.cargo_too_large_heat)
        };
        let mut events = self.events.clone();
        if !completed {
            events.push(RouteEvent::CargoTooLarge);
        }
        let outcome = JobOutcome {
            completed,
            payout,
            heat_gained,
            events,
        };
        let loot = if completed {
            generate_loot(&self.job, self.seed.wrapping_add(302), &garage.balance)
        } else {
            Vec::new()
        };

        garage.cash += payout;
        if !completed {
            garage.heat = garage
                .heat
                .saturating_add(garage.balance.cargo_too_large_heat);
        }
        garage.inventory.extend(loot.clone());
        self.phase = RoutePhase::Complete;

        Some(RouteRunReport {
            outcome,
            loot,
            payout_multiplier_percent: self.payout_multiplier_percent,
            backtracked: false,
            car_wrecked: false,
            caught_by_cops: false,
        })
    }

    fn complete_backtrack(&mut self, garage: &mut Garage) -> RouteRunReport {
        let payout = apply_payout_multiplier(self.job.payout, self.payout_multiplier_percent);
        garage.cash += payout;
        self.phase = RoutePhase::Complete;

        RouteRunReport {
            outcome: JobOutcome {
                completed: true,
                payout,
                heat_gained: 0,
                events: vec![RouteEvent::Backtracked],
            },
            loot: Vec::new(),
            payout_multiplier_percent: self.payout_multiplier_percent,
            backtracked: true,
            car_wrecked: false,
            caught_by_cops: false,
        }
    }

    fn complete_wreck(&mut self) -> RouteRunReport {
        RouteRunReport {
            outcome: JobOutcome {
                completed: false,
                payout: 0,
                heat_gained: self.heat_gained,
                events: self.events.clone(),
            },
            loot: Vec::new(),
            payout_multiplier_percent: 0,
            backtracked: false,
            car_wrecked: true,
            caught_by_cops: false,
        }
    }

    fn complete_caught(&mut self) -> RouteRunReport {
        RouteRunReport {
            outcome: JobOutcome {
                completed: false,
                payout: 0,
                heat_gained: self.heat_gained,
                events: self.events.clone(),
            },
            loot: Vec::new(),
            payout_multiplier_percent: 0,
            backtracked: false,
            car_wrecked: false,
            caught_by_cops: true,
        }
    }

    fn advance_segment(&mut self) {
        self.current_segment = self.current_segment.saturating_add(1);
        self.phase = RoutePhase::Segment;
    }
}

impl SegmentPreview {
    fn into_resolution_parts(self) -> SegmentResolution {
        SegmentResolution {
            segment_index: self.segment_index,
            segment_count: self.segment_count,
            encounter_chance: self.encounter_chance,
            crash_chance: self.crash_chance,
            caught_chance: self.caught_chance,
            roll: 0,
            outcome: SegmentOutcome::Clear,
            events: Vec::new(),
            heat_gained: 0,
        }
    }
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

fn segment_encounter_chance(job: &Job, config: &BalanceConfig) -> u8 {
    let payout_pressure = (job.payout / 100).clamp(0, 20) as u8;
    config
        .loot_base_drop_chance
        .saturating_add(job.heat.saturating_mul(4))
        .saturating_add(job.terrain.roughness())
        .saturating_add(job.cargo_size.saturating_mul(3))
        .saturating_add(payout_pressure)
        .min(config.chance_max)
}

fn segment_crash_chance(car: &Car, job: &Job, garage_heat: u8) -> u8 {
    let condition_risk = 100_u8
        .saturating_sub(car.condition.average())
        .saturating_div(10);
    let terrain_risk = job.terrain.roughness().saturating_div(2);
    let heat_risk = garage_heat.saturating_div(4);
    condition_risk
        .saturating_add(terrain_risk)
        .saturating_add(heat_risk)
        .min(18)
}

fn segment_caught_chance(job: &Job, garage_heat: u8) -> u8 {
    let payout_risk = (job.payout / 250).clamp(0, 8) as u8;
    job.heat
        .saturating_add(garage_heat.saturating_div(3))
        .saturating_add(job.cargo_size)
        .saturating_add(payout_risk)
        .min(16)
}

fn caught_heat_gained(job: &Job, garage_heat: u8) -> u8 {
    job.heat
        .saturating_add(garage_heat.saturating_div(4))
        .max(3)
        .min(10)
}

fn resolve_clear_segment(
    car: &mut Car,
    job: &Job,
    rng: &mut SeededRng,
    config: &BalanceConfig,
) -> Vec<RouteEvent> {
    let mut events = Vec::new();
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
        events.push(RouteEvent::PatrolCloseCall);
    }

    events
}

fn segment_heat_gained(events: &[RouteEvent], config: &BalanceConfig) -> u8 {
    let close_calls = events
        .iter()
        .filter(|event| matches!(event, RouteEvent::PatrolCloseCall))
        .count() as u8;
    close_calls.saturating_mul(config.patrol_close_call_heat)
}

fn segment_stress(car: &Car, job: &Job, config: &BalanceConfig) -> u8 {
    let mut stress = job
        .terrain
        .roughness()
        .saturating_mul(config.segment_terrain_stress_multiplier);
    if !car.drivetrain.handles_rough_terrain() {
        stress = stress.saturating_add(config.segment_rough_terrain_2wd_penalty);
    }
    if car.condition.average() < config.segment_low_condition_threshold {
        stress = stress.saturating_add(config.segment_low_condition_penalty);
    }
    stress.min(config.segment_stress_cap)
}

fn damage_for(terrain: crate::Terrain, roll: u8, config: &BalanceConfig) -> PartDamage {
    let base = terrain.roughness().max(1);
    if roll % 3 == 0 {
        PartDamage {
            tires: base + config.damage_tire_extra,
            ..PartDamage::default()
        }
    } else if roll % 3 == 1 {
        PartDamage {
            suspension: base + config.damage_suspension_extra,
            ..PartDamage::default()
        }
    } else {
        PartDamage {
            body: base + config.damage_body_extra,
            ..PartDamage::default()
        }
    }
}

fn route_marker(current_segment: u8, segment_count: u8) -> String {
    let mut marker = String::new();
    for index in 0..segment_count {
        if index < current_segment {
            marker.push('=');
        } else if index == current_segment {
            marker.push('>');
        } else {
            marker.push('-');
        }
    }
    marker
}

fn apply_payout_multiplier(payout: i32, multiplier_percent: u8) -> i32 {
    payout * multiplier_percent as i32 / 100
}

fn backtrack_payout_multiplier_percent(base_percent: u8, current_segment: u8, distance: u8) -> u8 {
    if distance == 0 {
        return 0;
    }
    let travelled_segments = current_segment.saturating_add(1).min(distance) as u16;
    (base_percent as u16 * travelled_segments / distance as u16) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{CargoType, Terrain};

    #[test]
    fn route_run_advances_through_patrol_and_fork() {
        let mut garage = Garage::starter();
        let job = Job::new(
            "Industrial parts delivery",
            CargoType::Parts,
            Terrain::Industrial,
            2,
            420,
            3,
            4,
        );
        let mut run = RouteRun::new(0, job, 11);

        assert_eq!(run.phase, RoutePhase::Segment);
        assert!(run.segment_preview(&garage).is_some());
        let resolution = drive_until_encounter(&mut run, &mut garage)
            .expect("route should produce an encounter");

        assert!(matches!(resolution.outcome, SegmentOutcome::Encounter(_)));
        assert!(run.next_encounter().is_some());
        assert!(run.encounter_preview(&garage).is_some());
    }

    #[test]
    fn backtrack_completes_with_reduced_payout_and_no_loot() {
        let mut garage = Garage::starter();
        let job = Job::new(
            "Industrial parts delivery",
            CargoType::Parts,
            Terrain::Industrial,
            2,
            420,
            3,
            4,
        );
        let mut run = RouteRun::new(0, job, 11);
        run.phase = RoutePhase::Fork;

        assert!(matches!(
            run.encounter_preview(&garage),
            Some(RouteEncounterPreview::Fork(_))
        ));
        run.resolve_fork(&mut garage, ForkChoice::Backtrack)
            .expect("fork should resolve");
        let report = run.complete(&mut garage).expect("route should complete");

        assert!(report.backtracked);
        assert_eq!(report.payout_multiplier_percent, 20);
        assert_eq!(report.outcome.payout, 84);
        assert!(report.loot.is_empty());
        assert_eq!(garage.cash, 834);
        assert_eq!(run.phase, RoutePhase::Complete);
    }

    #[test]
    fn backtrack_payout_scales_with_distance_travelled() {
        let mut garage = Garage::starter();
        let job = Job::new(
            "Industrial parts delivery",
            CargoType::Parts,
            Terrain::Industrial,
            2,
            420,
            3,
            4,
        );
        let mut run = RouteRun::new(0, job, 11);
        run.current_segment = 2;
        run.phase = RoutePhase::Fork;

        let resolution = run
            .resolve_fork(&mut garage, ForkChoice::Backtrack)
            .expect("fork should resolve");
        let report = run.complete(&mut garage).expect("route should complete");

        assert_eq!(resolution.payout_multiplier_percent, 60);
        assert_eq!(report.outcome.payout, 252);
    }

    #[test]
    fn crash_wrecks_and_removes_car() {
        let mut garage = Garage::starter();
        garage.cars[0].condition.engine = 1;
        garage.heat = 50;
        let job = Job::new(
            "Mountain pass emergency",
            CargoType::Contraband,
            Terrain::Mountain,
            2,
            900,
            8,
            4,
        );
        let starting_cars = garage.cars.len();
        let mut run = RouteRun::new(0, job, 1);
        run.seed = crash_seed_for(&run, &garage);

        let resolution = run
            .resolve_segment(&mut garage)
            .expect("segment should resolve");
        let report = run.complete(&mut garage).expect("wreck should report");

        assert_eq!(resolution.outcome, SegmentOutcome::Crash);
        assert!(report.car_wrecked);
        assert!(report.loot.is_empty());
        assert_eq!(garage.cars.len(), starting_cars - 1);
        assert_eq!(run.phase, RoutePhase::Complete);
    }

    #[test]
    fn cops_can_catch_driver_and_impound_car() {
        let mut garage = Garage::starter();
        garage.heat = 40;
        let job = Job::new(
            "Hot courier job",
            CargoType::Contraband,
            Terrain::City,
            3,
            1200,
            9,
            4,
        );
        let starting_cars = garage.cars.len();
        let mut run = RouteRun::new(0, job, 1);
        run.seed = caught_seed_for(&run, &garage);

        let resolution = run
            .resolve_segment(&mut garage)
            .expect("segment should resolve");
        let report = run.complete(&mut garage).expect("caught run should report");

        assert_eq!(resolution.outcome, SegmentOutcome::CaughtByCops);
        assert!(report.caught_by_cops);
        assert!(!report.car_wrecked);
        assert_eq!(report.outcome.payout, 0);
        assert_eq!(garage.cars.len(), starting_cars - 1);
        assert_eq!(run.phase, RoutePhase::Complete);
    }

    fn drive_until_encounter(run: &mut RouteRun, garage: &mut Garage) -> Option<SegmentResolution> {
        while run.phase == RoutePhase::Segment && run.current_segment < run.job.distance {
            let resolution = run.resolve_segment(garage)?;
            if matches!(resolution.outcome, SegmentOutcome::Encounter(_)) {
                return Some(resolution);
            }
        }
        None
    }

    fn crash_seed_for(run: &RouteRun, garage: &Garage) -> u64 {
        let crash_chance = run.segment_preview(garage).unwrap().crash_chance;
        (1..10_000)
            .find(|seed| {
                let mut rng = SeededRng::new(*seed);
                rng.d100() <= crash_chance
            })
            .expect("expected a seed that crashes")
    }

    fn caught_seed_for(run: &RouteRun, garage: &Garage) -> u64 {
        let preview = run.segment_preview(garage).unwrap();
        let min_roll = preview.crash_chance.saturating_add(1);
        let max_roll = preview
            .crash_chance
            .saturating_add(preview.caught_chance)
            .min(100);
        (1..10_000)
            .find(|seed| {
                let mut rng = SeededRng::new(*seed);
                let roll = rng.d100();
                roll >= min_roll && roll <= max_roll
            })
            .expect("expected a seed that gets caught")
    }
}
