// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Icon Generation Script for gamdl-GUI
// ======================================
//
// This script converts the source SVG icon (assets/icons/app-icon.svg) into all
// platform-specific icon formats required by Tauri 2.0 for application bundling.
//
// Generated outputs (written to src-tauri/icons/):
//   - 32x32.png        -- Small icon for taskbar/dock badges, file associations
//   - 128x128.png      -- Standard icon size for file managers, dock
//   - 256x256.png      -- Large icon for high-resolution displays
//   - 512x512.png      -- Extra-large for macOS Finder previews
//   - 128x128@2x.png   -- HiDPI variant: 256x256 actual pixels, displayed at 128x128 logical
//   - icon.png          -- Master 512x512 PNG (used as tray icon by tauri.conf.json)
//   - icon.ico          -- Windows icon bundle (contains 16, 24, 32, 48, 256px sizes)
//   - icon.icns         -- macOS icon bundle (contains 128, 256, 512px sizes)
//
// ICO File Format Details (Windows):
//   The ICO format is a container that holds multiple images at different sizes.
//   Windows selects the appropriate size based on context (taskbar=16-24px, desktop=32-48px,
//   Explorer large view=256px). Structure:
//     [6-byte header] [16-byte directory entry per image...] [PNG data per image...]
//   Each image can be BMP or PNG format. We use PNG (supported since Windows Vista)
//   for better compression and quality. Sizes >= 256 are stored with width/height = 0
//   per the ICO specification (0 means 256 in the 1-byte width/height fields).
//   @see https://en.wikipedia.org/wiki/ICO_(file_format)
//
// ICNS File Format Details (macOS):
//   The ICNS format is Apple's icon container. It stores multiple representations
//   with 4-byte OSType codes identifying each entry:
//     'ic07' = 128x128 PNG, 'ic08' = 256x256 PNG, 'ic09' = 512x512 PNG,
//     'ic10' = 1024x1024 PNG (or 512x512@2x Retina)
//   Structure: [4-byte 'icns' magic] [4-byte file size] [entries...]
//   Each entry: [4-byte type code] [4-byte entry size (incl. 8-byte header)] [PNG data]
//   @see https://en.wikipedia.org/wiki/Apple_Icon_Image_format
//
// Related files:
//   - assets/icons/app-icon.svg     -- Source SVG (single source of truth for the icon)
//   - src-tauri/tauri.conf.json     -- References these generated icons in bundle.icon array
//   - src-tauri/icons/              -- Output directory (gitignored; regenerated as needed)
//
// Usage: node scripts/generate-icons.mjs
// Requires: sharp (npm devDependency) -- High-performance image processing library
// @see https://sharp.pixelplumbing.com/ -- sharp documentation

import sharp from 'sharp';
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { deflateSync } from 'zlib';

// Resolve the directory of this script (ESM equivalent of __dirname)
const __dirname = dirname(fileURLToPath(import.meta.url));

// Project paths
const ROOT = join(__dirname, '..');
const SVG_PATH = join(ROOT, 'assets', 'icons', 'app-icon.svg');  // Source icon
const OUT_DIR = join(ROOT, 'src-tauri', 'icons');                  // Tauri icon directory

// PNG sizes to generate for Tauri's icon array.
// These sizes are referenced in tauri.conf.json bundle.icon:
//   32x32   -- Taskbar, file associations, small UI contexts
//   128x128 -- Standard icon size (Finder, dock, file manager)
//   256x256 -- HiDPI displays, large icon views, used for 128x128@2x
//   512x512 -- macOS large previews, master icon source
const PNG_SIZES = [32, 128, 256, 512];

/**
 * Generates a Windows ICO file from multiple PNG image buffers.
 *
 * ICO Binary Format:
 *   Offset 0-5:   Header (6 bytes)
 *     - Bytes 0-1: Reserved (always 0x0000)
 *     - Bytes 2-3: Image type (0x0001 = icon, 0x0002 = cursor)
 *     - Bytes 4-5: Number of images in the file
 *
 *   Offset 6+:    Directory entries (16 bytes each)
 *     - Byte 0:    Width in pixels (0 means 256)
 *     - Byte 1:    Height in pixels (0 means 256)
 *     - Byte 2:    Number of palette colors (0 for 32-bit)
 *     - Byte 3:    Reserved (always 0)
 *     - Bytes 4-5: Color planes (1 for ICO)
 *     - Bytes 6-7: Bits per pixel (32 for RGBA PNG)
 *     - Bytes 8-11:  Size of the image data in bytes
 *     - Bytes 12-15: Offset from beginning of file to image data
 *
 *   After all directory entries: Raw PNG data for each image, concatenated.
 *
 * @param {Array<{size: number, buffer: Buffer}>} images - PNG images to embed, sorted by size
 * @returns {Buffer} Complete ICO file as a Node.js Buffer
 *
 * @see https://en.wikipedia.org/wiki/ICO_(file_format)#Outline
 * @see https://learn.microsoft.com/en-us/previous-versions/ms997538(v=msdn.10) -- ICO spec
 */
