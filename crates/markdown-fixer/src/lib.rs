//! markdown-fixer：安全、行導向的 Markdown 語法檢查與自動修正函式庫。
//!
//! 這是 `markdown-autocorrect` skill 的 `markdown-fixer.mjs`（v1.0.0，見
//! `reference/js/`）的 Rust 移植版。行為以 JS 參考實作為準（oracle），規格見
//! repo 根目錄的 `PRD.md`。本 crate 不做任何 I/O。
//!
//! 骨架（skeleton）：以下公開 API 的簽章是各 crate 之間的契約；實作者可以
//! 增加內部模組，但修改公開簽章前必須同步更新 `PRD.md` 與 `docs/library.md`。

/// 本 crate 的版本。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 對齊的 JS 參考實作版本（`reference/js/markdown-fixer.mjs` 的 `VERSION`）。
pub const REFERENCE_VERSION: &str = "1.2.0";

/// 問題嚴重度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// 規則目錄中的一條規則（對應 JS 的 `RULES` 陣列元素）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Rule {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub severity: Severity,
    pub fixable: bool,
    pub summary: &'static str,
}

/// 一個診斷結果（對應 JS 的 `makeProblem` 回傳值）。
///
/// `line` 從 1 起算；`column` 從 1 起算，單位與 JS 字串索引相同（UTF-16 code unit）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Problem {
    pub rule: &'static str,
    pub name: &'static str,
    pub severity: Severity,
    pub fixable: bool,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// 引言巢狀深度上限的預設值（[`Options::max_quote_depth`]）。
pub const DEFAULT_MAX_QUOTE_DEPTH: usize = 32;

/// 超過資源上限（目前是引言巢狀深度）時的處理方式。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum LimitPolicy {
    /// 回報 `MDX086`（error、blocking），`fix_document` 拒絕修正整份文件。預設值。
    #[default]
    Error,
    /// 把超過上限的行當成不透明的保護行：不當成 reference definition、不修改、不診斷。
    Ignore,
}

/// 檢查與修正選項（對應 JS 的 `options`）。
///
/// 請用 `Options { ..Default::default() }` 建構，日後新增欄位才不會破壞相容性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// 只啟用這些規則；`None` 表示全部啟用。
    pub rules: Option<Vec<String>>,
    /// 停用這些規則。
    pub ignore_rules: Option<Vec<String>>,
    /// `MDX090` 行長上限；0 表示停用 MDX090。
    pub max_line_length: usize,
    /// 引言（`>`）巢狀深度上限，超過時依 [`Options::on_limit`] 處理；0 表示不限制。
    /// 對應 JS 的 `maxQuoteDepth`，預設 [`DEFAULT_MAX_QUOTE_DEPTH`]。
    pub max_quote_depth: usize,
    /// 超過資源上限時的處理方式。對應 JS 的 `onLimit`。
    pub on_limit: LimitPolicy,
    /// 渲染保護（Rust 專有）：修正後以 CommonMark/GFM 渲染比對，會意外改變渲染結果的
    /// 修正不套用，改記錄在 [`FixResult::skipped`]。預設開啟；需要 `render-guard`
    /// feature（預設啟用），未啟用該 feature 時這個欄位沒有作用。
    /// 關閉後行為與 JS 參考實作逐位元組相同（parity 測試用這個模式）。
    pub render_guard: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            rules: None,
            ignore_rules: None,
            max_line_length: 0,
            max_quote_depth: DEFAULT_MAX_QUOTE_DEPTH,
            on_limit: LimitPolicy::Error,
            render_guard: true,
        }
    }
}

/// 未知的規則 ID 等選項錯誤。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionsError {
    UnknownRule(String),
}

impl std::fmt::Display for OptionsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionsError::UnknownRule(id) => write!(f, "Unknown rule: {id}"),
        }
    }
}

impl std::error::Error for OptionsError {}

impl Options {
    /// 檢查 `rules` / `ignore_rules` 中的 ID 是否都存在於規則目錄。
    pub fn validate(&self) -> Result<(), OptionsError> {
        rules::validate(self)
    }
}

/// 因渲染保護而沒有套用的修正（[`FixResult::skipped`] 的元素）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SkippedFix {
    /// 被擋下的規則 ID。
    pub rule: &'static str,
    /// 原文中受影響的第一行（從 1 起算）。
    pub line: usize,
    /// 擋下的原因（英文，給人與 agent 閱讀）。
    pub reason: String,
}

/// `fix_document` 的結果（對應 JS 的 `fixDocument` 回傳值）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FixResult {
    /// 修正後的文字；`refused` 或未變更時等於輸入原文。
    pub text: String,
    pub changed: bool,
    /// 因結構錯誤（MDX081/083/053/085/086）或 frontmatter 陷阱而拒絕修正。
    pub refused: bool,
    /// 拒絕原因；未拒絕時為空字串。
    pub reason: String,
    /// 渲染保護擋下、沒有套用的修正（Rust 專有；`render_guard` 關閉時一律為空）。
    /// 序列化時空陣列會省略，讓 JSON 形狀與 JS 參考實作相同。
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    pub skipped: Vec<SkippedFix>,
}

/// 完整規則目錄，順序與 JS 的 `RULES` 相同。
pub fn rules() -> &'static [Rule] {
    rules::RULES
}

/// 依 ID 查規則。
pub fn rule(id: &str) -> Option<&'static Rule> {
    rules().iter().find(|r| r.id == id)
}

/// 檢查文字，回傳與 JS `diagnose(text, options)` 相同順序、相同內容的問題清單。
pub fn diagnose(text: &str, options: &Options) -> Vec<Problem> {
    diagnose::diagnose(text, options)
}

/// 套用安全修正，行為與 JS `fixDocument(text, options)` 相同。
pub fn fix_document(text: &str, options: &Options) -> FixResult {
    #[cfg(feature = "render-guard")]
    if options.render_guard {
        return guard::fix_document(text, options);
    }
    fix::fix_document(text, options)
}

/// Markdown 內容偵測（給剪貼簿、dedent-paste 等「輸入不一定是 Markdown」的情境）。
pub mod detect;

// 內部模組。行內處理一律以 UTF-16 code unit（`Vec<u16>`）運算，與 JS 字串索引一致。
mod blank_lines;
mod cjk_emphasis;
mod diagnose;
mod fix;
#[cfg(feature = "render-guard")]
mod guard;
mod jsregex;
mod jsstr;
mod line_fix;
mod mask;
#[cfg(test)]
mod parity_probe;
#[cfg(test)]
mod regex_hazards;
mod rules;
mod scan;
mod structure;
mod table;
mod unicode_tables;
