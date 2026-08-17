#!/usr/bin/env node
const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

function rustBin() {
  const candidates = [
    path.resolve(__dirname, '../../../target/release/ws-segment-rs'),
    path.resolve(__dirname, '../../../target/debug/ws-segment-rs'),
  ];
  for (const p of candidates) {
    if (fs.existsSync(p)) return p;
  }
  return 'ws-segment-rs';
}

const r = spawnSync(rustBin(), ['process', ...process.argv.slice(2)], { stdio: 'inherit' });
if (r.error && r.error.code === 'ENOENT') {
  console.error('ws-segment-rs Rust binary not found. Build with: cargo build -p ws-segment-rs-cli --release');
  process.exit(1);
}
process.exit(r.status == null ? 1 : r.status);
