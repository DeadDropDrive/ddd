# Compile-Time Data Tables

These TSV files are designer-editable source data for Dead Drop Drive.

`ddd_core`'s build script reads these files during compilation and generates Rust code into Cargo's `OUT_DIR`. The game does not load these files at runtime.

Current compiled tables:

- `cars.tsv`: car catalog with rarity, flavor text, real/spec data, game stats, starter eligibility, and car-market eligibility.
- `jobs.tsv`: starter contracts.
- `loot.tsv`: canonical loot/item catalog, including sell-only loot, vehicle upgrades, vouchers, black-market eligibility, primary drop ranges, and bonus drop ranges.

Black-market stock is generated from `loot.tsv` rows where `black_market_eligible` is `true`.

Starter car choices are generated from `cars.tsv` rows where `starter_eligible` is `true`.

Future car-market stock can be generated from `cars.tsv` rows where `market_eligible` is `true`.

Primary loot drops use rows with `primary_cargo`, `primary_min_roll`, and `primary_max_roll`.

Bonus loot drops use rows with `bonus_min_roll` and `bonus_max_roll`.

After changing a table, run:

```sh
cargo test
```

Cargo will rerun the build script when any table changes. Bad enum names, missing columns, or malformed rows fail the build.

Encounter and route behavior still lives in Rust because those entries are currently formulas and action logic rather than simple data rows. When those stabilize, they should get tables here and code generation in the `ddd_core` build script.
