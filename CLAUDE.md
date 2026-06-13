# Brawler — agent entry point

Project agent contract and required reading: @AGENTS.md

## Token discipline

Prefix shell/file commands with `rtk` (e.g. `rtk git`, `rtk grep`, `rtk read`, `rtk cargo`, `rtk rad`); it compresses output before it reaches context. `rtk proxy <cmd>` runs raw. Run `rtk trust` once in this repo so the project-local filters in `.rtk/` apply. Prefer `repoctx` and targeted reads over whole-file reads.