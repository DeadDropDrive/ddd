# Dead Drop Drive v0.1 Milestone

## Goal

Ship the first public playable prototype: a complete garage-to-route loop that shows the core fantasy of Dead Drop Drive without promising a finished game.

## Definition Of Done

`v0.1` should let a player:

- enter a driver name
- choose a starter car
- inspect a garage
- choose a contract
- drive a segment-based smuggling route
- resolve visible-risk route encounters
- suffer or avoid heat, damage, police impound, and wreck outcomes
- return to the garage after successful or failed runs
- repair cars
- install and uninstall vehicle upgrades
- buy and sell black-market parts
- buy and sell cars without selling the last car
- understand the missing save/load limitation from the README

## Current Status

Most of the playable loop exists. The project already has:

- Rust workspace with `ddd_core` and `ddd_cli`
- Ratatui/Crossterm terminal UI
- starter setup
- garage, mechanic, parts market, and car market screens
- contract selection
- segment-based route traversal
- Patrol and Fork route encounters
- critical crash and police impound failures
- loot, vehicle upgrades, install vouchers, and sale values
- compile-time TSV data tables for cars, jobs, and loot
- deterministic core tests

## Remaining Work

Before tagging `v0.1`, finish:

- README screenshot or terminal recording
- stale terminology pass across docs and comments
- broader manual TUI smoke test on a default terminal size
- at least one more pass over route balance values
- enough car/job/loot table entries to avoid the prototype feeling empty
- release notes that clearly call out no save/load yet

## Out Of Scope For v0.1

- save/load
- audio
- graphical UI
- procedural region map
- large car roster
- deep upgrade compatibility rules
- long-term campaign balancing
