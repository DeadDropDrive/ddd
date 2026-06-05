pub mod balance;
pub mod car;
pub mod garage;
pub mod job;
pub mod loot;
pub mod route;
pub mod route_run;

mod generated_tables {
    include!(concat!(env!("OUT_DIR"), "/tables.rs"));
}

pub use balance::BalanceConfig;
pub use car::{
    generate_car_market_offers, Car, CarMarketOffer, Drivetrain, EffectiveStats, EngineSpec,
    InstalledUpgrade, PartCondition, VehicleSpec,
};
pub use garage::{Garage, JobReport};
pub use generated_tables::{market_car_catalog, starter_car_catalog, starter_jobs};
pub use job::{CargoType, Job, Terrain};
pub use loot::{
    generate_black_market_offers, generate_loot, BlackMarketOffer, InstallCostVoucherEffect,
    LootEffect, LootItem, LootRarity, UpgradeEffect,
};
pub use route::{
    fork_choice_previews, patrol_action_previews, resolve_fork_choice, resolve_job,
    resolve_patrol_action, CheckModifier, ForkChoice, ForkChoicePreview, ForkResolution,
    JobOutcome, PatrolAction, PatrolActionPreview, PatrolResolution, RouteEvent,
};
pub use route_run::{
    RouteAction, RouteEncounterKind, RouteEncounterPreview, RoutePhase, RouteResolution, RouteRun,
    RouteRunReport, SegmentOutcome, SegmentPreview, SegmentResolution,
};
