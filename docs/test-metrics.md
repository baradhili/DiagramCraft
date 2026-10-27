# Test metrics ledger

Started 2026-10-27 (#97 §7 — "what to track instead of line
coverage"). This file is the durable record; update it as events
happen, not on a schedule.

## Per-suite wall-time budget

Current measured budgets (debug, this workstation, 2026-10-27) — a
suite whose runtime crosses its budget on CI deserves a look before it
lands:

| Suite | Runtime | Notes |
|---|---|---|
| vsdx (unit + roundtrip + synthetic + corpus subset) | ~4.0 s | corpus subset 3.9 s of it |
| dc-core (editor + touch) | <0.2 s | |
| dc (svg unit + cli) | <0.5 s | |
| dc-ui shell | 1.4 s | |
| dc-ui interactions | 3.7 s | |
| dc-ui group_edit / close_prompt / icon_pixels | <2.2 s combined | |
| dc-ui raster_async | 5.9 s | bounded pumps, count-based asserts |
| dc-ui render_perf gate (n=500) | 14.4 s | n=2000 twin behind `--ignored` |

Full workspace ≈ 35 s test time. Suite-total regression past ~60 s is
the review trigger.

## Golden churn — **weekly cadence** (owner direction 2026-10-27: at
current dev rate, monthly review is too slow)

Count of `UPDATE_SNAPSHOTS=1` regenerations per week (19 tracked
goldens; high churn means goldens pin the wrong granularity):

- Week of 2026-10-27: **1 event** — the owner-selected title-band
  accent change (#817e7b warm gray, 2026-10-27) legitimately re-rendered
  every golden that shows the band: 24 of 24 regenerated. (Before it:
  0 — the #89–#93/#96 perf waves and the entire §4 wave landed
  byte-identical.) A deliberate full-set repaint like this is healthy
  churn; the metric watches for *unexpected* regeneration counts.

## Platform-failure log (OS-only CI failures)

One-line triage per event; population starts at n=1:

- 2026-10-24, `ci` on `0.2.1`, **windows-latest** only, failing step
  `cargo test --workspace --locked`. Test name unrecoverable from the
  API without log download; not investigated. Fixed by the next run
  (2026-10-24T10:34 green, no code change identified) → treated as
  environmental; will reclassify if it recurs.

## Corpus pass count

- Tracked subset (blank + stride-19 of the #87 catalog, ~20 files):
  **green on every push** (`vsdx/tests/corpus.rs`).
- Full 374-stencil catalog twin: `--ignored`, release recipe
  (`cargo test -p vsdx --release --test corpus -- --ignored`) —
  2026-10-27: **374/374 green** (112 s debug).
- Full 679-file local corpus (untracked `Sample/Office16`): manual,
  last known green 2026-10-16 (v0.2 definition of done).

## CLI contract matrix

`dc/tests/cli.rs` is the matrix — render (single/multi/-N/--page/range),
roundtrip --verify, edit (cell/formula/text/pages/layers/connect/
undo/history + malformed-input rejections), info/inspect (incl. missing
file → stderr names the file). Green since 2026-10-27.

## Mutation score — NOT STARTED

`cargo-mutants` on vsdx formula/write + dc-core commands/scope is the
intended first target; not yet installed or run. No number exists.
