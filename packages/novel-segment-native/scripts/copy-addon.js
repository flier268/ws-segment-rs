const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '../../..');
const dest = path.join(__dirname, '..', 'novel-segment-napi.node');
const candidates = [
  path.join(root, 'target/release/libnovel_segment_napi.so'),
  path.join(root, 'target/debug/libnovel_segment_napi.so'),
  path.join(root, 'target/release/libnovel_segment_napi.dylib'),
  path.join(root, 'target/debug/libnovel_segment_napi.dylib'),
  path.join(root, 'target/release/novel_segment_napi.dll'),
  path.join(root, 'target/debug/novel_segment_napi.dll'),
];

const src = candidates.find((p) => fs.existsSync(p));
if (!src) {
  console.error('ws-segment-rs-napi addon not built. Run: cargo build -p ws-segment-rs-napi');
  process.exit(1);
}
fs.copyFileSync(src, dest);
console.log('copied', src, '->', dest);
