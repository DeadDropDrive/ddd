# Dead Drop Drive

Dead Drop Drive, or DDD for short, is a terminal-first car smuggling roguelike prototype written in Rust.

Run a clandestine garage, choose risky delivery contracts, tune cheap cars for bad routes, manage heat, and try to keep the fleet alive long enough to buy something better.

## Status

This is an early playable prototype, not a finished game. The current version has:

- A Ratatui/Crossterm terminal UI.
- Starter setup with driver name and starter car selection.
- Garage management with vehicle condition, repairs, inventory, installed parts, and heat.
- Contract selection and segment-based smuggling routes.
- Route encounters with visible risk previews and d100 resolution.
- Critical route failures, including wrecks and police impound outcomes.
- Loot, installable vehicle upgrades, install vouchers, and black-market selling.
- Rotating black-market parts offers and car-market offers.
- Compile-time data tables for cars, jobs, and loot.
- Deterministic core simulation logic covered by tests.

The project is currently single-player, local-only, and save/load is not implemented yet.

## Workspace

The Rust package names are:

- `ddd_core`: pure simulation and game rules.
- `ddd_cli`: terminal UI and input handling.

The crate directories are:

- `crates/ddd_core`
- `crates/ddd_cli`

## Requirements

- Rust stable
- A terminal with basic color support

Install Rust with `rustup` if needed:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Run

```sh
cargo run -p ddd_cli
```

The TUI starts with a setup screen. Common controls:

- `Tab` / `Left` / `Right`: move focus
- `Up` / `Down` or `j` / `k`: move selection
- `Enter`: confirm or apply the focused action
- `Esc`: return to the previous screen where available
- `q`: quit

From the garage:

- `m`: open The Mechanic
- `b`: open the parts black market
- `c`: open the car market
- `r`: repair the selected car

## Test

```sh
cargo test
```

## Data And Balance

Designer-editable source tables live in `config/tables/`:

- `cars.tsv`
- `jobs.tsv`
- `loot.tsv`

The build script in `ddd_core` reads these tables at compile time and generates static Rust data. The game does not read the TSV files at runtime.

## Balance

Game tuning constants live in the `ddd_core` package at `crates/ddd_core/src/balance.rs`.

`config/balance.toml` mirrors the current defaults for design review. The game does not load the TOML file yet; external balance loading is future work.

## Development Notes

Core gameplay rules should stay in `ddd_core`. The terminal UI should present data from the core layer and submit player choices back to it.

Before publishing changes, run:

```sh
cargo fmt
cargo test
```

## System Standards

Design and implementation rules for extending loot, encounters, routes, balance, vehicles, and presentation live in `docs/SYSTEM_STANDARDS.md`.

## First Milestone

The current target is `v0.1`: a complete playable garage-to-route prototype. Scope is tracked in `docs/MILESTONE_V0_1.md`.

## License

MIT
