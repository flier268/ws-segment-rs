//! Chinese word segmentation for web novels.
//!
//! This crate is a Rust port of the TypeScript `novel-segment` package.
//! Other Rust programs should depend on this crate only:
//!
//! ```toml
//! novel-segment = { path = "../ws-segment-rs/crates/novel-segment" }
//! ```
//!
//! ```rust,no_run
//! use novel_segment::{DoSegmentOptions, Segment, SegmentOptions};
//!
//! fn main() -> Result<(), novel_segment::Error> {
//!     let mut seg = Segment::new(SegmentOptions { auto_cjk: true, ..Default::default() });
//!     seg.use_default()?;
//!     let words = seg.do_segment("這是一個中文分詞模組。", DoSegmentOptions::default());
//!     for w in words {
//!         println!("{} {:?}", w.w, w.p);
//!     }
//!     Ok(())
//! }
//! ```

mod colors;
mod datetime;
mod zht;
mod dict;
mod error;
mod loader;
mod names;
mod optimizers;
mod options;
mod pipeline;
mod postag;
mod segment;
mod service;
mod stopword;
mod text;
mod tokenizers;
mod word;

pub use error::{Error, Result};
pub use options::{DoSegmentOptions, SegmentOptions};
pub use pipeline::{Optimizer, Tokenizer};
pub use postag::POSTAG;
pub use segment::Segment;
pub use service::{
    apply_extras, convert_joined, crlf_normalize, load_json_config, process_text, run_test,
    segment_words, ExpectedItem, SegmentMode, TestRequest, TestResult,
};
pub use word::{stringify, stringify_list, Word};
