#!/usr/bin/env node
// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Visual Regression Testing Script
// =================================
//
// Captures screenshots of the Vite dev server for visual comparison.
// Requires a running dev server (`npm run dev`) and Puppeteer.
//
// Usage:
//   npm run dev  (in another terminal)
//   node scripts/visual-regression.mjs capture    # Capture new baselines
//   node scripts/visual-regression.mjs compare    # Compare against baselines
//
// Screenshots are saved to tests/visual-regression/screenshots/
// Baselines are stored in tests/visual-regression/baselines/
//
// Note: This is a test infrastructure scaffold. Full pixel-diff comparison
// with pixelmatch is a follow-up (see #415 for details).

import puppeteer from 'puppeteer';
import { mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, '..');
const SCREENSHOT_DIR = join(PROJECT_ROOT, 'tests', 'visual-regression', 'screenshots');
const BASELINE_DIR = join(PROJECT_ROOT, 'tests', 'visual-regression', 'baselines');

const DEV_SERVER_URL = 'http://localhost:1420'; // Vite dev server default port

const PAGES = [
  { name: 'download', path: '/' },
  // Note: In-app routing is handled by React, not URL paths.
  // These capture the default page state.
];

const VIEWPORTS = [
  { name: 'desktop', width: 1280, height: 800 },
  { name: 'compact', width: 900, height: 600 },
];

const THEMES = ['dark', 'light'];

async function captureScreenshots(outputDir) {
  mkdirSync(outputDir, { recursive: true });

  console.log(`Capturing screenshots to: ${outputDir}`);
  console.log(`Dev server: ${DEV_SERVER_URL}`);

  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  try {
    for (const viewport of VIEWPORTS) {
      for (const theme of THEMES) {
        const page = await browser.newPage();
        await page.setViewport({ width: viewport.width, height: viewport.height });

        // Set color scheme preference
        await page.emulateMediaFeatures([
          { name: 'prefers-color-scheme', value: theme },
        ]);

        for (const pageConfig of PAGES) {
          const url = `${DEV_SERVER_URL}${pageConfig.path}`;
          try {
            await page.goto(url, { waitUntil: 'networkidle0', timeout: 10000 });
            // Wait for animations to settle
            await new Promise((r) => setTimeout(r, 1000));

            const filename = `${pageConfig.name}-${viewport.name}-${theme}.png`;
            await page.screenshot({
              path: join(outputDir, filename),
              fullPage: false,
            });
            console.log(`  ✓ ${filename}`);
          } catch (err) {
            console.error(`  ✗ ${pageConfig.name}-${viewport.name}-${theme}: ${err.message}`);
          }
        }

        await page.close();
      }
    }
  } finally {
    await browser.close();
  }
}

/**
 * Compare screenshots against baselines using pixelmatch.
 * Returns the number of screenshots that differ beyond threshold.
 */
async function compareScreenshots() {
  const { default: pixelmatch } = await import('pixelmatch');
  const { PNG } = await import('pngjs');
  const { readFileSync, readdirSync, writeFileSync } = await import('fs');

  if (!existsSync(BASELINE_DIR)) {
    console.error('No baselines found. Run: node scripts/visual-regression.mjs capture --baseline');
    return 1;
  }
  if (!existsSync(SCREENSHOT_DIR)) {
    console.error('No screenshots to compare. Run: node scripts/visual-regression.mjs capture');
    return 1;
  }

  const DIFF_DIR = join(PROJECT_ROOT, 'tests', 'visual-regression', 'diffs');
  mkdirSync(DIFF_DIR, { recursive: true });

  const baselineFiles = readdirSync(BASELINE_DIR).filter((f) => f.endsWith('.png'));
  let failures = 0;
  const THRESHOLD = 0.1; // 0.1 = 10% pixel tolerance

  for (const filename of baselineFiles) {
    const baselinePath = join(BASELINE_DIR, filename);
    const screenshotPath = join(SCREENSHOT_DIR, filename);

    if (!existsSync(screenshotPath)) {
      console.error(`  ✗ ${filename}: no screenshot found (missing)`);
      failures++;
      continue;
    }

    const baselineImg = PNG.sync.read(readFileSync(baselinePath));
    const screenshotImg = PNG.sync.read(readFileSync(screenshotPath));

    const { width, height } = baselineImg;
    if (screenshotImg.width !== width || screenshotImg.height !== height) {
      console.error(`  ✗ ${filename}: dimensions differ (${width}x${height} vs ${screenshotImg.width}x${screenshotImg.height})`);
      failures++;
      continue;
    }

    const diff = new PNG({ width, height });
    const numDiffPixels = pixelmatch(
      baselineImg.data,
      screenshotImg.data,
      diff.data,
      width,
      height,
      { threshold: THRESHOLD }
    );

    const diffPercent = (numDiffPixels / (width * height)) * 100;

    if (diffPercent > 0.1) {
      // Save diff image for inspection
      writeFileSync(join(DIFF_DIR, `diff-${filename}`), PNG.sync.write(diff));
      console.error(`  ✗ ${filename}: ${diffPercent.toFixed(2)}% pixels differ (${numDiffPixels} px)`);
      failures++;
    } else {
      console.log(`  ✓ ${filename}: ${diffPercent.toFixed(3)}% diff (ok)`);
    }
  }

  return failures;
}

// Main entry point
const command = process.argv[2] || 'capture';

if (command === 'capture') {
  const isBaseline = process.argv[3] === '--baseline';
  const dir = isBaseline ? BASELINE_DIR : SCREENSHOT_DIR;
  captureScreenshots(dir)
    .then(() => console.log(`\nDone. Screenshots saved to ${dir}`))
    .catch((err) => {
      console.error('Screenshot capture failed:', err.message);
      console.error('Is the dev server running? (npm run dev)');
      process.exit(1);
    });
} else if (command === 'compare') {
  compareScreenshots()
    .then((failures) => {
      if (failures > 0) {
        console.error(`\n${failures} screenshot(s) differ from baseline!`);
        process.exit(1);
      } else {
        console.log('\nAll screenshots match baselines.');
      }
    })
    .catch((err) => {
      console.error('Comparison failed:', err.message);
      process.exit(1);
    });
} else {
  console.log('Usage: node scripts/visual-regression.mjs [capture|compare] [--baseline]');
}
