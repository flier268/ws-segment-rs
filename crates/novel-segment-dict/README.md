# ws-segment-rs-dict

Default dictionary paths and CJK variant expansion for [`ws-segment-rs`](../novel-segment).

`text_list` / `auto_char` follow `@lazy-cjk/zh-table-list` (`safe: true`): zh-table-alias groups, jp-table-convert `TABLE_SAFE`, and OpenCC ST/TS/JP/TW/HK (including multi-target rows such as `藉 → 藉 借`). `arr_cjk` matches the whole-string convert used by CHS_NAMES / DATETIME / COLORS.
