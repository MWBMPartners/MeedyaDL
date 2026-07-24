#!/usr/bin/env node
// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// App Icon Generator — assembles the bundled app icons in src-tauri/icons/
// ============================================================================
//
// Fixes the macOS small-size app-icon defect (issue #1058): every
// representation in icon.icns / icon.ico used to be a naive downscale of
// the single 1024px master (src-tauri/icons/icon.png). Below ~64px the
// master's three equaliser slots are sub-2px alternating stripes that
// cannot survive resampling — they average to grey mush, and the .icns was
// also missing all four @2x representations (ic11/ic12/ic13/ic14).
//
// This is a deliberately Linux-friendly, pure-JS replacement for the
// macOS-only scripts/generate-icons.mjs (which needs puppeteer + PIL +
// `iconutil` and only writes the animated multi-theme logo variants to
// assets/brand/ — it does NOT touch src-tauri/icons/ and is unrelated to
// this generator; do not confuse the two).
//
// Size -> source mapping (see assets/brand/icon-tile-small.svg and
// icon-tile-medium.svg for the full design rationale):
//
//   16, 32   -> assets/brand/icon-tile-small.svg   (hand-authored, 1 slot)
//   48, 64   -> assets/brand/icon-tile-medium.svg  (hand-authored, 2 slots)
//   >=128    -> src-tauri/icons/icon.png, downscaled in linear light
//               (the master itself is NEVER modified or regenerated)
//
// Every SVG is rasterised DIRECTLY at its target pixel size (by overriding
// the root <svg> width/height before handing it to sharp) rather than
// rendered large and then downscaled — the small/medium variants were
// hand-drawn on a coarse pixel grid specifically so their edges land on
// pixel boundaries at 16/32/48/64px, which only holds if rsvg rasterises
// at that exact resolution.
//
// Usage: node scripts/generate-app-icons.mjs
//
// Determinism: every output is a pure function of the three source SVG/PNG
// files plus the constants below — running this script twice in a row
// must produce byte-identical files (verified as part of issue #1058).

import sharp from 'sharp';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const BRAND_DIR = resolve(ROOT, 'assets/brand');
const ICONS_DIR = resolve(ROOT, 'src-tauri/icons');
const TRAY_DIR = resolve(ICONS_DIR, 'tray');

const MASTER_PNG_PATH = resolve(ICONS_DIR, 'icon.png');
const SMALL_SVG_PATH = resolve(BRAND_DIR, 'icon-tile-small.svg');
const MEDIUM_SVG_PATH = resolve(BRAND_DIR, 'icon-tile-medium.svg');

// ---------------------------------------------------------------------------
// ONE-LINE SWITCH: the single equalizer slot in icon-tile-small.svg lands at
// exactly 1px tall at 16px — legible in testing but genuinely borderline.
// Both the with-slot and slot-free 16px candidates are rendered onto the
// contact sheet (scripts/icon-contact-sheet.mjs) for a side-by-side call.
// Flip this to `false` to ship the slot-free 16px icon everywhere a 16px
// representation is used (icns `icp4`, .ico 16px entry) instead.
export const SHIP_16PX_WITH_SLOT = true;
// ---------------------------------------------------------------------------

// PNG encode options held constant across every call so re-running the
// generator is byte-for-byte deterministic (no heuristic-driven variance).
export const PNG_OPTS = { compressionLevel: 9, effort: 10, adaptiveFiltering: false };

export { BRAND_DIR, ICONS_DIR, TRAY_DIR, MASTER_PNG_PATH, SMALL_SVG_PATH, MEDIUM_SVG_PATH };

/**
 * Rasterises raw SVG markup directly at `size` x `size` pixels by
 * overriding the root <svg> element's width/height attributes (the
 * viewBox — and therefore the drawing's coordinate system — is left
 * untouched). This makes rsvg (sharp's SVG backend) rasterise natively at
 * the requested resolution instead of rendering at an intrinsic size and
 * being resampled afterwards, which is what lets the hand-pixel-grid-snapped
 * small/medium variants stay crisp.
 */
