# myuri-rs

Rust port of [`Myuri`](https://github.com/JWriter20/Myuri) (`5.1.0`) with an embedded Rust port of [`cursory-js`](https://github.com/JWriter20/cursory-js) (`2.1.0`).

Simulates human pointer movement, clicks, drags, wheel/trackpad scrolling, and Markov-modeled typing with exact cross-language seed parity against TypeScript and Python.

## Module Layout

- `cursory` — embedded `trajectories.json.gz` dataset, NumPy-compatible `PCG64` + `SeedSequence` + `Ziggurat` PRNG, geometric morphing, correlated jitter, and trajectory sampling
- `math`, `rng`, `target_os`, `settings`, `profile`, `options`, `geometry` — Mulberry32 PRNG, per-seed behavioral profiles, viewport geometry, and padding/exclusion rules
- `motion` (`click_model`, `scroll_model`, `wheel_model`) — lognormal mouse hold/dwell/drag timing and bimodal flick/drag/notched/macOS wheel paths
- `typing` (`config`, `language`, `bigrams`, `keyboard`, `markov`, `timing`, `typer`) — neighbor-key/swap Markov typer, English bigram timing, Shift overlap/rollover scheduling
- `plan`, `page_functions`, `scroll_plan`, `page`, `actions`, `drivers` (`PlaywrightDriver` via `playwright-rs`), `cursor` (`Cursor`, `create_cursor`, `create_playwright_cursor`)

## Usage

```rust
use myuri_rs::{make_rng, plan_move, plan_type, Os, PlanMoveOptions, PlanTypeOptions, Vector};

fn main() -> myuri_rs::Result<()> {
    let pts = plan_move(
        Vector::new(50.0, 80.0),
        Vector::new(700.0, 500.0),
        &make_rng(42),
        &PlanMoveOptions {
            os: Some(Os::Macos),
            ..Default::default()
        },
    );
    assert!(!pts.is_empty());

    let keys = plan_type(
        "Hello Camoufox",
        &make_rng(42),
        &PlanTypeOptions {
            os: Some(Os::Macos),
            ..Default::default()
        },
    )?;
    assert!(!keys.is_empty());
    Ok(())
}
```

## Testing

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
