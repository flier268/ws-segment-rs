'use strict';

const fs = require('fs');
const path = require('path');

function resolveAddon() {
  const names = [
    path.join(__dirname, 'novel-segment-napi.node'),
    path.resolve(__dirname, '../../target/release/novel-segment-napi.node'),
    path.resolve(__dirname, '../../target/debug/novel-segment-napi.node'),
    path.resolve(__dirname, '../../target/release/libnovel_segment_napi.so'),
    path.resolve(__dirname, '../../target/debug/libnovel_segment_napi.so'),
  ];
  for (const p of names) {
    if (fs.existsSync(p)) {
      return require(p);
    }
  }
  throw new Error(
    'novel-segment native addon not found. Build with: cargo build -p novel-segment-napi && node packages/novel-segment-native/scripts/copy-addon.js'
  );
}

const { NativeSegment } = resolveAddon();

function mapDoOpts(options) {
  if (!options || typeof options !== 'object') {
    return undefined;
  }
  return {
    simple: options.simple,
    stripPunctuation: options.stripPunctuation,
    convertSynonym: options.convertSynonym,
    stripStopword: options.stripStopword,
    stripSpace: options.stripSpace,
  };
}

function stringifyWords(words) {
  if (!words) {
    return '';
  }
  if (typeof words === 'string') {
    return words;
  }
  return [].concat(words).map((w) => (typeof w === 'string' ? w : (w && w.w) || '')).join('');
}

/**
 * Duck-typed Segment used by Node CLI / API / MCP.
 */
class Segment {
  constructor(options = {}) {
    this._native = NativeSegment.create({
      autoCjk: options.autoCjk !== false,
      allMod: options.all_mod !== false && options.allMod !== false,
      nodeNovelMode: !!(options.nodeNovelMode || options.node_novel_mode),
      convertSynonym: options.optionsDoSegment
        ? options.optionsDoSegment.convertSynonym !== false
        : options.convertSynonym !== false,
    });
    this.options = options;
    this.inited = true;
  }

  static withNodeNovelDefault() {
    const s = Object.create(Segment.prototype);
    s._native = NativeSegment.withNodeNovelDefault();
    s.options = { nodeNovelMode: true, autoCjk: true, all_mod: true };
    s.inited = true;
    return s;
  }

  static stringify(words) {
    return stringifyWords(words);
  }

  doSegment(text, options) {
    return this._native.doSegment(String(text), mapDoOpts(options));
  }

  stringify(wordsOrText, options) {
    if (Array.isArray(wordsOrText)) {
      return stringifyWords(wordsOrText);
    }
    if (typeof wordsOrText === 'string') {
      return this._native.stringify(wordsOrText, mapDoOpts(options));
    }
    return stringifyWords(wordsOrText);
  }

  addWord(spec, p, f) {
    this._native.addWord(String(spec), p, f);
    return this;
  }

  addSynonym(canonical, variants) {
    this._native.addSynonym(String(canonical), [].concat(variants));
    return this;
  }

  addBlacklist(word) {
    this._native.addBlacklist(String(word));
    return this;
  }
}

function create(options) {
  return new Segment(options);
}

module.exports = {
  Segment,
  NativeSegment,
  create,
  createSegment: create,
  stringify: stringifyWords,
};
module.exports.default = module.exports;