function createIco(images) {
  // ICO file header: 6 bytes total
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // Reserved field: must be 0
  header.writeUInt16LE(1, 2); // Type: 1 = icon (.ico), 2 would be cursor (.cur)
  header.writeUInt16LE(images.length, 4); // Count of images in this ICO file

  // Build directory entries (one per image, 16 bytes each)
  const dirEntries = [];
  // Data starts after header (6 bytes) + all directory entries (16 bytes * count)
  let dataOffset = 6 + images.length * 16;

  for (const { size, buffer } of images) {
    const entry = Buffer.alloc(16);
    // Width/height: stored as uint8, so 256 wraps to 0 (ICO spec: 0 means 256)
    entry.writeUInt8(size >= 256 ? 0 : size, 0); // Width in pixels
    entry.writeUInt8(size >= 256 ? 0 : size, 1); // Height in pixels
    entry.writeUInt8(0, 2);  // Color palette size: 0 = no palette (true color)
    entry.writeUInt8(0, 3);  // Reserved: must be 0
    entry.writeUInt16LE(1, 4);  // Color planes: always 1 for ICO
    entry.writeUInt16LE(32, 6); // Bits per pixel: 32 = RGBA (8 bits per channel)
    entry.writeUInt32LE(buffer.length, 8); // Size of the PNG data in bytes
    entry.writeUInt32LE(dataOffset, 12);   // Absolute byte offset to the PNG data
    dirEntries.push(entry);
    dataOffset += buffer.length; // Advance offset for the next image's data
  }

  // Assemble the complete ICO file: header + directory + all image data
  return Buffer.concat([
    header,
    ...dirEntries,
    ...images.map((img) => img.buffer),
  ]);
}

/**
 * Generates a macOS ICNS file from PNG image buffers at specific sizes.
 *
 * ICNS Binary Format:
 *   Bytes 0-3:   Magic number: 'icns' (0x69636E73)
 *   Bytes 4-7:   Total file size (uint32 big-endian, includes this 8-byte header)
 *
 *   Followed by icon entries, each with:
 *     Bytes 0-3: OSType code identifying the icon size/format (e.g., 'ic07', 'ic08')
 *     Bytes 4-7: Entry size in bytes (uint32 big-endian, includes this 8-byte entry header)
 *     Bytes 8+:  Raw PNG image data
 *
 * OSType Codes for PNG-based icons (introduced in macOS 10.7+):
 *   'ic07' = 128x128   pixels
 *   'ic08' = 256x256   pixels
 *   'ic09' = 512x512   pixels
 *   'ic10' = 1024x1024 pixels (or 512x512@2x for Retina displays)
 *   'ic11' = 32x32@2x  (16x16@2x Retina)
 *   'ic12' = 64x64@2x  (32x32@2x Retina)
 *   'ic13' = 256x256@2x (128x128@2x Retina)
 *   'ic14' = 512x512@2x (256x256@2x Retina)
 *
 * @param {Map<number, Buffer>} pngMap - Map of pixel size (number) to PNG buffer
 * @returns {Buffer} Complete ICNS file as a Node.js Buffer
 *
 * @see https://en.wikipedia.org/wiki/Apple_Icon_Image_format
 * @see https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/Optimizing/Optimizing.html
 */
