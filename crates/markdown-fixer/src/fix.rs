//! `fixDocument` 的移植（JS 2099–2160 行）。

use crate::blank_lines::{
    apply_blank_line_fixes, apply_blank_line_normalization, apply_trailing_whitespace,
};
use crate::cjk_emphasis::apply_cjk_emphasis_fixes;
use crate::jsstr::{self, JsString};
use crate::line_fix::transform_line;
use crate::rules::Enabled;
use crate::scan::{
    Scan, normalize_for_analysis, scan_document_with, starts_frontmatter_after_leading_blanks,
};
use crate::table::apply_table_delimiter_fixes;
use crate::{FixResult, Options};

/// 修正會讓檔首的 `---` 變成 YAML frontmatter 時的拒絕原因（JS `FRONTMATTER_TRAP`）。
pub(crate) const FRONTMATTER_TRAP: &str = "A fix would turn a leading --- into YAML frontmatter. Fix the first lines manually; use *** if the line was meant as a horizontal rule.";

/// 修正迴圈的最大輪數。
const MAX_PASSES: usize = 6;

const BOM: u16 = 0xFEFF;
const CR: u16 = b'\r' as u16;
const LF: u16 = b'\n' as u16;

/// 會阻擋修正的結構問題（MDX081、MDX083、MDX053、MDX085、MDX086）的訊息，不受 `rules`／`ignore_rules` 影響。
pub(crate) fn blocking_reason(scan: &Scan) -> Option<&str> {
    scan.issues
        .iter()
        .find(|p| matches!(p.rule, "MDX081" | "MDX083" | "MDX053" | "MDX085" | "MDX086"))
        .map(|p| p.message.as_str())
}

fn refused(text: &str, reason: &str) -> FixResult {
    FixResult {
        text: text.to_string(),
        changed: false,
        refused: true,
        reason: reason.to_string(),
        skipped: Vec::new(),
    }
}

/// JS `fixDocument(text, options)`。
pub(crate) fn fix_document(text: &str, options: &Options) -> FixResult {
    let enabled = Enabled::new(options);
    let original: JsString = jsstr::u(text);
    let has_cr = original.contains(&CR);
    let has_crlf = original.windows(2).any(|w| w == [CR, LF]);
    let original_eol: &[u16] = if has_crlf {
        &[CR, LF]
    } else if has_cr {
        &[CR]
    } else {
        &[LF]
    };
    let bom_len = original.iter().take_while(|&&c| c == BOM).count();
    let had_bom = bom_len > 0;
    let normalized_original = normalize_for_analysis(&original);
    let encoding_changed = (had_bom && enabled.is("MDX001")) || (has_cr && enabled.is("MDX002"));

    let mut working = normalized_original.clone();
    let initial_scan = scan_document_with(&working, enabled.limits);
    let had_frontmatter = initial_scan.frontmatter_end.is_some();
    if let Some(reason) = blocking_reason(&initial_scan) {
        return refused(text, reason);
    }
    if !had_frontmatter && enabled.is("MDX006") && starts_frontmatter_after_leading_blanks(&working)
    {
        return refused(text, FRONTMATTER_TRAP);
    }

    for _ in 0..MAX_PASSES {
        let before = working.clone();
        let scan = scan_document_with(&working, enabled.limits);
        if let Some(reason) = blocking_reason(&scan) {
            return refused(text, reason);
        }
        if !had_frontmatter && scan.frontmatter_end.is_some() {
            return refused(text, FRONTMATTER_TRAP);
        }
        if !had_frontmatter
            && enabled.is("MDX006")
            && starts_frontmatter_after_leading_blanks(&working)
        {
            return refused(text, FRONTMATTER_TRAP);
        }
        let lines: Vec<JsString> = scan
            .lines
            .iter()
            .zip(&scan.info)
            .enumerate()
            .map(|(i, (line, info))| transform_line(line, info, i + 1, &enabled).line)
            .collect();
        working = jsstr::join_lines(&lines);
        working = apply_cjk_emphasis_fixes(&working, &enabled);
        working = apply_table_delimiter_fixes(&working, &enabled);
        working = apply_blank_line_fixes(&working, &enabled);
        working = apply_trailing_whitespace(&working, &enabled);
        working = apply_blank_line_normalization(&working, &enabled);
        if working == before {
            break;
        }
    }

    if !had_frontmatter
        && scan_document_with(&working, enabled.limits)
            .frontmatter_end
            .is_some()
    {
        return refused(text, FRONTMATTER_TRAP);
    }
    if !had_frontmatter && enabled.is("MDX006") && starts_frontmatter_after_leading_blanks(&working)
    {
        return refused(text, FRONTMATTER_TRAP);
    }
    if working == normalized_original && !encoding_changed {
        return FixResult {
            text: text.to_string(),
            changed: false,
            refused: false,
            reason: String::new(),
            skipped: Vec::new(),
        };
    }
    if had_bom && !enabled.is("MDX001") {
        let mut with_bom = original[..bom_len].to_vec();
        with_bom.extend_from_slice(&working);
        working = with_bom;
    }
    if !enabled.is("MDX002") && original_eol != [LF] {
        let mut restored = Vec::with_capacity(working.len() + working.len() / 16);
        for &c in &working {
            if c == LF {
                restored.extend_from_slice(original_eol);
            } else {
                restored.push(c);
            }
        }
        working = restored;
    }

    let changed = working != original;
    FixResult {
        text: jsstr::to_string(&working),
        changed,
        refused: false,
        reason: String::new(),
        skipped: Vec::new(),
    }
}
