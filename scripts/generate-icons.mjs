#!/usr/bin/env node
// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Icon Generator — renders icon.svg in all modes and platform formats
// =====================================================================
//
// For each mode (light, dark, 3xCB light, 3xCB dark) generates:
//   icon[-mode].png, icon[-mode].ico, favicon[-mode].ico,
//   icon[-mode].icns, icon[-mode]-liquidglass.png,
//   icon[-mode]-liquidglass.icns
//
// Usage: node scripts/generate-icons.mjs

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

const MODES = [
  { suffix: '',               mode: null,              label: 'default (light)' },
  { suffix: '-dark',          mode: 'dark',            label: 'dark' },
  { suffix: '-cb-deutan',     mode: 'cb-deutan',       label: 'CB deuteranopia' },
  { suffix: '-cb-protan',     mode: 'cb-protan',       label: 'CB protanopia' },
  { suffix: '-cb-tritan',     mode: 'cb-tritan',       label: 'CB tritanopia' },
  { suffix: '-cb-deutan-dark', mode: 'cb-deutan-dark', label: 'CB deutan (dark)' },
  { suffix: '-cb-protan-dark', mode: 'cb-protan-dark', label: 'CB protan (dark)' },
  { suffix: '-cb-tritan-dark', mode: 'cb-tritan-dark', label: 'CB tritan (dark)' },
];

// Icon CSS variable overrides per mode (matching icon.svg's palette)
const MODE_STYLES = {
  'dark': {'--logo-disc':'#94A3B8','--logo-disc-highlight':'#CBD5E1','--logo-vinyl':'#64748B','--logo-vinyl-groove':'#7A8A9E','--logo-vinyl-groove-alt':'#94A3B8','--logo-accent':'#CBD5E1','--logo-secondary':'#F8FAFC','--logo-shadow':'rgba(0,0,0,0.7)'},
  'cb-deutan': {'--logo-disc':'#7B8FAA','--logo-disc-highlight':'#9BADC4','--logo-vinyl':'#5A6E87','--logo-vinyl-groove':'#6A7E97','--logo-vinyl-groove-alt':'#7B8FAA','--logo-accent':'#785EF0','--logo-secondary':'#FFB000'},
  'cb-protan': {'--logo-disc':'#7B8FAA','--logo-disc-highlight':'#9BADC4','--logo-vinyl':'#5A6E87','--logo-vinyl-groove':'#6A7E97','--logo-vinyl-groove-alt':'#7B8FAA','--logo-accent':'#FFB000','--logo-secondary':'#DC267F'},
  'cb-tritan': {'--logo-disc':'#7B8FAA','--logo-disc-highlight':'#9BADC4','--logo-vinyl':'#5A6E87','--logo-vinyl-groove':'#6A7E97','--logo-vinyl-groove-alt':'#7B8FAA','--logo-accent':'#D55E00','--logo-secondary':'#CC79A7'},
  'cb-deutan-dark': {'--logo-disc':'#8A9AB5','--logo-disc-highlight':'#B8C5D8','--logo-vinyl':'#5A6A82','--logo-vinyl-groove':'#6E7E96','--logo-vinyl-groove-alt':'#8A9AB5','--logo-accent':'#9B85FF','--logo-secondary':'#FFC940','--logo-shadow':'rgba(0,0,0,0.7)'},
  'cb-protan-dark': {'--logo-disc':'#8A9AB5','--logo-disc-highlight':'#B8C5D8','--logo-vinyl':'#5A6A82','--logo-vinyl-groove':'#6E7E96','--logo-vinyl-groove-alt':'#8A9AB5','--logo-accent':'#FFC940','--logo-secondary':'#FF5CA1','--logo-shadow':'rgba(0,0,0,0.7)'},
  'cb-tritan-dark': {'--logo-disc':'#8A9AB5','--logo-disc-highlight':'#B8C5D8','--logo-vinyl':'#5A6A82','--logo-vinyl-groove':'#6E7E96','--logo-vinyl-groove-alt':'#8A9AB5','--logo-accent':'#FF8533','--logo-secondary':'#E09DBF','--logo-shadow':'rgba(0,0,0,0.7)'},
};

