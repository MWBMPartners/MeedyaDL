# Visual Regression Tests

Screenshots and baselines for visual regression testing, written by
`scripts/visual-regression.mjs`. It needs the dev server running (`npm run dev`)
and Puppeteer. Run `node scripts/visual-regression.mjs capture --baseline` to
save baselines, `capture` to take new screenshots, and `compare` to check them
against the baselines pixel by pixel. Images that differ are saved to
`tests/visual-regression/diffs/`.

Screenshots go in `tests/visual-regression/screenshots/` and baselines in
`tests/visual-regression/baselines/`; both folders are created when the script
runs. `compare` creates `diffs/` and saves an image there for each screen that differs.

See #415 for details.
