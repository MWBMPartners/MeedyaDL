#!/usr/bin/env node
// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Icon Generator — renders icon.svg to all platform icon formats
// ================================================================
//
// Outputs (in assets/brand/new/):
//   icon.png              — 1024x1024 static PNG with alpha
//   icon.ico              — Windows multi-size ICO (16-256px)
//   favicon.ico           — Web favicon ICO (16/32/48px)
//   icon.icns             — macOS ICNS via iconutil
//   icon-liquidglass.png  — Apple Liquid Glass (1024px, 10% inset)
//   icon-liquidglass.icns — Apple Liquid Glass ICNS
//
// Usage: node scripts/generate-icons.mjs
// Requires: puppeteer, Pillow (pip3), iconutil (macOS)

import puppeteer from 'puppeteer';
import { execSync, execFileSync } from 'child_process';
import { mkdirSync, rmSync, existsSync, readFileSync, writeFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const BRAND_DIR = resolve(ROOT, 'assets/brand/new');
const TEMP_DIR = resolve(ROOT, '.icon-temp');
const SVG_FILE = resolve(BRAND_DIR, 'icon.svg');

const ICO_SIZES = [16, 32, 48, 64, 128, 256];
const FAVICON_SIZES = [16, 32, 48];
const ALL_SIZES = [16, 32, 48, 64, 128, 256, 512, 1024];

async function main() {
  console.log('Icon Generator');
  console.log('==============\n');

  if (existsSync(TEMP_DIR)) rmSync(TEMP_DIR, { recursive: true });
  mkdirSync(TEMP_DIR, { recursive: true });

  const svgContent = readFileSync(SVG_FILE, 'utf-8');
  const browser = await puppeteer.launch({ headless: true, args: ['--no-sandbox'] });

  // Render at all sizes
  console.log('Rendering sizes...');
  for (const size of ALL_SIZES) {
    const page = await browser.newPage();
    await page.setViewport({ width: size, height: size, deviceScaleFactor: 1 });
    await page.setContent(`<!DOCTYPE html><html><head><style>*{margin:0;padding:0}body{background:transparent;width:${size}px;height:${size}px;overflow:hidden}svg{width:${size}px;height:${size}px}</style></head><body>${svgContent}</body></html>`, { waitUntil: 'domcontentloaded', timeout: 10000 });
    await new Promise((r) => setTimeout(r, 500));
    await page.screenshot({ path: resolve(TEMP_DIR, `icon-${size}.png`), type: 'png', omitBackground: true });
    await page.close();
    process.stdout.write(`  ${size}px`);
  }
  console.log('\n');
  await browser.close();

  const src = (s) => resolve(TEMP_DIR, `icon-${s}.png`);

  // 1. icon.png
  execSync(`cp "${src(1024)}" "${resolve(BRAND_DIR, 'icon.png')}"`);
  logFile('icon.png');

  // 2. icon.ico
  genICO('icon.ico', ICO_SIZES);

  // 3. favicon.ico
  genICO('favicon.ico', FAVICON_SIZES);

  // 4. icon.icns
  genICNS('icon.icns', false);

  // 5. Liquid Glass icon.png
  genLiquidGlass('icon-liquidglass.png', 1024, 0.10);

  // 6. Liquid Glass .icns
  genLiquidGlassICNS('icon-liquidglass.icns', 0.10);

  // Cleanup
  if (existsSync(TEMP_DIR)) rmSync(TEMP_DIR, { recursive: true });
  console.log('\nDone!');

  function genICO(name, sizes) {
    const out = resolve(BRAND_DIR, name);
    const pyScript = resolve(TEMP_DIR, '_gen_ico.py');
    // Pillow ICO: the first image's save() with sizes= param generates
    // the multi-resolution ICO. We open the largest, then pass sizes.
    const largest = Math.max(...sizes);
    const sizesTuples = sizes.map((s) => `(${s},${s})`).join(',');
    const code = `
from PIL import Image
img = Image.open('${src(largest)}')
img.save('${out}', format='ICO', sizes=[${sizesTuples}])
`;
    writeFileSync(pyScript, code);
    execSync(`python3 "${pyScript}"`, { stdio: 'pipe' });
    logFile(name, `sizes: ${sizes.join(', ')}`);
  }

  function genICNS(name, liquidGlass) {
    const iconsetDir = resolve(TEMP_DIR, name.replace('.icns', '.iconset'));
    if (existsSync(iconsetDir)) rmSync(iconsetDir, { recursive: true });
    mkdirSync(iconsetDir, { recursive: true });
    const map = [
      ['icon_16x16.png', 16], ['icon_16x16@2x.png', 32],
      ['icon_32x32.png', 32], ['icon_32x32@2x.png', 64],
      ['icon_128x128.png', 128], ['icon_128x128@2x.png', 256],
      ['icon_256x256.png', 256], ['icon_256x256@2x.png', 512],
      ['icon_512x512.png', 512], ['icon_512x512@2x.png', 1024],
    ];
    for (const [fname, size] of map) {
      execSync(`cp "${src(size)}" "${resolve(iconsetDir, fname)}"`);
    }
    const out = resolve(BRAND_DIR, name);
    execFileSync('iconutil', ['-c', 'icns', iconsetDir, '-o', out]);
    logFile(name);
  }

  function genLiquidGlass(name, size, insetRatio) {
    const out = resolve(BRAND_DIR, name);
    const inset = Math.round(size * insetRatio);
    const content = size - inset * 2;
    execSync(`python3 -c "
from PIL import Image
icon = Image.open('${src(size)}')
resized = icon.resize((${content},${content}), Image.LANCZOS)
canvas = Image.new('RGBA', (${size},${size}), (0,0,0,0))
canvas.paste(resized, (${inset},${inset}), resized)
canvas.save('${out}', 'PNG')
"`, { stdio: 'pipe' });
    logFile(name, `${size}px, ${Math.round(insetRatio * 100)}% inset`);
  }

  function genLiquidGlassICNS(name, insetRatio) {
    const iconsetDir = resolve(TEMP_DIR, name.replace('.icns', '.iconset'));
    if (existsSync(iconsetDir)) rmSync(iconsetDir, { recursive: true });
    mkdirSync(iconsetDir, { recursive: true });
    const map = [
      ['icon_16x16.png', 16], ['icon_16x16@2x.png', 32],
      ['icon_32x32.png', 32], ['icon_32x32@2x.png', 64],
      ['icon_128x128.png', 128], ['icon_128x128@2x.png', 256],
      ['icon_256x256.png', 256], ['icon_256x256@2x.png', 512],
      ['icon_512x512.png', 512], ['icon_512x512@2x.png', 1024],
    ];
    const entries = map.map(([f, s]) => `('${f}',${s})`).join(',');
    execSync(`python3 -c "
from PIL import Image
import os
entries = [${entries}]
for fname, size in entries:
    icon = Image.open('${TEMP_DIR}/icon-' + str(size) + '.png')
    inset = max(1, int(size * ${insetRatio}))
    content = size - inset * 2
    resized = icon.resize((content, content), Image.LANCZOS)
    canvas = Image.new('RGBA', (size, size), (0,0,0,0))
    canvas.paste(resized, (inset, inset), resized)
    canvas.save(os.path.join('${iconsetDir}', fname), 'PNG')
"`, { stdio: 'pipe' });
    const out = resolve(BRAND_DIR, name);
    execFileSync('iconutil', ['-c', 'icns', iconsetDir, '-o', out]);
    logFile(name);
  }

  function logFile(name, extra) {
    const p = resolve(BRAND_DIR, name);
    const kb = Math.round(readFileSync(p).length / 1024);
    console.log(`  ${name} (${kb} KB)${extra ? ` — ${extra}` : ''}`);
  }
}

main().catch((e) => { console.error('Fatal:', e); process.exit(1); });