function createIcns(pngMap) {
  // Map of ICNS OSType codes to the pixel sizes we have available
  const iconTypes = [
    { type: 'ic07', size: 128 },   // 128x128 standard resolution
    { type: 'ic08', size: 256 },   // 256x256 standard resolution
    { type: 'ic09', size: 512 },   // 512x512 standard resolution
    { type: 'ic10', size: 512 },   // 512x512@2x (Retina) -- reuses our largest available PNG
  ];

  const entries = [];
  for (const { type, size } of iconTypes) {
    const png = pngMap.get(size);
    if (!png) continue; // Skip if we don't have this size

    // Build the entry: [4-byte type code][4-byte total entry length][PNG data]
    const typeCode = Buffer.from(type, 'ascii');  // e.g., 'ic07' as 4 ASCII bytes
    const totalLen = Buffer.alloc(4);
    totalLen.writeUInt32BE(8 + png.length, 0);    // Entry length = 8 (header) + data length
    entries.push(Buffer.concat([typeCode, totalLen, png]));
  }

  // ICNS file header: [4-byte 'icns' magic][4-byte total file size]
  const magic = Buffer.from('icns', 'ascii');
  const dataBuffer = Buffer.concat(entries);
  const fileSize = Buffer.alloc(4);
  fileSize.writeUInt32BE(8 + dataBuffer.length, 0); // File size = 8 (header) + all entries

  return Buffer.concat([magic, fileSize, dataBuffer]);
}

/**
 * Main entry point: reads the SVG source and generates all icon formats.
 *
 * Process:
 *   1. Read the SVG source file from assets/icons/app-icon.svg
 *   2. Use sharp to rasterize the SVG at each required PNG size (32, 128, 256, 512)
 *   3. Write individual PNG files (referenced by tauri.conf.json bundle.icon)
 *   4. Create the HiDPI variant (128x128@2x.png = 256px actual)
 *   5. Create the master icon.png (512px, used as tray icon)
 *   6. Assemble the Windows ICO file with 16, 24, 32, 48, 256px sizes
 *   7. Assemble the macOS ICNS file with 128, 256, 512px sizes
 */
async function main() {
  console.log('Reading SVG source...');
  const svgBuffer = readFileSync(SVG_PATH);

  // Ensure the output directory exists (creates parent dirs too)
  mkdirSync(OUT_DIR, { recursive: true });

  // ---- Step 1: Generate standard PNG sizes ----
  // These are the sizes listed in tauri.conf.json bundle.icon
  const pngMap = new Map();
  for (const size of PNG_SIZES) {
    console.log(`  Generating ${size}x${size}.png...`);
    const pngBuffer = await sharp(svgBuffer)
      .resize(size, size)  // Resize SVG to exact pixel dimensions
      .png()               // Output as PNG format (lossless, supports transparency)
      .toBuffer();
    pngMap.set(size, pngBuffer);

    // Write each size as a separate file (e.g., "32x32.png", "128x128.png")
    writeFileSync(join(OUT_DIR, `${size}x${size}.png`), pngBuffer);
  }

  // ---- Step 2: Generate HiDPI variant ----
  // 128x128@2x.png is a naming convention for Retina/HiDPI displays.
  // The "@2x" suffix tells the OS this image has 2x pixel density:
  // it's 256x256 actual pixels but intended to be displayed at 128x128 logical points.
  // This provides crisp icons on macOS Retina and Windows high-DPI displays.
  console.log('  Generating 128x128@2x.png (256px)...');
  writeFileSync(join(OUT_DIR, '128x128@2x.png'), pngMap.get(256));

  // ---- Step 3: Generate master icon ----
  // icon.png (512x512) is the canonical "master" icon used as the system tray icon
  // (referenced in tauri.conf.json app.trayIcon.iconPath) and as a fallback
  // when other sizes are not available.
  console.log('  Generating icon.png (512px)...');
  writeFileSync(join(OUT_DIR, 'icon.png'), pngMap.get(512));

  // ---- Step 4: Generate Windows ICO ----
  // Windows uses ICO files containing multiple embedded images at standard sizes.
  // The OS picks the best size for each context:
  //   16px  -- Small icon (title bar, taskbar on older Windows)
  //   24px  -- Medium taskbar icon (Windows 7+ default)
  //   32px  -- Standard icon (desktop, file manager default view)
  //   48px  -- Large icon (Start menu, file manager tiles)
  //   256px -- Extra-large icon (Explorer large/extra-large view, jumbo icons)
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

  // ---- Step 5: Generate macOS ICNS ----
  // macOS uses ICNS files containing multiple icon representations.
  // The system selects the best resolution based on display density
  // and the icon display context (Dock, Finder, Spotlight, etc.).
  console.log('  Generating icon.icns...');
  const icnsBuffer = createIcns(pngMap);
  writeFileSync(join(OUT_DIR, 'icon.icns'), icnsBuffer);

  console.log('Done! Generated icons in src-tauri/icons/');
}

// Execute and handle errors
main().catch((err) => {
  console.error('Icon generation failed:', err);
  process.exit(1);
});
