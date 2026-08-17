# novel-segment-native

Node-API 綁定，讓 MCP / API / CLI 繼續用 Node.js，分詞走 Rust 核心。

```bash
cargo build -p ws-segment-rs-napi
node packages/novel-segment-native/scripts/copy-addon.js
```

```js
const { create } = require('novel-segment-native');
const seg = create({ autoCjk: true, allMod: true, convertSynonym: true });
console.log(seg.doSegment('这是一个中文分词模块。'));
```
