# Dead Drop Drive System Standards

This document defines the rules for adding new gameplay elements without making the simulation logically inconsistent.

The current rule of thumb: every new element should make the garage loop richer while preserving the same core language of cars, cargo, heat, damage, risk, and reward.

## Core Principles

1. Game logic belongs in `ddd_core`.
2. CLI/TUI code presents decisions, but does not invent mechanics.
3. New mechanics should expose preview data before resolution when the player is making a risky choice.
4. Random outcomes must be deterministic from an explicit seed.
5. Balance numbers belong in `BalanceConfig`, with matching notes in `config/balance.toml`.
6. Real vehicle specs and game balance stats are separate.
7. A harsh outcome is fair only if the risk was visible before the player committed.

## Required Pattern For Player-Facing Risk

Any risky player choice should have two phases:

1. Preview
2. Resolution

Preview should include:

- action label
- check type
- d100 target or explicit "no roll"
- visible modifiers
- success result
- failure result

Resolution should include:

- selected action
- target, if any
- roll, if any
- success/failure
- heat change
- damage change
- payout/reward change
- any generated loot or state changes

## Encounter Standards

Encounters are tactical route events that force a decision under pressure.

Examples:

- patrol
- fork in the road
- roadblock
- checkpoint inspection
- mechanical crisis
- rival crew
- fuel shortage

Every encounter type should define:

- an action enum, such as `PatrolAction` or `ForkChoice`
- a preview struct
- a resolution struct
- a preview function
- a resolution function
- one or more tests proving the expected consequences

Encounters should not directly print text or read input. They should return data for CLI/TUI layers to present.

### Encounter Checklist

Before adding an encounter, answer:

- What fantasy does this represent in the automotive smuggling world?
- What car attributes matter?
- What current state matters: damage, heat, cargo, terrain, cash, inventory?
- What is the safe option?
- What is the risky option?
- What is the greedy option?
- What does success change?
- What does failure change?
- Which values belong in `BalanceConfig`?

## Route Standards

Routes are chains of decisions and consequences, not a single roll.

Route elements should be able to affect:

- car condition
- garage heat
- payout multiplier
- job completion
- cargo outcome
- loot chance
- future encounter odds

Current route logic lives in `RouteRun`. The TUI handles presentation, asks core for route state and encounter previews, and submits route actions back to core.

Route state should track:

- selected car
- selected job
- current segment
- accumulated heat
- payout multiplier
- completion/failure state
- next encounter
- route log
- generated loot

The route model currently supports segment-by-segment traversal, deterministic segment checks, critical crash outcomes, police impound outcomes, dynamic Patrol/Fork encounter selection, backtracking, and completion reports.

## Loot Standards

Loot should feel like automotive black-market rewards, not generic fantasy treasure.

Loot can be:

- sell-only contraband
- car parts
- garage tools
- forged documents
- favors
- black-market vouchers
- upgrade components

Every loot item should have:

- name
- rarity
- black-market value
- effect metadata

Loot effects may be inactive metadata at first, but they should describe an intended future use.

### Loot Rarity

Use rarity to communicate excitement and expected power/value:

- `Common`: mostly sellable, low-value, routine finds
- `Uncommon`: useful garage or vehicle-adjacent items
- `Rare`: high-value, build-defining, or strategically meaningful items

### Loot Rules

- Loot generation must be deterministic from a seed.
- Loot chance should consider job danger.
- Higher heat, cargo risk, or harder terrain can justify better loot odds or value.
- Loot should not replace base payout; it should add variable reward.
- Installable loot should eventually create a choice: sell now or keep for a build.

## Balance Standards

Any value that affects tuning should live in `BalanceConfig`.

Examples:

- base check chances
- chance caps
- heat penalties
- damage amounts
- payout multipliers
- loot drop chances
- repair-related modifiers
- route stress values

When adding a new balance field:

- add it to `BalanceConfig`
- add its default
- mirror it in `config/balance.toml`
- use it from core logic
- add or update tests if behavior changes

Do not bury balance numbers in CLI code.

## Vehicle Standards

Vehicle data has two layers.

Real/spec layer:

- make
- model
- trim
- body style
- engine layout
- displacement
- aspiration
- horsepower
- torque
- weight
- cargo volume
- fuel capacity
- transmission
- drivetrain

Game/balance layer:

- value
- cargo capacity
- stealth
- effective stats
- condition
- future installed upgrades

Real specs should be as accurate as practical. Game stats should be tuned for play and do not need to map literally to real specifications.

## CLI/TUI Standards

Presentation layers should:

- show previews before risky choices
- show target and roll after resolution
- show consequences immediately
- show persistent state changes on the next garage screen
- avoid hiding important modifiers
- present post-run choices as optional actions rather than fixed prompt chains

Presentation layers should not:

- generate loot
- calculate encounter odds
- apply damage directly
- mutate heat directly except through core APIs
- contain balance constants

## Test Standards

Each new system should have focused tests for:

- deterministic behavior from a fixed seed
- at least one success path
- at least one failure path
- state changes such as heat, damage, payout, inventory, or completion
- edge cases such as safe/no-roll choices

Tests should verify game rules, not CLI formatting.

## Current Known Debt

- `config/balance.toml` mirrors defaults but is not loaded by the game yet.
- Installed loot has no equipment slots, stacking limits, uninstall flow, or compatibility rules yet.
- The TUI is now the main UX path, but it is still a first pass and needs stronger layout polish, screenshots, and broader interaction testing.
- Loot effects can now be installed as simple car upgrades, but install rules are still intentionally basic.
- Encounter preview/resolution structs are still type-specific and similar enough that shared structure may become useful as more encounter types are added.
- Save/load is not implemented yet.
- The route system has only two interactive encounter families, so route variety is still thin.
- Car and job tables are intentionally small and need more content before the loop has staying power.

## Next Architecture Target

Continue evolving `RouteRun` and the data tables until the route layer can support richer run variety:

- more encounter families
- region/job-driven encounter weighting
- clearer separation between route timeline data and encounter formulas
- save/load friendly route and garage state
- enough route outcomes to make repeated runs feel meaningfully different

The TUI should remain a thin shell around this route-run state machine.