async function rasterizeSvgAtSize(svgText, size) {
  const sized = svgText.replace(/<svg /, `<svg width="${size}" height="${size}" `);
  return sharp(Buffer.from(sized)).png(PNG_OPTS).toBuffer();
}

/**
 * icon-tile-small.svg's triangle is a single evenodd <path> whose second
 * subpath is the equaliser-slot cut-out hole. Removing that subpath (and
 * nothing else) yields the slot-free 16px candidate while keeping every
 * other coordinate — background gradient, triangle silhouette, pill —
 * identical. The literal coordinates are matched exactly so this silently
 * no-ops (rather than corrupting the path) if the source SVG ever changes
 * without updating this string.
 */
function stripSlotHole(svgText) {
  const HOLE_SUBPATH = '\n       M352,320 L672,320 L672,384 L352,384 Z';
  if (!svgText.includes(HOLE_SUBPATH)) {
    throw new Error(
      'stripSlotHole(): expected slot-hole subpath not found in icon-tile-small.svg — ' +
        'the SVG changed shape; update the HOLE_SUBPATH constant in generate-app-icons.mjs.'
    );
  }
  return svgText.replace(HOLE_SUBPATH, '');
}

/**
 * Converts icon-tile-small.svg (a two-tone tile: gradient background +
 * white glyph) into a transparent-background solid-colour glyph suitable
 * for tray/menu-bar icons — strips the background tile rect and flattens
 * every white fill to `color`. Reuses the exact same triangle+slot+pill
 * geometry as the bundled small variant so the tray icon and the app icon
 * read as the same shape at the same sizes.
 */
function toGlyphSvg(svgText, color) {
  const BACKGROUND_RECT =
    '<rect x="0" y="0" width="1024" height="1024" fill="url(#tileGradientSmall)" />\n\n  ';
  if (!svgText.includes(BACKGROUND_RECT)) {
    throw new Error(
      'toGlyphSvg(): expected background rect not found in icon-tile-small.svg — ' +
        'the SVG changed shape; update the BACKGROUND_RECT constant in generate-app-icons.mjs.'
    );
  }
  return svgText.replace(BACKGROUND_RECT, '').replaceAll('fill="#F9F8F4"', `fill="${color}"`);
}

export async function renderTileSmall(size, { includeSlot = true } = {}) {
  let svg = readFileSync(SMALL_SVG_PATH, 'utf8');
  if (!includeSlot) svg = stripSlotHole(svg);
  return rasterizeSvgAtSize(svg, size);
}

export async function renderTileMedium(size) {
  const svg = readFileSync(MEDIUM_SVG_PATH, 'utf8');
  return rasterizeSvgAtSize(svg, size);
}

export async function renderGlyph(size, color) {
  const svg = toGlyphSvg(readFileSync(SMALL_SVG_PATH, 'utf8'), color);
  return rasterizeSvgAtSize(svg, size);
}

/**
 * Downscales the untouched 1024px master in LINEAR light: sharp's
 * `.gamma()` linearises the image before the resize kernel runs and
 * re-encodes back to sRGB afterwards, which avoids the darkened/muddy
 * midtones a naive sRGB-space resize produces. The master file itself is
 * only ever read here, never written.
 *
 * Kernel: Mitchell-Netravali rather than Lanczos3. On this master's hard
 * white-triangle-on-dark-gradient edges Lanczos3 produces a visible
 * single-pixel ringing halo (measured: an undershoot ~26 units darker
 * than the surrounding background, immediately followed by a ~5-unit
 * overshoot above pure white) — subtle at 1x but obvious once magnified
 * on the contact sheet. Mitchell trades a touch of sharpness for a
 * monotonic (ringing-free) edge transition, which reads as cleaner at
 * every bundled size.
 */
export async function renderMasterDownscale(size) {
  return sharp(MASTER_PNG_PATH)
    .gamma()
    .resize(size, size, { kernel: sharp.kernel.mitchell })
    .png(PNG_OPTS)
    .toBuffer();
}

