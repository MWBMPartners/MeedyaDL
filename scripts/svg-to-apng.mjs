#!/usr/bin/env node
// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// SVG to APNG Converter
// ======================
// Renders animated SVG files frame-by-frame in a headless browser,
// then assembles the frames into an APNG (Animated PNG) with full
// alpha transparency.
//
// Usage:
//   node scripts/svg-to-apng.mjs
//
// Requirements:
//   - puppeteer (bundled with project via npx)
//   - ffmpeg (for APNG assembly)
//
// Output:
//   assets/brand/new/logo.apng
//   assets/brand/new/logotype.apng

import puppeteer from 'puppeteer';
import { execFileSync } from 'child_process';
import { mkdirSync, rmSync, existsSync, readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const BRAND_DIR = resolve(ROOT, 'assets/brand/new');
const TEMP_DIR = resolve(ROOT, '.apng-frames');

// ── Configuration ─────────────────────────────────────────────
const CONFIGS = [
  {
    name: 'logo',
    svgFile: resolve(BRAND_DIR, 'logo.svg'),
    outputFile: resolve(BRAND_DIR, 'logo.apng'),
    width: 512,
    height: 512,
    fps: 15,
    // Full cycle = 8s crossfade. Capture one complete cycle.
    durationSeconds: 8,
  },
  {
    name: 'logotype',
    svgFile: resolve(BRAND_DIR, 'logotype.svg'),
    outputFile: resolve(BRAND_DIR, 'logotype.apng'),
    width: 600,
    height: 130,
    fps: 15,
    // Gradient shimmer cycle = 4s. Capture two cycles for smoothness.
    durationSeconds: 8,
  },
];

// ── Main ──────────────────────────────────────────────────────
async function main() {
  console.log('SVG to APNG Converter');
  console.log('=====================\n');

  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  for (const config of CONFIGS) {
    console.log(`\n── ${config.name} ──`);
    console.log(`  SVG: ${config.svgFile}`);
    console.log(`  Size: ${config.width}x${config.height}`);
    console.log(`  FPS: ${config.fps}`);
    console.log(`  Duration: ${config.durationSeconds}s`);

    const frameDir = resolve(TEMP_DIR, config.name);
    if (existsSync(frameDir)) rmSync(frameDir, { recursive: true });
    mkdirSync(frameDir, { recursive: true });

    const totalFrames = config.fps * config.durationSeconds;
    const frameDelayMs = 1000 / config.fps;
    console.log(`  Total frames: ${totalFrames}`);

    // Read the SVG content
    const svgContent = readFileSync(config.svgFile, 'utf-8');

    // Create a page with the SVG
    const page = await browser.newPage();
    await page.setViewport({
      width: config.width,
      height: config.height,
      deviceScaleFactor: 1,
    });

    // Load SVG as an HTML page with transparent background
    const html = `<!DOCTYPE html>
<html>
<head>
  <style>
    * { margin: 0; padding: 0; }
    body { background: transparent; width: ${config.width}px; height: ${config.height}px; overflow: hidden; }
    svg { width: ${config.width}px; height: ${config.height}px; }
  </style>
</head>
<body>${svgContent}</body>
</html>`;

    await page.setContent(html, { waitUntil: 'networkidle0' });

    // Wait a moment for fonts to load and animations to start
    await new Promise((r) => setTimeout(r, 500));

    // Capture frames
    console.log('  Capturing frames...');
    for (let i = 0; i < totalFrames; i++) {
      const framePath = resolve(frameDir, `frame_${String(i).padStart(4, '0')}.png`);
      await page.screenshot({
        path: framePath,
        type: 'png',
        omitBackground: true, // Transparent background
        clip: {
          x: 0,
          y: 0,
          width: config.width,
          height: config.height,
        },
      });

      // Wait for next frame
      if (i < totalFrames - 1) {
        await new Promise((r) => setTimeout(r, frameDelayMs));
      }

      // Progress indicator
      if ((i + 1) % 20 === 0 || i === totalFrames - 1) {
        process.stdout.write(`\r  Captured ${i + 1}/${totalFrames} frames`);
      }
    }
    console.log('');

    await page.close();

    // Assemble APNG using ffmpeg (execFileSync handles spaces in paths)
    console.log('  Assembling APNG...');
    const ffmpegArgs = [
      '-y',
      '-framerate', String(config.fps),
      '-i', resolve(frameDir, 'frame_%04d.png'),
      '-plays', '0',           // Loop forever
      '-f', 'apng',
      config.outputFile,
    ];

    try {
      execFileSync('ffmpeg', ffmpegArgs, { stdio: 'pipe' });
      const stat = existsSync(config.outputFile)
        ? Math.round(
            readFileSync(config.outputFile).length / 1024
          )
        : 0;
      console.log(`  Output: ${config.outputFile} (${stat} KB)`);
    } catch (err) {
      console.error(`  ffmpeg failed: ${err.stderr?.toString() || err.message}`);
    }
  }

  await browser.close();

  // Clean up temp frames
  if (existsSync(TEMP_DIR)) {
    rmSync(TEMP_DIR, { recursive: true });
    console.log('\n  Cleaned up temp frames.');
  }

  console.log('\nDone!');
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
