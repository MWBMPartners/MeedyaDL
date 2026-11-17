// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Developer Access Passphrase Record Maker for MeedyaDL
// =====================================================
//
// Turns the passphrase you choose for the hidden developer tools into the
// value of the `DEV_ACCESS_HASH` build secret. The passphrase itself never
// goes in the secret or the app; only this record does.
//
// Usage:
//   node scripts/make-dev-access-hash.mjs
//
// It asks for the passphrase twice without showing it, then prints one line:
//
//   pbkdf2-sha256$600000$<salt as hex>$<derived key as hex>
//
// Put that whole line in the `DEV_ACCESS_HASH` repository secret. Running it
// again with the same passphrase prints a different line (a new random salt);
// either line works.
//
// The app checks a typed passphrase by repeating the same calculation
// (`passphrase_is_accepted` in `src-tauri/src/commands/credentials.rs`), so the
// format here and the format there must stay the same. A Rust test in that file
// holds a record made by the same calculation in Node (with a fixed salt) to
// prove they agree.
//
// Why so many rounds: the record ships inside every copy of the app, so anyone
// can copy it out and guess passphrases on their own computer. Each round makes
// every guess more expensive. 600,000 is OWASP's figure for PBKDF2-SHA256; the
// app refuses a record with fewer than 100,000.

import { pbkdf2Sync, randomBytes } from 'node:crypto';
import { createInterface } from 'node:readline';

/** Rounds of PBKDF2-HMAC-SHA256 written into the record. */
const ROUNDS = 600_000;

/** Bytes of random salt (the app requires at least 16). */
const SALT_BYTES = 16;

/** Bytes of derived key (the app requires exactly 32). */
const KEY_BYTES = 32;

/**
 * Asks a question on the terminal without echoing what is typed.
 *
 * @param {string} question - The prompt to show.
 * @returns {Promise<string>} What was typed, without the final newline.
 */
function askHidden(question) {
  return new Promise((resolve) => {
    const rl = createInterface({ input: process.stdin, output: process.stdout, terminal: true });
    // Readline clears the line and redraws "prompt + what was typed" whenever
    // it refreshes, so the prompt is handed to readline (writing it ourselves
    // first got it wiped straight away), and every redraw shows the prompt
    // alone. Typed characters are never shown.
    rl._writeToOutput = (text) => {
      if (text.startsWith(question)) process.stdout.write(question);
    };
    rl.question(question, (answer) => {
      rl.close();
      process.stdout.write('\n');
      resolve(answer);
    });
  });
}

if (!process.stdin.isTTY) {
  // Typing is hidden by taking over the terminal, which only works when the
  // script is run in one. Piped input could also mix the two answers together.
  console.error('Run this in a terminal and type the passphrase when asked.');
  process.exit(1);
}

const first = await askHidden('Passphrase: ');
const second = await askHidden('Same passphrase again: ');

if (first.trim() === '') {
  // The app refuses an empty or whitespace-only entry, so a record for one
  // could never be used.
  console.error('The passphrase cannot be empty.');
  process.exit(1);
}
if (first !== second) {
  console.error('The two entries were different. Nothing was made.');
  process.exit(1);
}

const salt = randomBytes(SALT_BYTES);
const key = pbkdf2Sync(first, salt, ROUNDS, KEY_BYTES, 'sha256');
console.log(`pbkdf2-sha256$${ROUNDS}$${salt.toString('hex')}$${key.toString('hex')}`);
