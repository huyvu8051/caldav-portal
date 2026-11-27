# Working on caldav-portal

## Do not benchmark

Do not write or run performance measurements unless there is a real
performance problem to diagnose. If a number already exists in a commit
message or a README, cite it -- do not reproduce it to confirm it.

## Verify behaviour, always

A row appearing in a table is not the property that matters. Assert the thing
a user would notice: that a credential logs in, that an old password stops
working, that a second tab does not overwrite the first.

Check that a test bites. Break the line it is about and confirm that test --
and ideally only that test -- goes red.

## Run the gates in CI's order

`cargo fmt --check`, then clippy, then tests, then the wasm build. CI runs fmt
first; running only the later ones locally has already cost two builds on a
formatting diff.

## The design system is shared, not copied-and-forgotten

`src/lib.rs`'s `TAILWIND_CONFIG` and `BASE_STYLE`, and `src/ui.rs`, come from
`webcaldav` and should stay in step with it. `e2e/tests/design-system.spec.ts`
asserts the rules on computed styles, so drift is a failing test rather than a
matter of taste.