// ---------------------------------------------------------------------------
// .icns assembly (pure JS — no iconutil, so this runs on Linux/CI/macOS
// alike). Format: 8-byte header ("icns" + big-endian u32 total length),
// then N reps of [4-byte type][4-byte big-endian length incl. header][PNG].
// ---------------------------------------------------------------------------
function buildIcns(reps) {
  const chunks = [];
  let total = 8;
  for (const { type, buffer } of reps) {
    if (type.length !== 4) throw new Error(`icns type code must be 4 chars: "${type}"`);
    const chunkHeader = Buffer.alloc(8);
    chunkHeader.write(type, 0, 'ascii');
    chunkHeader.writeUInt32BE(8 + buffer.length, 4);
    chunks.push(chunkHeader, buffer);
    total += 8 + buffer.length;
  }
  const fileHeader = Buffer.alloc(8);
  fileHeader.write('icns', 0, 'ascii');
  fileHeader.writeUInt32BE(total, 4);
  return Buffer.concat([fileHeader, ...chunks]);
}

// ---------------------------------------------------------------------------
// .ico assembly (pure JS). Modern ICO readers (Windows Explorer/Shell,
// browsers) accept PNG-compressed image data directly inside each
// directory entry, so no BMP/DIB conversion is needed.
// ---------------------------------------------------------------------------
function buildIco(entries) {
  const count = entries.length;
  const dirHeader = Buffer.alloc(6);
  dirHeader.writeUInt16LE(0, 0); // reserved
  dirHeader.writeUInt16LE(1, 2); // type: 1 = icon
  dirHeader.writeUInt16LE(count, 4);

  const dirEntries = [];
  const imageBuffers = [];
  let offset = 6 + count * 16;
  for (const { size, buffer } of entries) {
    const dim = size >= 256 ? 0 : size; // 0 means 256 in the ICO format
    const entry = Buffer.alloc(16);
    entry.writeUInt8(dim, 0); // width
    entry.writeUInt8(dim, 1); // height
    entry.writeUInt8(0, 2); // colour count (0 = no palette)
    entry.writeUInt8(0, 3); // reserved
    entry.writeUInt16LE(1, 4); // colour planes
    entry.writeUInt16LE(32, 6); // bits per pixel
    entry.writeUInt32LE(buffer.length, 8); // size of image data
    entry.writeUInt32LE(offset, 12); // offset of image data
    dirEntries.push(entry);
    imageBuffers.push(buffer);
    offset += buffer.length;
  }
  return Buffer.concat([dirHeader, ...dirEntries, ...imageBuffers]);
}

function writeFile(path, buffer) {
  writeFileSync(path, buffer);
  console.log(`  ${path.replace(ROOT + '/', '')} (${(buffer.length / 1024).toFixed(1)} KB)`);
}

