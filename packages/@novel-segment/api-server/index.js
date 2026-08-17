'use strict';

/**
 * Thin launcher. The HTTP API is the Rust `novel-segment serve` binary.
 */
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

function rustBin() {
  const candidates = [
    path.resolve(__dirname, '../../../target/release/novel-segment'),
    path.resolve(__dirname, '../../../target/debug/novel-segment'),
  ];
  for (const p of candidates) {
    if (fs.existsSync(p)) return p;
  }
  return 'novel-segment';
}

function listen(bind) {
  const child = spawn(rustBin(), ['serve', '--bind', bind || '127.0.0.1:3000'], {
    stdio: 'inherit',
  });
  child.on('error', (e) => {
    console.error(e.message);
    console.error('Build with: cargo build -p novel-segment-cli --release');
    process.exit(1);
  });
  return child;
}

if (require.main === module) {
  listen(process.env.PORT ? `0.0.0.0:${process.env.PORT}` : process.argv[2]);
}

module.exports = { listen };
