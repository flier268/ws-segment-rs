'use strict';

const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

function rustBin() {
  const roots = [
    path.resolve(__dirname, '../../../target/release/novel-segment'),
    path.resolve(__dirname, '../../../target/debug/novel-segment'),
  ];
  for (const p of roots) {
    if (fs.existsSync(p)) return p;
  }
  return 'novel-segment';
}

function run(extraArgs) {
  const args = extraArgs.concat(process.argv.slice(2));
  const r = spawnSync(rustBin(), args, { stdio: 'inherit' });
  if (r.error && r.error.code === 'ENOENT') {
    console.error('novel-segment Rust binary not found. Build with: cargo build -p novel-segment-cli --release');
    process.exit(1);
  }
  process.exit(r.status == null ? 1 : r.status);
}

module.exports = { run, rustBin };
