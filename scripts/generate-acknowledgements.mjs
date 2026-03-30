#!/usr/bin/env node
// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Generates ACKNOWLEDGEMENTS.md from engines.toml, Cargo.toml, and package.json.
// Run: node scripts/generate-acknowledgements.mjs
//
// Sources:
//   - src-tauri/engines.toml  → Download engines (only enabled ones)
//   - src-tauri/Cargo.toml    → Rust backend dependencies
//   - package.json            → Frontend npm dependencies
//
// The generated file lists only currently enabled/shipping dependencies.
// Licence names are hyperlinked to the official licence text.
// Run this script whenever engines are enabled/disabled or deps change.

import { readFileSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

// ── Licence URL mapping ──
// Official licence text URLs for hyperlinking in the acknowledgements.
const LICENCE_URLS = {
  'MIT': 'https://opensource.org/licenses/MIT',
  'MIT / Apache-2.0': 'https://opensource.org/licenses/MIT',
  'Apache-2.0': 'https://www.apache.org/licenses/LICENSE-2.0',
  'ISC': 'https://opensource.org/licenses/ISC',
  'MPL-2.0': 'https://www.mozilla.org/en-US/MPL/2.0/',
  'LGPL / GPL': 'https://www.gnu.org/licenses/gpl-3.0.en.html',
  'LGPL-2.1': 'https://www.gnu.org/licenses/old-licenses/lgpl-2.1.en.html',
  'BSD-2-Clause': 'https://opensource.org/licenses/BSD-2-Clause',
  'Unlicense': 'https://unlicense.org/',
  'GPL-3.0': 'https://www.gnu.org/licenses/gpl-3.0.en.html',
};

function licenceLink(licence) {
  const url = LICENCE_URLS[licence];
  return url ? `[${licence}](${url})` : licence;
}

// ── Parse engines.toml for enabled engines ──
function parseEngines() {
  const toml = readFileSync(join(ROOT, 'src-tauri/engines.toml'), 'utf-8');
  const engines = [];
  const engineBlocks = toml.split(/\[engines\./g).slice(1);
  for (const block of engineBlocks) {
    const lines = block.split('\n');
    const get = (key) => {
      const line = lines.find((l) => l.trim().startsWith(`${key} =`) || l.trim().startsWith(`${key}=`));
      if (!line) return undefined;
      return line.split('=').slice(1).join('=').trim().replace(/^["']|["']$/g, '').split('#')[0].trim();
    };
    if (get('enabled') === 'true') {
      engines.push({
        name: get('name') || '',
        homepage: get('homepage') || '',
        description: get('description') || '',
      });
    }
  }
  return engines;
}

// ── Parse Cargo.toml for dependencies ──
function parseCargoToml() {
  const toml = readFileSync(join(ROOT, 'src-tauri/Cargo.toml'), 'utf-8');
  const depsMatch = toml.match(/\[dependencies\]([\s\S]*?)(?=\[dev-dependencies\]|\[features\]|$)/);
  if (!depsMatch) return [];

  const section = depsMatch[1];
  const deps = [];

  // Crate → { licence, repo } mapping
  const crateInfo = {
    'tauri': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/tauri-apps/tauri' },
    'serde': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/serde-rs/serde' },
    'serde_json': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/serde-rs/json' },
    'tokio': { licence: 'MIT', repo: 'https://github.com/tokio-rs/tokio' },
    'reqwest': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/seanmonstar/reqwest' },
    'zip': { licence: 'MIT', repo: 'https://github.com/zip-rs/zip2' },
    'flate2': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/rust-lang/flate2-rs' },
    'tar': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/alexcrichton/tar-rs' },
    'sha2': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/RustCrypto/hashes' },
    'configparser': { licence: 'MIT', repo: 'https://github.com/QEDK/configparser-rs' },
    'toml': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/toml-rs/toml' },
    'keyring': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/hwchen/keyring-rs' },
    'uuid': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/uuid-rs/uuid' },
    'chrono': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/chronotope/chrono' },
    'log': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/rust-lang/log' },
    'tracing': { licence: 'MIT', repo: 'https://github.com/tokio-rs/tracing' },
    'tracing-subscriber': { licence: 'MIT', repo: 'https://github.com/tokio-rs/tracing' },
    'tracing-appender': { licence: 'MIT', repo: 'https://github.com/tokio-rs/tracing' },
    'sentry': { licence: 'Apache-2.0', repo: 'https://github.com/getsentry/sentry-rust' },
    'sentry-tracing': { licence: 'Apache-2.0', repo: 'https://github.com/getsentry/sentry-rust' },
    'thiserror': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/dtolnay/thiserror' },
    'dirs': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/dirs-dev/dirs-rs' },
    'regex': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/rust-lang/regex' },
    'jsonwebtoken': { licence: 'MIT', repo: 'https://github.com/Keats/jsonwebtoken' },
    'rookie': { licence: 'MIT', repo: 'https://github.com/nicedude/rookie' },
    'cookie': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/SergioBenitez/cookie-rs' },
    'url': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/servo/rust-url' },
    'sys-locale': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/1Password/sys-locale' },
    'mp4ameta': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/Saecki/rust-mp4ameta' },
    'rusty-chromaprint': { licence: 'MIT', repo: 'https://github.com/nicedude/rusty-chromaprint' },
    'symphonia': { licence: 'MPL-2.0', repo: 'https://github.com/pdeljanov/Symphonia' },
    'base64': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/marshallpierce/rust-base64' },
    'roxmltree': { licence: 'MIT / Apache-2.0', repo: 'https://github.com/RazrFalcon/roxmltree' },
  };

  const crateRegex = /^([a-z][a-z0-9_-]*)\s*=/gm;
  let match;
  while ((match = crateRegex.exec(section)) !== null) {
    const name = match[1];
    if (name.startsWith('tauri-plugin-') || name === 'tauri-build') continue;
    const info = crateInfo[name] || { licence: 'See crate', repo: `https://crates.io/crates/${name}` };
    deps.push({ name, licence: info.licence, repo: info.repo });
  }
  return deps;
}

// ── Parse package.json for frontend deps ──
function parsePackageJson() {
  const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf-8'));
  const deps = [];
  const pkgInfo = {
    'react': { licence: 'MIT', repo: 'https://github.com/facebook/react' },
    'react-dom': { licence: 'MIT', repo: 'https://github.com/facebook/react' },
    'zustand': { licence: 'MIT', repo: 'https://github.com/pmndrs/zustand' },
    'react-markdown': { licence: 'MIT', repo: 'https://github.com/remarkjs/react-markdown' },
    'i18next': { licence: 'MIT', repo: 'https://github.com/i18next/i18next' },
    'react-i18next': { licence: 'MIT', repo: 'https://github.com/i18next/react-i18next' },
    'i18next-browser-languagedetector': { licence: 'MIT', repo: 'https://github.com/i18next/i18next-browser-languageDetector' },
    'lucide-react': { licence: 'ISC', repo: 'https://github.com/lucide-icons/lucide' },
    'rehype-raw': { licence: 'MIT', repo: 'https://github.com/rehypejs/rehype-raw' },
    'rehype-sanitize': { licence: 'MIT', repo: 'https://github.com/rehypejs/rehype-sanitize' },
    'remark-gfm': { licence: 'MIT', repo: 'https://github.com/remarkjs/remark-gfm' },
  };

  for (const name of Object.keys(pkg.dependencies || {})) {
    if (name.startsWith('@tauri-apps/') || name.startsWith('@dnd-kit/') || name.startsWith('@sentry/')) continue;
    const info = pkgInfo[name] || { licence: 'MIT', repo: `https://www.npmjs.com/package/${name}` };
    deps.push({ name, licence: info.licence, repo: info.repo });
  }
  return deps;
}

// ── Tauri plugins ──
function parseTauriPlugins() {
  const toml = readFileSync(join(ROOT, 'src-tauri/Cargo.toml'), 'utf-8');
  const plugins = [];
  const regex = /^(tauri-plugin-[a-z-]+)\s*=/gm;
  let match;
  while ((match = regex.exec(toml)) !== null) plugins.push(match[1]);
  return plugins;
}

// ── External tools ──
const EXTERNAL_TOOLS = [
  { name: 'FFmpeg', homepage: 'https://ffmpeg.org/', licence: 'LGPL / GPL', description: 'Audio/video processing and remuxing' },
  { name: 'mp4decrypt (Bento4)', homepage: 'https://www.bento4.com/', licence: 'MIT', description: 'MP4 DRM decryption' },
  { name: 'N_m3u8DL-RE', homepage: 'https://github.com/nilaoda/N_m3u8DL-RE', licence: 'MIT', description: 'HLS/DASH stream downloader' },
  { name: 'MP4Box (GPAC)', homepage: 'https://gpac.io/', licence: 'LGPL-2.1', description: 'MP4 container muxing/remuxing' },
  { name: 'MediaInfo', homepage: 'https://mediaarea.net/en/MediaInfo', licence: 'BSD-2-Clause', description: 'Media file analysis and codec detection' },
  { name: 'python-build-standalone', homepage: 'https://github.com/indygreg/python-build-standalone', licence: 'MPL-2.0', description: 'Portable Python runtime' },
];

// ── Generate ──
const engines = parseEngines();
const rustDeps = parseCargoToml();
const frontendDeps = parsePackageJson();
const tauriPlugins = parseTauriPlugins();

let md = `<!-- AUTO-GENERATED by scripts/generate-acknowledgements.mjs -->
<!-- Do not edit manually. Run: node scripts/generate-acknowledgements.mjs -->

# Open Source Acknowledgements

MeedyaDL is built on top of many open-source projects. We are grateful to the developers and maintainers of these libraries and tools.

## Download Engines

| Project | Description |
|---------|-------------|
`;
for (const e of engines) {
  const link = e.homepage ? `[${e.name}](${e.homepage})` : e.name;
  md += `| ${link} | ${e.description} |\n`;
}

md += `\n## External Tools\n\n| Project | License | Description |\n|---------|---------|-------------|\n`;
for (const t of EXTERNAL_TOOLS) {
  md += `| [${t.name}](${t.homepage}) | ${licenceLink(t.licence)} | ${t.description} |\n`;
}

md += `\n## Rust Dependencies\n\n| Crate | License |\n|-------|--------|\n`;
for (const d of rustDeps) {
  md += `| [${d.name}](${d.repo}) | ${licenceLink(d.licence)} |\n`;
}

md += `\n## Frontend Dependencies\n\n| Package | License |\n|---------|--------|\n`;
for (const d of frontendDeps) {
  md += `| [${d.name}](${d.repo}) | ${licenceLink(d.licence)} |\n`;
}

md += `\n## Tauri Plugins\n\n| Plugin | License |\n|--------|--------|\n`;
for (const p of tauriPlugins) {
  md += `| ${p} | ${licenceLink('MIT / Apache-2.0')} |\n`;
}

md += `\n---\n\nThis file is auto-generated from \`engines.toml\`, \`Cargo.toml\`, and \`package.json\`.\nRegenerate with: \`node scripts/generate-acknowledgements.mjs\`\n`;

writeFileSync(join(ROOT, 'ACKNOWLEDGEMENTS.md'), md);
console.log(`Generated ACKNOWLEDGEMENTS.md (${engines.length} engines, ${rustDeps.length} Rust crates, ${frontendDeps.length} npm packages, ${tauriPlugins.length} Tauri plugins)`);
