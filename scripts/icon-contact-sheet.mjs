#!/usr/bin/env node
// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Icon Contact Sheet + Regression Verification (issue #1058)
// ============================================================================
//
// Two jobs in one script:
//
//  1. (default) Renders a PNG contact sheet comparing the OLD (naive
//     1024px-master downscale) and NEW (hand-authored small/medium variant)
//     app icons at 16/32/48/64/128px, each magnified 8x with nearest-
//     neighbour scaling so the pixel-level difference is visible without
//     squinting. Also renders both 16px candidates (with-slot / slot-free)
//     side by side for the owner to pick between (see SHIP_16PX_WITH_SLOT
//     in generate-app-icons.mjs for the one-line switch).
//
//     Output: .github/audits/icon-contact-sheet-2026-07-24.png
//
//  2. (--verify) Runs the same numeric legibility checks against the OLD
//     and NEW 32px icons and asserts the OLD icon FAILS while the NEW icon
//     PASSES — this is the actual regression test for the defect described
//     in issue #1058. Prints the measured numbers either way. Exits 1 if
//     the assertion doesn't hold.
//
// IMPORTANT — "OLD" here means "the icon set as committed at HEAD", read via
// `git show HEAD:<path>`. This only works BEFORE the regenerated icons in
// src-tauri/icons/ are committed (i.e. run this script, or at least capture
// its output, in the same working session as `node
// scripts/generate-app-icons.mjs`, before `git add`-ing the result). Once
// the regenerated icons are committed, HEAD *is* the new (fixed) icon set
// and this script no longer has anything to contrast against — that's
// expected; the contact sheet PNG is the durable point-in-time record.
//
// Usage:
//   node scripts/icon-contact-sheet.mjs            # writes the contact sheet
//   node scripts/icon-contact-sheet.mjs --verify    # runs the numeric checks