async function main() {
  console.log('App Icon Generator (issue #1058)');
  console.log('=================================\n');

  if (!existsSync(MASTER_PNG_PATH)) {
    throw new Error(`Master icon not found: ${MASTER_PNG_PATH}`);
  }
  if (!existsSync(TRAY_DIR)) mkdirSync(TRAY_DIR, { recursive: true });

  console.log(`16px small variant: ${SHIP_16PX_WITH_SLOT ? 'with slot' : 'slot-free'} (SHIP_16PX_WITH_SLOT)\n`);

  // ---- Render every distinct raster once, reuse the buffer everywhere
  // the same pixel content is needed (keeps output deterministic and
  // avoids re-encoding the same PNG multiple times with possibly
  // different byte output). ----
  console.log('Rendering rasters...');
  const small16 = await renderTileSmall(16, { includeSlot: SHIP_16PX_WITH_SLOT });
  const small32 = await renderTileSmall(32, { includeSlot: true });
  const medium48 = await renderTileMedium(48);
  const medium64 = await renderTileMedium(64);
  const master128 = await renderMasterDownscale(128);
  const master256 = await renderMasterDownscale(256);
  const master512 = await renderMasterDownscale(512);
  const master1024 = readFileSync(MASTER_PNG_PATH); // master is read-only, never re-encoded
  console.log('  done\n');

  // ---- Bundled top-level PNGs (tauri.conf.json `bundle.icon` + the
  // existing sibling sizes already committed alongside them). Filenames
  // preserved exactly; android/, ios/, Square*.png and StoreLogo.png are
  // unused legacy scaffold artifacts (not referenced anywhere in
  // tauri.conf.json or the codebase) and are intentionally left alone —
  // out of scope for the macOS small-icon defect this generator fixes. ----
  console.log('Writing bundled PNGs...');
  writeFile(resolve(ICONS_DIR, '32x32.png'), small32);
  writeFile(resolve(ICONS_DIR, '64x64.png'), medium64);
  writeFile(resolve(ICONS_DIR, '128x128.png'), master128);
  writeFile(resolve(ICONS_DIR, '128x128@2x.png'), master256);
  writeFile(resolve(ICONS_DIR, '256x256.png'), master256);
  writeFile(resolve(ICONS_DIR, '512x512.png'), master512);
  console.log('  (icon.png master left untouched)\n');

  // ---- icon.icns — all ten representations, including the four @2x
  // reps (ic11/ic12/ic13/ic14) that were previously missing entirely. ----
  console.log('Assembling icon.icns...');
  const icns = buildIcns([
    { type: 'icp4', buffer: small16 }, // 16
    { type: 'icp5', buffer: small32 }, // 32
    { type: 'ic11', buffer: small32 }, // 16@2x = 32
    { type: 'ic12', buffer: medium64 }, // 32@2x = 64
    { type: 'ic07', buffer: master128 }, // 128
    { type: 'ic13', buffer: master256 }, // 128@2x = 256
    { type: 'ic08', buffer: master256 }, // 256
    { type: 'ic14', buffer: master512 }, // 256@2x = 512
    { type: 'ic09', buffer: master512 }, // 512
    { type: 'ic10', buffer: master1024 }, // 512@2x = 1024
  ]);
  writeFile(resolve(ICONS_DIR, 'icon.icns'), icns);
  console.log('');

  // ---- icon.ico — 256/128/64/48/32/16, matching the existing entry
  // order. 16/32 route through the small variant, 48/64 through medium,
  // 128/256 through the master downscale. ----
  console.log('Assembling icon.ico...');
  const ico = buildIco([
    { size: 256, buffer: master256 },
    { size: 128, buffer: master128 },
    { size: 64, buffer: medium64 },
    { size: 48, buffer: medium48 },
    { size: 32, buffer: small32 },
    { size: 16, buffer: small16 },
  ]);
  writeFile(resolve(ICONS_DIR, 'icon.ico'), ico);
  console.log('');

  // ---- Tray / menu-bar icons — regenerated from the small variant's
  // glyph (triangle + single slot + pill), preserving the existing
  // convention: solid black glyph (macOS template image,
  // `iconAsTemplate: true` in tauri.conf.json) + solid white glyph
  // (non-macOS light-on-dark tray fallback), both alpha-anti-aliased on
  // a transparent background, at 1x (32) and 2x (64). ----
  console.log('Rendering tray icons...');
  const trayBlack32 = await renderGlyph(32, '#000000');
  const trayBlack64 = await renderGlyph(64, '#000000');
  const trayWhite32 = await renderGlyph(32, '#FFFFFF');
  const trayWhite64 = await renderGlyph(64, '#FFFFFF');
  writeFile(resolve(TRAY_DIR, 'tray-icon.png'), trayBlack32);
  writeFile(resolve(TRAY_DIR, 'tray-icon@2x.png'), trayBlack64);
  writeFile(resolve(TRAY_DIR, 'tray-icon-white.png'), trayWhite32);
  writeFile(resolve(TRAY_DIR, 'tray-icon-white@2x.png'), trayWhite64);

  console.log('\nDone.');
}

// Only run when executed directly (`node scripts/generate-app-icons.mjs`) —
// scripts/icon-contact-sheet.mjs imports renderTileSmall/renderTileMedium/
// renderMasterDownscale from this module and must not trigger a full
// regeneration as a side effect of that import.
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((e) => {
    console.error('Fatal:', e);
    process.exit(1);
  });
}
