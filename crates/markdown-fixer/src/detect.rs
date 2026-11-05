//! Markdown 內容偵測。規格見 PRD.md〈6.6 Markdown 偵測〉。

/// 偵測結果。
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Detection {
    /// 是否判定為 Markdown。
    pub is_markdown: bool,
    /// 正向訊號分數總和。
    pub score: i32,
    /// 命中的訊號名稱（穩定字串，例如 `"atx-heading"`、`"fenced-code"`）。
    pub signals: Vec<&'static str>,
}

/// 保守地判斷一段文字是否「看起來像 Markdown」。規格見 PRD.md〈Markdown 偵測〉。
pub fn looks_like_markdown(text: &str) -> Detection {
    let _ = text;
    todo!("skeleton: implement in detect agent")
}