import sharp from 'sharp';
import { execFileSync } from 'child_process';
import { writeFileSync, existsSync, mkdirSync, readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import {
  ICONS_DIR,
  renderTileSmall,
  renderTileMedium,
  renderMasterDownscale,
} from './generate-app-icons.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const OUTPUT_PATH = resolve(ROOT, '.github/audits/icon-contact-sheet-2026-07-24.png');

// ---------------------------------------------------------------------------
// OLD icon extraction — read straight from git HEAD (see module comment).
// ---------------------------------------------------------------------------
function readAtHead(relPath) {
  return execFileSync('git', ['show', `HEAD:${relPath}`], { cwd: ROOT, maxBuffer: 1024 * 1024 * 64 });
}

/** Minimal read-only .icns parser — returns the PNG payload for `type`. */
function readIcnsRep(buffer, type) {
  let i = 8;
  while (i < buffer.length) {
    const repType = buffer.toString('ascii', i, i + 4);
    const len = buffer.readUInt32BE(i + 4);
    if (repType === type) return buffer.subarray(i + 8, i + len);
    i += len;
  }
  throw new Error(`icns representation "${type}" not found`);
}

/** Minimal read-only .ico parser — returns the PNG payload for `size`. */
function readIcoEntry(buffer, size) {
  const count = buffer.readUInt16LE(4);
  const wantDim = size >= 256 ? 0 : size;
  for (let i = 0; i < count; i++) {
    const off = 6 + i * 16;
    const w = buffer.readUInt8(off);
    if (w === wantDim) {
      const dataSize = buffer.readUInt32LE(off + 8);
      const dataOffset = buffer.readUInt32LE(off + 12);
      return buffer.subarray(dataOffset, dataOffset + dataSize);
    }
  }
  throw new Error(`ico entry for size ${size} not found`);
}

async function loadOld() {
  const icns = readAtHead('src-tauri/icons/icon.icns');
  const ico = readAtHead('src-tauri/icons/icon.ico');
  return {
    16: readIcnsRep(icns, 'icp4'),
    32: readAtHead('src-tauri/icons/32x32.png'),
    48: readIcoEntry(ico, 48),
    64: readAtHead('src-tauri/icons/64x64.png'),
    128: readAtHead('src-tauri/icons/128x128.png'),
  };
}

async function loadNew() {
  // 16px and 48px have no standalone bundled file (they only ever lived
  // inside .icns/.ico) — render them directly via the same generator
  // functions generate-app-icons.mjs uses, so this is byte-identical to
  // what actually ships. 32/64/128 are read straight from the already-
  // regenerated files on disk.
  return {
    16: await renderTileSmall(16, { includeSlot: true }),
    '16candidateSlotFree': await renderTileSmall(16, { includeSlot: false }),
    32: readFileSync(resolve(ICONS_DIR, '32x32.png')),
    48: await renderTileMedium(48),
    64: readFileSync(resolve(ICONS_DIR, '64x64.png')),
    128: readFileSync(resolve(ICONS_DIR, '128x128.png')),
  };
}

// ---------------------------------------------------------------------------
// Contact sheet rendering
// ---------------------------------------------------------------------------
const MAGNIFY = 8;
const CELL_PAD = 16;
const LABEL_H = 22;
const ROW_GAP = 28;
const COL_GAP = 40;

async function magnify(buffer, size) {
  return sharp(buffer)
    .resize(size * MAGNIFY, size * MAGNIFY, { kernel: 'nearest' })
    .png()
    .toBuffer();
}

async function toDataUri(buffer) {
  return `data:image/png;base64,${buffer.toString('base64')}`;
}

async function buildContactSheet(oldIcons, newIcons) {
  const rows = [
    { size: 16, label: '16px', old: oldIcons[16], new: newIcons[16] },
    { size: 32, label: '32px', old: oldIcons[32], new: newIcons[32] },
    { size: 48, label: '48px', old: oldIcons[48], new: newIcons[48] },
    { size: 64, label: '64px', old: oldIcons[64], new: newIcons[64] },
    { size: 128, label: '128px', old: oldIcons[128], new: newIcons[128] },
  ];
  // Special row: both 16px candidates, for the owner to pick between.
  const candidateRow = {
    size: 16,
    label: '16px candidates',
    old: newIcons[16], // "with slot" (current default)
    new: newIcons['16candidateSlotFree'],
    oldLabel: 'With slot (shipped)',
    newLabel: 'Slot-free (alternative)',
  };

  // Column width is fixed to the largest row (128px) so columns line up,
  // but each row's HEIGHT is sized to that row's own icon (16/32/48/64px
  // rows don't need 128px-tall cells) — avoids the mostly-empty rows a
  // fixed grid would produce.
  const maxCellSize = 128 * MAGNIFY;
  const colWidth = maxCellSize + CELL_PAD * 2;
  const headerH = 90;
  const sheetW = COL_GAP + colWidth * 2 + COL_GAP;

  const images = [];
  const textLabels = [];
  let y = headerH;

  async function addRow(row, oldLabel, newLabel) {
    const cellSize = row.size * MAGNIFY;
    const rowHeight = cellSize + CELL_PAD * 2 + LABEL_H;
    const oldMag = await magnify(row.old, row.size);
    const newMag = await magnify(row.new, row.size);
    const oldOffsetX = COL_GAP + Math.round((colWidth - cellSize) / 2);
    const newOffsetX = COL_GAP + colWidth + COL_GAP + Math.round((colWidth - cellSize) / 2);
    const imgY = y + LABEL_H + CELL_PAD;
    images.push({ input: oldMag, left: oldOffsetX, top: imgY });
    images.push({ input: newMag, left: newOffsetX, top: imgY });
    const textY = y + LABEL_H - 4;
    y += rowHeight + ROW_GAP;
    textLabels.push({ oldLabel, newLabel, rowLabel: row.label, textY });
  }

  for (const row of rows) {
    await addRow(row, 'OLD (naive downscale)', 'NEW (hand-authored)');
  }
  await addRow(candidateRow, candidateRow.oldLabel, candidateRow.newLabel);
  const sheetH = y + ROW_GAP;

  // Build the label/frame SVG overlay (text renders via librsvg/pango,
  // which sharp already bundles).
  let svgParts = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${sheetW}" height="${sheetH}">`,
    `<rect x="0" y="0" width="${sheetW}" height="${sheetH}" fill="#1b1f24" />`,
    `<text x="${COL_GAP}" y="36" font-family="sans-serif" font-size="26" font-weight="bold" fill="#F9F8F4">MeedyaDL App Icon — Small-Size Fix Contact Sheet (issue #1058)</text>`,
    `<text x="${COL_GAP}" y="62" font-family="sans-serif" font-size="15" fill="#A7AEB6">Left: OLD (1024px master, naive downscale) — Right: NEW (hand-authored small/medium variants). Magnified ${MAGNIFY}x, nearest-neighbour.</text>`,
  ];
  for (const t of textLabels) {
    const oldColX = COL_GAP;
    const newColX = COL_GAP + colWidth + COL_GAP;
    svgParts.push(
      `<text x="${oldColX}" y="${t.textY}" font-family="sans-serif" font-size="17" fill="#F9F8F4">${t.rowLabel} — ${t.oldLabel}</text>`
    );
    svgParts.push(
      `<text x="${newColX}" y="${t.textY}" font-family="sans-serif" font-size="17" fill="#F9F8F4">${t.rowLabel} — ${t.newLabel}</text>`
    );
  }
  svgParts.push('</svg>');
  const overlaySvg = Buffer.from(svgParts.join(''));

  const base = sharp({
    create: { width: sheetW, height: sheetH, channels: 4, background: '#1b1f24' },
  });

  const composited = await base
    .composite([{ input: overlaySvg }, ...images.map((im) => ({ input: im.input, left: im.left, top: im.top }))])
    .png()
    .toBuffer();

  return composited;
}

