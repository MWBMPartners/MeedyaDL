# Visual Regression Tests

Screenshots and baselines for visual regression testing, written by
`scripts/visual-regression.mjs`. It needs the dev server running (`npm run dev`)
and Puppeteer. Run `node scripts/visual-regression.mjs capture --baseline` to
save baselines, `capture` to take new screenshots, and `compare` to check them
against the baselines pixel by pixel. Images that differ are saved to
`tests/visual-regression/diffs/`.

Screenshots go in `tests/visual-regression/screenshots/` and baselines in
`tests/visual-regression/baselines/`. `capture` creates the folder it writes
to (`screenshots/`, or `baselines/` with `--baseline`). `compare` creates
`diffs/` and saves an image there for each screen whose pixels differ; a
missing screenshot or a size mismatch is reported as a failure without an
image.

See #415 for details.
