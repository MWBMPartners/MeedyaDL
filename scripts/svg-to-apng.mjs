#!/usr/bin/env node
// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// SVG to Animated PNG Converter (with mode variants)
// ====================================================
// Renders animated SVG files frame-by-frame in a headless browser,
// then assembles the frames into APNG files with full alpha transparency.
//
// Generates all colour mode variants for both logo and wordtype:
//   - default (light), dark, cb-deutan, cb-protan, cb-tritan,
//     cb-deutan-dark, cb-protan-dark, cb-tritan-dark
//
// Output uses .png extension for compatibility (files are APNG format).
//
// Usage:
//   node scripts/svg-to-apng.mjs
//
// Requirements:
//   - puppeteer (npm install --save-dev puppeteer)
//   - ffmpeg (brew install ffmpeg)

import puppeteer from 'puppeteer';
import { execFileSync } from 'child_process';
import { mkdirSync, rmSync, existsSync, readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const BRAND_DIR = resolve(ROOT, 'assets/brand');
const TEMP_DIR = resolve(ROOT, '.apng-frames');

// ── Modes to generate ─────────────────────────────────────────
const MODES = [
  { suffix: '',               mode: null,              label: 'default (light)' },
  { suffix: '-dark',          mode: 'dark',            label: 'dark' },
  { suffix: '-cb-deutan',     mode: 'cb-deutan',       label: 'CB deuteranopia' },
  { suffix: '-cb-protan',     mode: 'cb-protan',       label: 'CB protanopia' },
  { suffix: '-cb-tritan',     mode: 'cb-tritan',       label: 'CB tritanopia' },
  { suffix: '-cb-deutan-dark', mode: 'cb-deutan-dark', label: 'CB deuteranopia (dark)' },
  { suffix: '-cb-protan-dark', mode: 'cb-protan-dark', label: 'CB protanopia (dark)' },
  { suffix: '-cb-tritan-dark', mode: 'cb-tritan-dark', label: 'CB tritanopia (dark)' },
];

// ── SVG sources ───────────────────────────────────────────────
const SVGS = [
  {
    name: 'logo',
    svgFile: resolve(BRAND_DIR, 'logo.svg'),
    width: 512,
    height: 512,
    fps: 15,
    durationSeconds: 8,
  },
  {
    name: 'wordtype',
    svgFile: resolve(BRAND_DIR, 'wordtype.svg'),
    width: 600,
    height: 130,
    fps: 15,
    durationSeconds: 8,
  },
];

// ── Main ──────────────────────────────────────────────────────
async function main() {
  console.log('SVG to Animated PNG Converter');
  console.log('==============================');
  console.log(`Generating ${SVGS.length} SVGs x ${MODES.length} modes = ${SVGS.length * MODES.length} files\n`);

  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  for (const svgConfig of SVGS) {
    console.log(`\n━━ ${svgConfig.name.toUpperCase()} ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);

    const svgContent = readFileSync(svgConfig.svgFile, 'utf-8');
    const totalFrames = svgConfig.fps * svgConfig.durationSeconds;
    const frameDelayMs = 1000 / svgConfig.fps;

    for (const modeConfig of MODES) {
      const outputName = `${svgConfig.name}${modeConfig.suffix}.png`;
      const outputFile = resolve(BRAND_DIR, outputName);
      const frameDir = resolve(TEMP_DIR, `${svgConfig.name}${modeConfig.suffix}`);

      console.log(`\n  ${outputName} (${modeConfig.label})`);

      if (existsSync(frameDir)) rmSync(frameDir, { recursive: true });
      mkdirSync(frameDir, { recursive: true });

      // Create page
      const page = await browser.newPage();
      await page.setViewport({
        width: svgConfig.width,
        height: svgConfig.height,
        deviceScaleFactor: 1,
      });

      // Build HTML with mode applied via inline script (modeConfig.mode
      // is referenced directly in the <script> block below, not via a
      // query param on the SVG — kept as inline JS for Puppeteer's
      // `setContent` flow).
      const html = `<!DOCTYPE html>
<html>
<head>
  <style>
    * { margin: 0; padding: 0; }
    body { background: transparent; width: ${svgConfig.width}px; height: ${svgConfig.height}px; overflow: hidden; }
    svg { width: ${svgConfig.width}px; height: ${svgConfig.height}px; }
  </style>
</head>
<body>${svgContent}</body>
<script>
  // Apply mode via the SVG's own script mechanism
  ${modeConfig.mode ? `
  (function() {
    var svg = document.querySelector('svg');
    if (!svg) return;
    var mode = '${modeConfig.mode}';
    if (mode.indexOf('-dark') > 0 && mode.indexOf('cb-') === 0) {
      svg.classList.add('dark');
      svg.classList.add(mode.replace('-dark', ''));
    } else {
      svg.classList.add(mode);
    }
    // Also set inline styles as fallback
    var style = svg.style;
    ${generateInlineStyleJS(svgConfig.name, modeConfig.mode)}
  })();
  ` : ''}
</script>
</html>`;

      await page.setContent(html, { waitUntil: 'domcontentloaded', timeout: 10000 });
      // Wait for fonts to load and animations to start
      await new Promise((r) => setTimeout(r, 2000));

      // Measure content bounds for trimming
      const bounds = await page.evaluate(() => {
        const svg = document.querySelector('svg');
        if (!svg) return null;
        const els = svg.querySelectorAll('circle,path,line,rect,ellipse,text,polyline');
        let minX = 9999, minY = 9999, maxX = 0, maxY = 0;
        els.forEach((el) => {
          try {
            const r = el.getBoundingClientRect();
            if (r.width > 0 && r.height > 0) {
              minX = Math.min(minX, r.x);
              minY = Math.min(minY, r.y);
              maxX = Math.max(maxX, r.x + r.width);
              maxY = Math.max(maxY, r.y + r.height);
            }
          } catch (e) {}
        });
        if (minX >= maxX) return null;
        return { minX, minY, maxX, maxY };
      });

      let clipRegion = { x: 0, y: 0, width: svgConfig.width, height: svgConfig.height };
      if (bounds) {
        const pad = 4;
        clipRegion = {
          x: Math.max(0, Math.floor(bounds.minX) - pad),
          y: Math.max(0, Math.floor(bounds.minY) - pad),
          width: Math.min(svgConfig.width, Math.ceil(bounds.maxX - bounds.minX) + pad * 2),
          height: Math.min(svgConfig.height, Math.ceil(bounds.maxY - bounds.minY) + pad * 2),
        };
        console.log(`    Trimmed: ${clipRegion.width}x${clipRegion.height}`);
      }

      // Capture frames
      for (let i = 0; i < totalFrames; i++) {
        const framePath = resolve(frameDir, `frame_${String(i).padStart(4, '0')}.png`);
        await page.screenshot({
          path: framePath,
          type: 'png',
          omitBackground: true,
          clip: clipRegion,
        });
        if (i < totalFrames - 1) {
          await new Promise((r) => setTimeout(r, frameDelayMs));
        }
        if ((i + 1) % 30 === 0 || i === totalFrames - 1) {
          process.stdout.write(`\r    Captured ${i + 1}/${totalFrames} frames`);
        }
      }
      console.log('');

      await page.close();

      // Assemble APNG
      try {
        execFileSync('ffmpeg', [
          '-y',
          '-framerate', String(svgConfig.fps),
          '-i', resolve(frameDir, 'frame_%04d.png'),
          '-plays', '0',
          '-f', 'apng',
          outputFile,
        ], { stdio: 'pipe' });

        const sizeKB = Math.round(readFileSync(outputFile).length / 1024);
        console.log(`    Output: ${outputName} (${sizeKB} KB)`);
      } catch (err) {
        console.error(`    ffmpeg failed: ${err.stderr?.toString().split('\n').pop()}`);
      }
    }
  }

  await browser.close();

  // Clean up
  if (existsSync(TEMP_DIR)) {
    rmSync(TEMP_DIR, { recursive: true });
    console.log('\nCleaned up temp frames.');
  }

  console.log('\nDone!');
}

// Generate inline style.setProperty() calls for a given SVG + mode
function generateInlineStyleJS(svgName, mode) {
  const LOGO_MODES = {
    'dark': {'--logo-primary':'#E2E8F0','--logo-secondary':'#F8FAFC','--logo-accent':'#CBD5E1','--logo-disc':'#94A3B8','--logo-disc-highlight':'#CBD5E1','--logo-vinyl':'#64748B','--logo-vinyl-groove':'#7A8A9E','--logo-vinyl-groove-alt':'#94A3B8','--logo-shadow':'rgba(0,0,0,0.7)','--logo-glow':'rgba(226,232,240,0.15)'},
    'cb-deutan': {'--logo-primary':'#648FFF','--logo-secondary':'#FFB000','--logo-accent':'#785EF0','--logo-disc':'#7B8FAA','--logo-disc-highlight':'#9BADC4','--logo-vinyl':'#5A6E87','--logo-vinyl-groove':'#6A7E97','--logo-vinyl-groove-alt':'#7B8FAA'},
    'cb-protan': {'--logo-primary':'#648FFF','--logo-secondary':'#DC267F','--logo-accent':'#FFB000','--logo-disc':'#7B8FAA','--logo-disc-highlight':'#9BADC4','--logo-vinyl':'#5A6E87','--logo-vinyl-groove':'#6A7E97','--logo-vinyl-groove-alt':'#7B8FAA'},
    'cb-tritan': {'--logo-primary':'#D55E00','--logo-secondary':'#CC79A7','--logo-accent':'#D55E00','--logo-disc':'#7B8FAA','--logo-disc-highlight':'#9BADC4','--logo-vinyl':'#5A6E87','--logo-vinyl-groove':'#6A7E97','--logo-vinyl-groove-alt':'#7B8FAA'},
    'cb-deutan-dark': {'--logo-primary':'#85AAFF','--logo-secondary':'#FFC940','--logo-accent':'#9B85FF','--logo-disc':'#8A9AB5','--logo-disc-highlight':'#B8C5D8','--logo-vinyl':'#5A6A82','--logo-vinyl-groove':'#6E7E96','--logo-vinyl-groove-alt':'#8A9AB5','--logo-shadow':'rgba(0,0,0,0.7)'},
    'cb-protan-dark': {'--logo-primary':'#85AAFF','--logo-secondary':'#FF5CA1','--logo-accent':'#FFC940','--logo-disc':'#8A9AB5','--logo-disc-highlight':'#B8C5D8','--logo-vinyl':'#5A6A82','--logo-vinyl-groove':'#6E7E96','--logo-vinyl-groove-alt':'#8A9AB5','--logo-shadow':'rgba(0,0,0,0.7)'},
    'cb-tritan-dark': {'--logo-primary':'#FF8533','--logo-secondary':'#E09DBF','--logo-accent':'#FF8533','--logo-disc':'#8A9AB5','--logo-disc-highlight':'#B8C5D8','--logo-vinyl':'#5A6A82','--logo-vinyl-groove':'#6E7E96','--logo-vinyl-groove-alt':'#8A9AB5','--logo-shadow':'rgba(0,0,0,0.7)'},
  };
  const WORDTYPE_MODES = {
    'dark': {'--wordtype-primary':'#E2E8F0','--wordtype-secondary':'#F8FAFC','--wordtype-accent':'#CBD5E1','--wordtype-glow':'#E2E8F0','--wordtype-shadow':'rgba(0,0,0,0.7)'},
    'cb-deutan': {'--wordtype-primary':'#648FFF','--wordtype-secondary':'#FFB000','--wordtype-accent':'#785EF0','--wordtype-glow':'#648FFF'},
    'cb-protan': {'--wordtype-primary':'#648FFF','--wordtype-secondary':'#DC267F','--wordtype-accent':'#FFB000','--wordtype-glow':'#648FFF'},
    'cb-tritan': {'--wordtype-primary':'#D55E00','--wordtype-secondary':'#CC79A7','--wordtype-accent':'#D55E00','--wordtype-glow':'#CC79A7'},
    'cb-deutan-dark': {'--wordtype-primary':'#85AAFF','--wordtype-secondary':'#FFC940','--wordtype-accent':'#9B85FF','--wordtype-glow':'#85AAFF','--wordtype-shadow':'rgba(0,0,0,0.7)'},
    'cb-protan-dark': {'--wordtype-primary':'#85AAFF','--wordtype-secondary':'#FF5CA1','--wordtype-accent':'#FFC940','--wordtype-glow':'#85AAFF','--wordtype-shadow':'rgba(0,0,0,0.7)'},
    'cb-tritan-dark': {'--wordtype-primary':'#FF8533','--wordtype-secondary':'#E09DBF','--wordtype-accent':'#FF8533','--wordtype-glow':'#E09DBF','--wordtype-shadow':'rgba(0,0,0,0.7)'},
  };

  const modes = svgName === 'logo' ? LOGO_MODES : WORDTYPE_MODES;
  const props = modes[mode];
  if (!props) return '';

  return Object.entries(props)
    .map(([k, v]) => `style.setProperty('${k}', '${v}');`)
    .join('\n    ');
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