// ---------------------------------------------------------------------------
// Verification profiles — measured/derived geometry (fractions of a 1024
// canvas) for the OLD master and the NEW small variant, used to locate the
// pill/gap/slot regions independently of which asset is being measured.
//
// OLD (master) slot y/x bounds were measured directly off the committed
// src-tauri/icons/icon.png (a vertical + horizontal dark-run scan at the
// canvas centre) rather than guessed — see the issue #1058 implementation
// notes for the scan. NEW (small variant) bounds come straight from
// assets/brand/icon-tile-small.svg's coordinates.
// ---------------------------------------------------------------------------
const OLD_PROFILE = {
  triangleTopFrac: 76 / 1024,
  apexFrac: 696 / 1024,
  triangleX0Frac: 170 / 1024,
  triangleX1Frac: 853 / 1024,
  pillTopFrac: 831 / 1024,
  pillBottomFrac: 952 / 1024,
  pillX0Frac: 236 / 1024,
  pillX1Frac: 788 / 1024,
  slots: [
    { yTopFrac: 308 / 1024, yBottomFrac: 357 / 1024, xLeftFrac: 349 / 1024, xRightFrac: 674 / 1024 },
    { yTopFrac: 412 / 1024, yBottomFrac: 461 / 1024, xLeftFrac: 385 / 1024, xRightFrac: 638 / 1024 },
    { yTopFrac: 517 / 1024, yBottomFrac: 567 / 1024, xLeftFrac: 437 / 1024, xRightFrac: 586 / 1024 },
  ],
};

const NEW_PROFILE = {
  triangleTopFrac: 96 / 1024,
  apexFrac: 736 / 1024,
  triangleX0Frac: 160 / 1024,
  triangleX1Frac: 864 / 1024,
  pillTopFrac: 864 / 1024,
  pillBottomFrac: 992 / 1024,
  pillX0Frac: 240 / 1024,
  pillX1Frac: 784 / 1024,
  slots: [{ yTopFrac: 320 / 1024, yBottomFrac: 384 / 1024, xLeftFrac: 352 / 1024, xRightFrac: 672 / 1024 }],
};

