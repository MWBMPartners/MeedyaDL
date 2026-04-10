#!/usr/bin/env node
// Copyright (c) 2026 MeedyaDL
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
  console.log('Pixel-diff comparison not yet implemented.');
  console.log('See #415 for planned pixelmatch integration.');
  // Future: compare SCREENSHOT_DIR against BASELINE_DIR using pixelmatch
  if (!existsSync(BASELINE_DIR)) {
    console.log('No baselines found. Run: node scripts/visual-regression.mjs capture --baseline');
  }
} else {
  console.log('Usage: node scripts/visual-regression.mjs [capture|compare] [--baseline]');
}
