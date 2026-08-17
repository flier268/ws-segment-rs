use napi::bindgen_prelude::*;
use napi_derive::napi;
use novel_segment::{DoSegmentOptions, Segment, SegmentOptions};
use std::sync::Mutex;

#[napi(object)]
pub struct JsSegmentOptions {
    pub auto_cjk: Option<bool>,
    pub all_mod: Option<bool>,
    pub node_novel_mode: Option<bool>,
    pub convert_synonym: Option<bool>,
}

#[napi(object)]
pub struct JsDoSegmentOptions {
    pub simple: Option<bool>,
    pub strip_punctuation: Option<bool>,
    pub convert_synonym: Option<bool>,
    pub strip_stopword: Option<bool>,
    pub strip_space: Option<bool>,
}

#[napi(object)]
pub struct JsWord {
    pub w: String,
    pub p: Option<u32>,
    pub f: Option<f64>,
}

#[napi]
pub struct NativeSegment {
    inner: Mutex<Segment>,
}

fn map_err(e: novel_segment::Error) -> Error {
    Error::from_reason(e.to_string())
}

#[napi]
impl NativeSegment {
    #[napi(factory)]
    pub fn create(options: Option<JsSegmentOptions>) -> Result<Self> {
        let o = options.unwrap_or(JsSegmentOptions {
            auto_cjk: Some(true),
            all_mod: Some(true),
            node_novel_mode: Some(false),
            convert_synonym: Some(true),
        });
        let mut seg = Segment::new(SegmentOptions {
            auto_cjk: o.auto_cjk.unwrap_or(true),
            all_mod: o.all_mod.unwrap_or(true),
            node_novel_mode: o.node_novel_mode.unwrap_or(false),
            options_do_segment: DoSegmentOptions {
                convert_synonym: o.convert_synonym.or(Some(true)),
                ..Default::default()
            },
            ..Default::default()
        });
        seg.use_default().map_err(map_err)?;
        Ok(Self {
            inner: Mutex::new(seg),
        })
    }

    #[napi(factory)]
    pub fn with_node_novel_default() -> Result<Self> {
        let seg = Segment::with_node_novel_default().map_err(map_err)?;
        Ok(Self {
            inner: Mutex::new(seg),
        })
    }

    #[napi]
    pub fn do_segment(&self, text: String, options: Option<JsDoSegmentOptions>) -> Result<Vec<JsWord>> {
        let seg = self.inner.lock().map_err(|e| Error::from_reason(e.to_string()))?;
        let opts = options
            .map(|o| DoSegmentOptions {
                simple: o.simple,
                strip_punctuation: o.strip_punctuation,
                convert_synonym: o.convert_synonym,
                strip_stopword: o.strip_stopword,
                strip_space: o.strip_space,
                disable_modules: Vec::new(),
            })
            .unwrap_or_default();
        Ok(seg
            .do_segment(&text, opts)
            .into_iter()
            .map(|w| JsWord {
                w: w.w,
                p: w.p,
                f: w.f,
            })
            .collect())
    }

    #[napi]
    pub fn do_segment_simple(&self, text: String, options: Option<JsDoSegmentOptions>) -> Result<Vec<String>> {
        Ok(self
            .do_segment(text, options)?
            .into_iter()
            .map(|w| w.w)
            .collect())
    }

    #[napi]
    pub fn stringify(&self, text: String, options: Option<JsDoSegmentOptions>) -> Result<String> {
        Ok(self.do_segment_simple(text, options)?.concat())
    }

    #[napi]
    pub fn add_word(&self, spec: String, p: Option<u32>, f: Option<f64>) -> Result<()> {
        let mut seg = self.inner.lock().map_err(|e| Error::from_reason(e.to_string()))?;
        seg.add_word(&spec, p, f).map_err(map_err)?;
        Ok(())
    }

    #[napi]
    pub fn add_synonym(&self, canonical: String, variants: Vec<String>) -> Result<()> {
        let mut seg = self.inner.lock().map_err(|e| Error::from_reason(e.to_string()))?;
        let refs: Vec<&str> = variants.iter().map(|s| s.as_str()).collect();
        seg.add_synonym(&canonical, &refs);
        Ok(())
    }

    #[napi]
    pub fn add_blacklist(&self, word: String) -> Result<()> {
        let mut seg = self.inner.lock().map_err(|e| Error::from_reason(e.to_string()))?;
        seg.add_blacklist(&word);
        Ok(())
    }
}
