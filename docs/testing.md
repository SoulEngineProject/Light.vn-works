# Testing

## Layout
```
tests/
  rs/                 Rust integration tests (one binary)
    main.rs           declares the modules: `mod works_test; mod smoke_test;`
    works_test.rs     pure-function + content-validation tests (rstest)
    smoke_test.rs     HTTP handler tests (drives build_app via oneshot)
  js/                 client-side logic tests (Node built-in runner)
    search.test.js    tag: search matching (public/search.js)
    view.test.js      year-open cascade (public/view.js)
```

- Cargo only auto-discovers `tests/*.rs`, so the `tests/rs/` root is declared once in `Cargo.toml` (`[[test]] name = "rs"`). Adding a new Rust test file = drop it in `tests/rs/` and add one `mod` line to `main.rs`; no Cargo.toml change.
- Client logic that needs testing lives in its own script (`public/search.js`, `public/view.js`) as a UMD module — the browser gets globals, Node `require`s it. `home.js` consumes those globals. This keeps one source of truth instead of mirroring logic into Rust.

## Running
- Rust: `cargo test`
- JS: `cd tests/js && node --test` (no npm install — uses Node's built-in runner, Node 18+)
- Coverage: `cargo llvm-cov --summary-only` (see below)

Stop the local server first if it's running — `cargo test` rebuilds the binary and can't while the exe is in use.

## Conventions
- Every test body is split into `// given:`, `// when:`, `// then:` sections.
- Rust: parameterized cases use `rstest` (`#[case::name(...)]` — the name shows in `cargo test` output). Shared data comes from `#[fixture]`s.
- JS: each `test(...)` uses `node:assert`; keep the tested logic in a pure function so the DOM isn't needed.

## Coverage
- Tool: `cargo llvm-cov` (LLVM source-based, cross-platform — same numbers on Windows and Linux CI).
- CI gates on it: `cargo llvm-cov --fail-under-lines 74`. The floor is a ratchet — raise it as coverage grows, never lower it to make a change pass.
- `main.rs` reads only `PORT` and boots the server, so it sits near 0% and drags the total down; the meaningful coverage is in `lib.rs` and `app.rs`.

## CI (`.github/workflows/rust.yml`)
Runs on push/PR to main: clippy (`-D warnings`), `cargo audit`, `cargo test`, `node --test`, then the coverage gate. `Swatinem/rust-cache` caches the build; `cargo-llvm-cov` is installed as a prebuilt binary.