function isWhite(r, g, b, thresh = 235) {
  return r > thresh && g > thresh && b > thresh;
}
function isDark(r, g, b) {
  return (r + g + b) / 3 < 170;
}
function isMidtone(r, g, b) {
  const avg = (r + g + b) / 3;
  return avg >= 100 && avg <= 170;
}

async function toRaw(buffer) {
  const { data, info } = await sharp(buffer).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  return { data, width: info.width, height: info.height };
}

function pixelAt(raw, x, y) {
  const i = (y * raw.width + x) * 4;
  return [raw.data[i], raw.data[i + 1], raw.data[i + 2], raw.data[i + 3]];
}

/**
 * Runs the four issue-#1058 legibility checks against a 32px icon raster,
 * using `profile` to locate the pill/gap/slot regions. Returns a results
 * object with the raw measured numbers plus a per-check boolean and an
 * overall `pass`.
 */
async function measure(buffer, size, profile, label) {
  const raw = await toRaw(buffer);

  // (A) >=3 fully-white pill rows, sampled on a horizontal strip inset by
  // the pill's corner radius so the rounded end anti-aliasing doesn't
  // count against a row that's otherwise solid white.
  const pillX0 = profile.pillX0Frac * size;
  const pillX1 = profile.pillX1Frac * size;
  const pillHeightPx = (profile.pillBottomFrac - profile.pillTopFrac) * size;
  const radius = Math.max(1, Math.round(pillHeightPx / 2));
  const stripC0 = Math.max(0, Math.ceil(pillX0 + radius));
  const stripC1 = Math.min(size - 1, Math.floor(pillX1 - radius));
  const pillRowTop = Math.round(profile.pillTopFrac * size);
  const pillRowBottom = Math.round(profile.pillBottomFrac * size);
  let fullyWhitePillRows = 0;
  for (let y = pillRowTop; y < pillRowBottom; y++) {
    let whiteCount = 0;
    let total = 0;
    for (let x = stripC0; x <= stripC1; x++) {
      const [r, g, b] = pixelAt(raw, x, y);
      total++;
      if (isWhite(r, g, b)) whiteCount++;
    }
    if (total > 0 && whiteCount / total >= 0.9) fullyWhitePillRows++;
  }

  // (B) >=3 background rows between triangle apex and pill (no white
  // pixels anywhere in the row).
  const gapRowTop = Math.round(profile.apexFrac * size);
  const gapRowBottom = Math.round(profile.pillTopFrac * size);
  let backgroundGapRows = 0;
  for (let y = gapRowTop; y < gapRowBottom; y++) {
    let whiteCount = 0;
    for (let x = 0; x < size; x++) {
      const [r, g, b] = pixelAt(raw, x, y);
      if (isWhite(r, g, b)) whiteCount++;
    }
    if (whiteCount === 0) backgroundGapRows++;
  }

  // (C) slot rows >=80% dark within slot bounds (aggregated across every
  // slot the profile defines).
  let slotDarkPixels = 0;
  let slotTotalPixels = 0;
  for (const slot of profile.slots) {
    // floor/ceil (not round) so every row/column with ANY sub-pixel
    // overlap with the intended slot region is sampled — rounding to a
    // single nearest row would under-sample the region and hide exactly
    // the anti-aliased "mush" rows the defect produces.
    const y0 = Math.floor(slot.yTopFrac * size);
    const y1 = Math.ceil(slot.yBottomFrac * size);
    const x0 = Math.floor(slot.xLeftFrac * size);
    const x1 = Math.ceil(slot.xRightFrac * size);
    for (let y = y0; y < y1; y++) {
      for (let x = x0; x < x1; x++) {
        const [r, g, b] = pixelAt(raw, x, y);
        slotTotalPixels++;
        if (isDark(r, g, b)) slotDarkPixels++;
      }
    }
  }
  const slotDarkFraction = slotTotalPixels > 0 ? slotDarkPixels / slotTotalPixels : 0;

  // (D) <10% of glyph-area pixels (triangle bbox + pill bbox) in the
  // 100-170 midtone band.
  let midtoneCount = 0;
  let glyphAreaTotal = 0;
  const triX0 = Math.round(profile.triangleX0Frac * size);
  const triX1 = Math.round(profile.triangleX1Frac * size);
  const triY0 = Math.round(profile.triangleTopFrac * size);
  const triY1 = Math.round(profile.apexFrac * size);
  for (let y = triY0; y < triY1; y++) {
    for (let x = triX0; x < triX1; x++) {
      const [r, g, b] = pixelAt(raw, x, y);
      glyphAreaTotal++;
      if (isMidtone(r, g, b)) midtoneCount++;
    }
  }
  for (let y = pillRowTop; y < pillRowBottom; y++) {
    for (let x = Math.round(pillX0); x < Math.round(pillX1); x++) {
      const [r, g, b] = pixelAt(raw, x, y);
      glyphAreaTotal++;
      if (isMidtone(r, g, b)) midtoneCount++;
    }
  }
  const midtoneFraction = glyphAreaTotal > 0 ? midtoneCount / glyphAreaTotal : 0;

  const checks = {
    pillRows: { value: fullyWhitePillRows, pass: fullyWhitePillRows >= 3 },
    gapRows: { value: backgroundGapRows, pass: backgroundGapRows >= 3 },
    slotDark: { value: slotDarkFraction, pass: slotDarkFraction >= 0.8 },
    midtone: { value: midtoneFraction, pass: midtoneFraction < 0.1 },
  };
  const pass = Object.values(checks).every((c) => c.pass);

  console.log(`\n  ${label}:`);
  console.log(
    `    fully-white pill rows:     ${checks.pillRows.value} (need >=3)  ${checks.pillRows.pass ? 'PASS' : 'FAIL'}`
  );
  console.log(
    `    background gap rows:       ${checks.gapRows.value} (need >=3)  ${checks.gapRows.pass ? 'PASS' : 'FAIL'}`
  );
  console.log(
    `    slot dark fraction:        ${(checks.slotDark.value * 100).toFixed(1)}% (need >=80%)  ${
      checks.slotDark.pass ? 'PASS' : 'FAIL'
    }`
  );
  console.log(
    `    glyph-area midtone frac:   ${(checks.midtone.value * 100).toFixed(1)}% (need <10%)  ${
      checks.midtone.pass ? 'PASS' : 'FAIL'
    }`
  );
  console.log(`    overall: ${pass ? 'PASS' : 'FAIL'}`);

  return { label, checks, pass };
}

