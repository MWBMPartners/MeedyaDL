// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Icon generation script for gamdl-GUI.
// Converts the source SVG icon (assets/icons/app-icon.svg) into all
// platform-specific icon formats required by Tauri:
// - PNG files at various sizes (32x32, 128x128, 256x256, 512x512)
// - 128x128@2x.png (256x256 actual pixels for HiDPI)
// - icon.png (512x512 master PNG)
// - icon.icns (macOS app icon bundle)
// - icon.ico (Windows app icon with multiple sizes)
//
// Usage: node scripts/generate-icons.mjs

import sharp from 'sharp';
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { deflateSync } from 'zlib';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const SVG_PATH = join(ROOT, 'assets', 'icons', 'app-icon.svg');
const OUT_DIR = join(ROOT, 'src-tauri', 'icons');

// Sizes needed for Tauri icons
const PNG_SIZES = [32, 128, 256, 512];

/**
 * Generates a Windows ICO file from multiple PNG buffers.
 * ICO format: header + directory entries + image data.
 *
 * @param {Array<{size: number, buffer: Buffer}>} images - PNG images sorted by size
 * @returns {Buffer} The ICO file buffer
 */
function createIco(images) {
  // ICO header: 6 bytes (reserved=0, type=1 for icon, count)
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type: 1 = icon
  header.writeUInt16LE(images.length, 4); // image count

  // Directory entries: 16 bytes each
  const dirEntries = [];
  let dataOffset = 6 + images.length * 16;

  for (const { size, buffer } of images) {
    const entry = Buffer.alloc(16);
    entry.writeUInt8(size >= 256 ? 0 : size, 0); // width (0 = 256)
    entry.writeUInt8(size >= 256 ? 0 : size, 1); // height (0 = 256)
    entry.writeUInt8(0, 2);  // color palette
    entry.writeUInt8(0, 3);  // reserved
    entry.writeUInt16LE(1, 4);  // color planes
    entry.writeUInt16LE(32, 6); // bits per pixel
    entry.writeUInt32LE(buffer.length, 8); // image data size
    entry.writeUInt32LE(dataOffset, 12);   // offset to image data
    dirEntries.push(entry);
    dataOffset += buffer.length;
  }

  // Concatenate header + directory + image data
  return Buffer.concat([
    header,
    ...dirEntries,
    ...images.map((img) => img.buffer),
  ]);
}

/**
 * Generates a macOS ICNS file from PNG buffers at specific sizes.
 * ICNS format uses 'icns' magic + size, then type-tagged icon entries.
 *
 * @param {Map<number, Buffer>} pngMap - Map of size → PNG buffer
 * @returns {Buffer} The ICNS file buffer
 */
function createIcns(pngMap) {
  // ICNS icon type codes mapped to sizes
  const iconTypes = [
    { type: 'ic07', size: 128 },   // 128x128
    { type: 'ic08', size: 256 },   // 256x256
    { type: 'ic09', size: 512 },   // 512x512
    { type: 'ic10', size: 512 },   // 512x512@2x (we use 512 as our max)
  ];

  const entries = [];
  for (const { type, size } of iconTypes) {
    const png = pngMap.get(size);
    if (!png) continue;

    // Each entry: 4-byte type + 4-byte total length + data
    const typeCode = Buffer.from(type, 'ascii');
    const totalLen = Buffer.alloc(4);
    totalLen.writeUInt32BE(8 + png.length, 0);
    entries.push(Buffer.concat([typeCode, totalLen, png]));
  }

  // File header: 'icns' magic + 4-byte total file size
  const magic = Buffer.from('icns', 'ascii');
  const dataBuffer = Buffer.concat(entries);
  const fileSize = Buffer.alloc(4);
  fileSize.writeUInt32BE(8 + dataBuffer.length, 0);

  return Buffer.concat([magic, fileSize, dataBuffer]);
}

async function main() {
  console.log('Reading SVG source...');
  const svgBuffer = readFileSync(SVG_PATH);

  // Ensure output directory exists
  mkdirSync(OUT_DIR, { recursive: true });

  // Generate PNGs at each required size
  const pngMap = new Map();
  for (const size of PNG_SIZES) {
    console.log(`  Generating ${size}x${size}.png...`);
    const pngBuffer = await sharp(svgBuffer)
      .resize(size, size)
      .png()
      .toBuffer();
    pngMap.set(size, pngBuffer);

    // Write the standard size PNGs
    writeFileSync(join(OUT_DIR, `${size}x${size}.png`), pngBuffer);
  }

  // 128x128@2x is actually 256x256 pixels (HiDPI)
  console.log('  Generating 128x128@2x.png (256px)...');
  writeFileSync(join(OUT_DIR, '128x128@2x.png'), pngMap.get(256));

  // icon.png is the master 512x512
  console.log('  Generating icon.png (512px)...');
  writeFileSync(join(OUT_DIR, 'icon.png'), pngMap.get(512));

  // Generate Windows ICO (contains 16, 32, 48, 256 sizes)
  console.log('  Generating icon.ico...');
  const icoSizes = [16, 24, 32, 48, 256];
  const icoImages = [];
  for (const size of icoSizes) {
    const pngBuffer = await sharp(svgBuffer)
      .resize(size, size)
      .png()
      .toBuffer();
    icoImages.push({ size, buffer: pngBuffer });
  }
  const icoBuffer = createIco(icoImages);
  writeFileSync(join(OUT_DIR, 'icon.ico'), icoBuffer);

  // Generate macOS ICNS
  console.log('  Generating icon.icns...');
  const icnsBuffer = createIcns(pngMap);
  writeFileSync(join(OUT_DIR, 'icon.icns'), icnsBuffer);

  console.log('Done! Generated icons in src-tauri/icons/');
}

main().catch((err) => {
  console.error('Icon generation failed:', err);
  process.exit(1);
});
