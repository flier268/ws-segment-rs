'use strict';

/**
 * Node entry: re-exports the Rust native addon when built.
 * CLI / API / MCP now live in `crates/novel-segment-cli`.
 */

const { POSTAG } = require('@novel-segment/postag/lib/postag/ids');
const { stringify } = require('@novel-segment/stringify');

let native;
try {
  native = require('novel-segment-native');
} catch (e) {
  native = null;
}

const Segment = native ? native.Segment : function MissingNative() {
  throw new Error(
    'novel-segment JS core has been removed. Build the Rust CLI (`cargo build -p novel-segment-cli`) or the native addon (`cargo build -p novel-segment-napi`).'
  );
};

if (native) {
  Segment.stringify = native.stringify || Segment.stringify;
}

Segment.POSTAG = POSTAG;
Segment.Segment = Segment;
Segment.stringify = Segment.stringify || stringify;

Object.defineProperty(Segment, 'version', {
  get() { return require('./version').version; },
});
Object.defineProperty(Segment, 'versions', {
  get() { return require('./version').versions; },
});

function useDefault(segment) {
  return segment;
}

module.exports = Segment;
module.exports.default = Segment;
module.exports.Segment = Segment;
module.exports.POSTAG = POSTAG;
module.exports.stringify = stringify;
module.exports.useDefault = useDefault;
module.exports.create = native ? native.create : () => new Segment();
module.exports.createSegment = module.exports.create;