async function runVerify(oldIcons, newIcons) {
  console.log('Verification (issue #1058) — 32px legibility checks');
  console.log('=====================================================');

  const oldResult = await measure(oldIcons[32], 32, OLD_PROFILE, 'OLD 32x32.png (naive downscale)');
  const newResult = await measure(newIcons[32], 32, NEW_PROFILE, 'NEW 32x32.png (hand-authored)');

  console.log('\nRegression-test assertion: OLD must FAIL, NEW must PASS.');
  const ok = !oldResult.pass && newResult.pass;
  console.log(ok ? '  RESULT: OK — regression demonstrated and fixed.' : '  RESULT: FAILED — assertion did not hold.');
  if (!ok) process.exitCode = 1;
}

async function main() {
  const verifyMode = process.argv.includes('--verify');

  console.log('Loading OLD icons from git HEAD...');
  const oldIcons = await loadOld();
  console.log('Loading NEW icons (regenerated)...');
  const newIcons = await loadNew();

  if (verifyMode) {
    await runVerify(oldIcons, newIcons);
    return;
  }

  console.log('Building contact sheet...');
  const sheet = await buildContactSheet(oldIcons, newIcons);
  const outDir = dirname(OUTPUT_PATH);
  if (!existsSync(outDir)) mkdirSync(outDir, { recursive: true });
  writeFileSync(OUTPUT_PATH, sheet);
  console.log(`Wrote ${OUTPUT_PATH.replace(ROOT + '/', '')} (${(sheet.length / 1024).toFixed(1)} KB)`);
}

main().catch((e) => {
  console.error('Fatal:', e);
  process.exit(1);
});