async function main() {
  console.log('Icon Generator (all modes)');
  console.log('===========================');
  console.log(`${MODES.length} modes x 6 formats = ${MODES.length * 6} files\n`);

  if (existsSync(TEMP_DIR)) rmSync(TEMP_DIR, { recursive: true });
  mkdirSync(TEMP_DIR, { recursive: true });

  const svgContent = readFileSync(SVG_FILE, 'utf-8');
  const browser = await puppeteer.launch({ headless: true, args: ['--no-sandbox'] });

  for (const modeConfig of MODES) {
    const s = modeConfig.suffix;
    const modeDir = resolve(TEMP_DIR, `mode${s || '-default'}`);
    mkdirSync(modeDir, { recursive: true });

    console.log(`\n── icon${s} (${modeConfig.label}) ──`);

    // Build inline style overrides
    const styleOverrides = modeConfig.mode && MODE_STYLES[modeConfig.mode]
      ? Object.entries(MODE_STYLES[modeConfig.mode]).map(([k,v]) => `svg.style.setProperty('${k}','${v}');`).join('')
      : '';

    // Render at all sizes
    process.stdout.write('  Rendering: ');
    for (const size of ALL_SIZES) {
      const page = await browser.newPage();
      await page.setViewport({ width: size, height: size, deviceScaleFactor: 1 });
      const html = `<!DOCTYPE html><html><head><style>*{margin:0;padding:0}body{background:transparent;width:${size}px;height:${size}px;overflow:hidden}svg{width:${size}px;height:${size}px}</style></head><body>${svgContent}</body><script>(function(){var svg=document.querySelector('svg');if(!svg)return;${styleOverrides}})()</script></html>`;
      await page.setContent(html, { waitUntil: 'domcontentloaded', timeout: 10000 });
      await new Promise((r) => setTimeout(r, 300));
      await page.screenshot({ path: resolve(modeDir, `${size}.png`), type: 'png', omitBackground: true });
      await page.close();
      process.stdout.write(`${size} `);
    }
    console.log('');

    const src = (sz) => resolve(modeDir, `${sz}.png`);

    // icon.png
    execSync(`cp "${src(1024)}" "${resolve(BRAND_DIR, `icon${s}.png`)}"`);
    logFile(`icon${s}.png`);

    // icon.ico
    genICO(`icon${s}.ico`, ICO_SIZES, src);

    // favicon.ico
    genICO(`favicon${s}.ico`, FAVICON_SIZES, src);

    // icon.icns
    genICNS(`icon${s}.icns`, src);

    // Liquid Glass .png
    genLiquidGlass(`icon${s}-liquidglass.png`, 1024, 0.10, src);

    // Liquid Glass .icns
    genLiquidGlassICNS(`icon${s}-liquidglass.icns`, 0.10, src);
  }

  await browser.close();
  if (existsSync(TEMP_DIR)) rmSync(TEMP_DIR, { recursive: true });
  console.log('\nDone!');

  // ── Helper functions ────────────────────────────────────────

  function genICO(name, sizes, src) {
    const out = resolve(BRAND_DIR, name);
    const pyScript = resolve(TEMP_DIR, '_ico.py');
    const largest = Math.max(...sizes);
    const sizesTuples = sizes.map((sz) => `(${sz},${sz})`).join(',');
    writeFileSync(pyScript, `from PIL import Image\nImage.open('${src(largest)}').save('${out}', format='ICO', sizes=[${sizesTuples}])\n`);
    execSync(`python3 "${pyScript}"`, { stdio: 'pipe' });
    logFile(name);
  }

  function genICNS(name, src) {
    const dir = resolve(TEMP_DIR, name.replace('.icns', '.iconset'));
    if (existsSync(dir)) rmSync(dir, { recursive: true });
    mkdirSync(dir, { recursive: true });
    const map = [[16,16],[32,32],[32,64],[64,128],[128,256],[256,512],[512,1024]];
    const names = ['icon_16x16','icon_16x16@2x','icon_32x32','icon_32x32@2x','icon_128x128','icon_128x128@2x','icon_256x256','icon_256x256@2x','icon_512x512','icon_512x512@2x'];
    const entries = [[16,'icon_16x16'],[32,'icon_16x16@2x'],[32,'icon_32x32'],[64,'icon_32x32@2x'],[128,'icon_128x128'],[256,'icon_128x128@2x'],[256,'icon_256x256'],[512,'icon_256x256@2x'],[512,'icon_512x512'],[1024,'icon_512x512@2x']];
    for (const [sz, fname] of entries) {
      execSync(`cp "${src(sz)}" "${resolve(dir, fname + '.png')}"`);
    }
    const out = resolve(BRAND_DIR, name);
    execFileSync('iconutil', ['-c', 'icns', dir, '-o', out]);
    logFile(name);
  }

  function genLiquidGlass(name, size, insetRatio, src) {
    const out = resolve(BRAND_DIR, name);
    const inset = Math.round(size * insetRatio);
    const content = size - inset * 2;
    const pyScript = resolve(TEMP_DIR, '_lg.py');
    writeFileSync(pyScript, `from PIL import Image\nicon=Image.open('${src(size)}')\nresized=icon.resize((${content},${content}),Image.LANCZOS)\ncanvas=Image.new('RGBA',(${size},${size}),(0,0,0,0))\ncanvas.paste(resized,(${inset},${inset}),resized)\ncanvas.save('${out}','PNG')\n`);
    execSync(`python3 "${pyScript}"`, { stdio: 'pipe' });
    logFile(name);
  }

  function genLiquidGlassICNS(name, insetRatio, src) {
    const dir = resolve(TEMP_DIR, name.replace('.icns', '.iconset'));
    if (existsSync(dir)) rmSync(dir, { recursive: true });
    mkdirSync(dir, { recursive: true });
    const entries = [[16,'icon_16x16'],[32,'icon_16x16@2x'],[32,'icon_32x32'],[64,'icon_32x32@2x'],[128,'icon_128x128'],[256,'icon_128x128@2x'],[256,'icon_256x256'],[512,'icon_256x256@2x'],[512,'icon_512x512'],[1024,'icon_512x512@2x']];
    const pyScript = resolve(TEMP_DIR, '_lgicns.py');
    const pyEntries = entries.map(([sz, fn]) => `(${sz},'${fn}')`).join(',');
    writeFileSync(pyScript, `
from PIL import Image
import os
for sz, fn in [${pyEntries}]:
    icon = Image.open('${resolve(TEMP_DIR, 'mode-placeholder')}/' + str(sz) + '.png')
    inset = max(1, int(sz * ${insetRatio}))
    content = sz - inset * 2
    resized = icon.resize((content, content), Image.LANCZOS)
    canvas = Image.new('RGBA', (sz, sz), (0, 0, 0, 0))
    canvas.paste(resized, (inset, inset), resized)
    canvas.save(os.path.join('${dir}', fn + '.png'), 'PNG')
`.replace('mode-placeholder', `mode${MODES.find(m => name.includes(m.suffix) || (!m.suffix && !name.includes('-cb') && !name.includes('-dark')))?.suffix || '-default'}`));
    // Simpler: just use the src function path pattern
    writeFileSync(pyScript, `
from PIL import Image
import os
entries = [${pyEntries}]
for sz, fn in entries:
    icon = Image.open(os.path.join('${dirname(src(16))}', str(sz) + '.png'))
    inset = max(1, int(sz * ${insetRatio}))
    content = sz - inset * 2
    resized = icon.resize((content, content), Image.LANCZOS)
    canvas = Image.new('RGBA', (sz, sz), (0, 0, 0, 0))
    canvas.paste(resized, (inset, inset), resized)
    canvas.save(os.path.join('${dir}', fn + '.png'), 'PNG')
`);
    execSync(`python3 "${pyScript}"`, { stdio: 'pipe' });
    const out = resolve(BRAND_DIR, name);
    execFileSync('iconutil', ['-c', 'icns', dir, '-o', out]);
    logFile(name);
  }

  function logFile(name) {
    const p = resolve(BRAND_DIR, name);
    const kb = Math.round(readFileSync(p).length / 1024);
    console.log(`  ${name} (${kb} KB)`);
  }
}

main().catch((e) => { console.error('Fatal:', e); process.exit(1); });
