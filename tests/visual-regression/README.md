# Visual Regression Tests

Screenshots and baselines for visual regression testing, written by
`scripts/visual-regression.mjs`. It needs the dev server running (`npm run dev`)
and Puppeteer. Run `node scripts/visual-regression.mjs capture` to save
baselines and `... compare` to check against them.

Screenshots go in `tests/visual-regression/screenshots/` and baselines in
`tests/visual-regression/baselines/`; both folders are created when the script
runs. The script is still a starting point: it captures the screens but does
not yet compare them pixel by pixel.

See #415 for details.
