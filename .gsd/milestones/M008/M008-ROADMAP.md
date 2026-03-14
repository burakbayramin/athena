# M008: ath init Interactive Setup

**Vision:** `ath init` guides new users through project setup with generated configs and environment validation.

## Success Criteria

- `ath init` creates `.ath/` directory with example config files
- Reports which API keys are detected
- Doesn't overwrite existing configs
- All tests pass

## Slices

- [x] **S01: Working init command** `risk:low` `depends:[]`
  > After this: `ath init` creates .ath/ with agents.toml, skills.toml, reports env status — proven by unit tests
