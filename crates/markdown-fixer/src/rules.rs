//! 規則目錄、選項驗證與規則啟用判斷（對應 JS 的 `RULES`、`makeProblem`、`createEnabled`）。

use std::collections::HashSet;

use crate::{Options, OptionsError, Problem, Rule, Severity};

/// 規則目錄，順序與 JS `RULES` 相同（由 oracle 產生，不要手改內容）。
pub(crate) static RULES: &[Rule] = &[
    Rule {
        id: "MDX001",
        name: "bom",
        category: "encoding",
        severity: Severity::Warning,
        fixable: true,
        summary: "Remove a UTF-8 BOM from the start of the file.",
    },
    Rule {
        id: "MDX002",
        name: "line-endings",
        category: "encoding",
        severity: Severity::Warning,
        fixable: true,
        summary: "Normalize CRLF/CR line endings to LF.",
    },
    Rule {
        id: "MDX003",
        name: "final-newline",
        category: "whitespace",
        severity: Severity::Warning,
        fixable: true,
        summary: "End the file with exactly one newline.",
    },
    Rule {
        id: "MDX004",
        name: "trailing-whitespace",
        category: "whitespace",
        severity: Severity::Warning,
        fixable: true,
        summary: "Remove trailing whitespace, preserving two-space hard breaks.",
    },
    Rule {
        id: "MDX005",
        name: "multiple-blank-lines",
        category: "whitespace",
        severity: Severity::Warning,
        fixable: true,
        summary: "Collapse two or more blank lines to one.",
    },
    Rule {
        id: "MDX006",
        name: "leading-blank-lines",
        category: "whitespace",
        severity: Severity::Warning,
        fixable: true,
        summary: "Remove blank lines before the first block.",
    },
    Rule {
        id: "MDX007",
        name: "hard-tab",
        category: "whitespace",
        severity: Severity::Warning,
        fixable: false,
        summary: "Avoid tab characters in prose and indentation.",
    },
    Rule {
        id: "MDX008",
        name: "non-breaking-space",
        category: "whitespace",
        severity: Severity::Warning,
        fixable: true,
        summary: "Replace NBSP (U+00A0) with a normal space outside code.",
    },
    Rule {
        id: "MDX009",
        name: "zero-width-character",
        category: "encoding",
        severity: Severity::Warning,
        fixable: false,
        summary: "Detect stray zero-width spaces and mid-file BOMs.",
    },
    Rule {
        id: "MDX010",
        name: "atx-missing-space",
        category: "heading",
        severity: Severity::Warning,
        fixable: true,
        summary: "Add a space after the ATX heading marker (#).",
    },
    Rule {
        id: "MDX011",
        name: "atx-multiple-space",
        category: "heading",
        severity: Severity::Warning,
        fixable: true,
        summary: "Use one space after the ATX heading marker.",
    },
    Rule {
        id: "MDX012",
        name: "atx-closing-hashes",
        category: "heading",
        severity: Severity::Warning,
        fixable: true,
        summary: "Remove optional closing hashes from an ATX heading.",
    },
    Rule {
        id: "MDX013",
        name: "heading-blank-lines",
        category: "heading",
        severity: Severity::Warning,
        fixable: true,
        summary: "Separate top-level headings from surrounding paragraphs with blank lines.",
    },
    Rule {
        id: "MDX014",
        name: "heading-indent",
        category: "heading",
        severity: Severity::Info,
        fixable: false,
        summary: "Verify indented headings are intentional (for example, inside a list).",
    },
    Rule {
        id: "MDX015",
        name: "heading-increment",
        category: "heading",
        severity: Severity::Warning,
        fixable: false,
        summary: "Do not skip heading levels.",
    },
    Rule {
        id: "MDX016",
        name: "empty-heading",
        category: "heading",
        severity: Severity::Warning,
        fixable: false,
        summary: "Remove or fill an empty heading.",
    },
    Rule {
        id: "MDX017",
        name: "heading-trailing-punctuation",
        category: "heading",
        severity: Severity::Info,
        fixable: false,
        summary: "Avoid ending headings with sentence punctuation.",
    },
    Rule {
        id: "MDX018",
        name: "first-heading-h1",
        category: "heading",
        severity: Severity::Info,
        fixable: false,
        summary: "Start the document outline with an H1.",
    },
    Rule {
        id: "MDX019",
        name: "multiple-h1",
        category: "heading",
        severity: Severity::Info,
        fixable: false,
        summary: "Prefer one H1 per document; repeated H1s are sometimes intentional in changelogs and task logs.",
    },
    Rule {
        id: "MDX020",
        name: "blockquote-space",
        category: "blockquote",
        severity: Severity::Warning,
        fixable: true,
        summary: "Put one space after each blockquote marker.",
    },
    Rule {
        id: "MDX030",
        name: "list-marker-space",
        category: "list",
        severity: Severity::Warning,
        fixable: true,
        summary: "Add a space between a list marker and its content.",
    },
    Rule {
        id: "MDX031",
        name: "list-marker-style",
        category: "list",
        severity: Severity::Warning,
        fixable: true,
        summary: "Use one bullet marker per list.",
    },
    Rule {
        id: "MDX032",
        name: "list-blank-lines",
        category: "list",
        severity: Severity::Warning,
        fixable: false,
        summary: "Separate lists from surrounding paragraphs with blank lines.",
    },
    Rule {
        id: "MDX035",
        name: "empty-list-item",
        category: "list",
        severity: Severity::Warning,
        fixable: false,
        summary: "Remove empty list items or give them content.",
    },
    Rule {
        id: "MDX040",
        name: "emphasis-inner-space",
        category: "emphasis",
        severity: Severity::Warning,
        fixable: true,
        summary: "Remove spaces just inside emphasis delimiters.",
    },
    Rule {
        id: "MDX041",
        name: "unmatched-emphasis",
        category: "emphasis",
        severity: Severity::Warning,
        fixable: false,
        summary: "Check for a possible unmatched strong or strikethrough delimiter.",
    },
    Rule {
        id: "MDX042",
        name: "cjk-emphasis-unpaired",
        category: "emphasis",
        severity: Severity::Warning,
        fixable: true,
        summary: "Report ** or * that cannot pair next to CJK or full-width punctuation; move closing-side punctuation outside when safe.",
    },
    Rule {
        id: "MDX050",
        name: "fence-language",
        category: "fence",
        severity: Severity::Info,
        fixable: false,
        summary: "Add a language to fenced code blocks when one is known.",
    },
    Rule {
        id: "MDX051",
        name: "fence-unclosed",
        category: "fence",
        severity: Severity::Error,
        fixable: false,
        summary: "Close every fenced code block.",
    },
    Rule {
        id: "MDX052",
        name: "fence-blank-lines",
        category: "fence",
        severity: Severity::Warning,
        fixable: true,
        summary: "Surround top-level fenced code blocks with blank lines.",
    },
    Rule {
        id: "MDX053",
        name: "fence-invalid",
        category: "fence",
        severity: Severity::Error,
        fixable: false,
        summary: "Use a valid fence info string.",
    },
    Rule {
        id: "MDX060",
        name: "table-delimiter-width",
        category: "table",
        severity: Severity::Warning,
        fixable: true,
        summary: "Use at least three hyphens in each table delimiter cell.",
    },
    Rule {
        id: "MDX061",
        name: "table-column-count",
        category: "table",
        severity: Severity::Error,
        fixable: false,
        summary: "Keep the same number of cells in every table row.",
    },
    Rule {
        id: "MDX062",
        name: "table-blank-lines",
        category: "table",
        severity: Severity::Warning,
        fixable: true,
        summary: "Surround top-level tables with blank lines.",
    },
    Rule {
        id: "MDX063",
        name: "table-pipe-in-code",
        category: "table",
        severity: Severity::Warning,
        fixable: false,
        summary: "Escape pipes inside code spans or URLs in table cells.",
    },
    Rule {
        id: "MDX070",
        name: "bare-url",
        category: "link",
        severity: Severity::Info,
        fixable: false,
        summary: "Prefer an explicit link or angle-bracket autolink for bare URLs.",
    },
    Rule {
        id: "MDX071",
        name: "empty-link",
        category: "link",
        severity: Severity::Warning,
        fixable: false,
        summary: "Give links meaningful text and a destination.",
    },
    Rule {
        id: "MDX072",
        name: "link-space-before-destination",
        category: "link",
        severity: Severity::Info,
        fixable: false,
        summary: "A space between ] and ( prevents link parsing.",
    },
    Rule {
        id: "MDX073",
        name: "reference-missing-definition",
        category: "link",
        severity: Severity::Warning,
        fixable: false,
        summary: "Define every explicit reference-link label.",
    },
    Rule {
        id: "MDX074",
        name: "reference-duplicate-definition",
        category: "link",
        severity: Severity::Warning,
        fixable: false,
        summary: "Define each reference-link label once.",
    },
    Rule {
        id: "MDX075",
        name: "image-alt",
        category: "link",
        severity: Severity::Warning,
        fixable: false,
        summary: "Add image alt text or confirm the image is decorative.",
    },
    Rule {
        id: "MDX080",
        name: "setext-ambiguity",
        category: "structure",
        severity: Severity::Warning,
        fixable: false,
        summary: "A line of --- or === after a paragraph is a Setext heading, not a rule.",
    },
    Rule {
        id: "MDX081",
        name: "frontmatter-unclosed",
        category: "structure",
        severity: Severity::Error,
        fixable: false,
        summary: "Close YAML frontmatter with --- or ... .",
    },
    Rule {
        id: "MDX082",
        name: "frontmatter-blank-line",
        category: "structure",
        severity: Severity::Warning,
        fixable: true,
        summary: "Put a blank line after frontmatter.",
    },
    Rule {
        id: "MDX083",
        name: "html-comment-unclosed",
        category: "structure",
        severity: Severity::Error,
        fixable: false,
        summary: "Close every HTML comment.",
    },
    Rule {
        id: "MDX084",
        name: "fullwidth-markdown-syntax",
        category: "structure",
        severity: Severity::Warning,
        fixable: false,
        summary: "Use ASCII punctuation for Markdown syntax.",
    },
    Rule {
        id: "MDX085",
        name: "math-unclosed",
        category: "structure",
        severity: Severity::Error,
        fixable: false,
        summary: "Close every $$ math block.",
    },
    Rule {
        id: "MDX090",
        name: "line-length",
        category: "style",
        severity: Severity::Info,
        fixable: false,
        summary: "Keep prose lines within the configured limit.",
    },
];

/// 依 ID 查規則（JS 的 `RULE_BY_ID.get`）。
pub(crate) fn find(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

/// `makeProblem` 的 `overrides`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Overrides {
    pub(crate) severity: Option<Severity>,
    pub(crate) fixable: Option<bool>,
}

impl Overrides {
    /// 不覆寫（JS 的 `{}`）。
    pub(crate) const NONE: Overrides = Overrides {
        severity: None,
        fixable: None,
    };

    /// `{ fixable: false }`。
    pub(crate) const NOT_FIXABLE: Overrides = Overrides {
        severity: None,
        fixable: Some(false),
    };

    /// `{ severity }`。
    pub(crate) const fn severity(severity: Severity) -> Overrides {
        Overrides {
            severity: Some(severity),
            fixable: None,
        }
    }
}

/// JS `makeProblem(ruleId, line, column, message, overrides)`。
///
/// `line`、`column` 從 1 起算，小於 1 時夾成 1（JS 的 `Math.max(1, …)` 與 `column || 1`）。
/// `column` 單位是 UTF-16 code unit。未知的規則 ID 是程式錯誤；為了不 panic，
/// 這時 name 為空字串、severity 為 warning。
pub(crate) fn make_problem(
    rule_id: &'static str,
    line: usize,
    column: usize,
    message: impl Into<String>,
    overrides: Overrides,
) -> Problem {
    let rule = find(rule_id);
    debug_assert!(rule.is_some(), "unknown rule {rule_id}");
    let (name, severity, fixable) = rule.map_or(("", Severity::Warning, false), |r| {
        (r.name, r.severity, r.fixable)
    });
    Problem {
        rule: rule.map_or(rule_id, |r| r.id),
        name,
        severity: overrides.severity.unwrap_or(severity),
        fixable: overrides.fixable.unwrap_or(fixable),
        line: line.max(1),
        column: column.max(1),
        message: message.into(),
    }
}

/// JS `createEnabled(options)` 回傳的 `isEnabled`。
#[derive(Debug, Clone)]
pub(crate) struct Enabled {
    include: Option<HashSet<String>>,
    ignore: Option<HashSet<String>>,
    max_line_length: usize,
}

impl Enabled {
    /// `createEnabled(options)`。注意 `rules: Some(vec![])` 會停用全部規則（JS 空陣列是 truthy）。
    pub(crate) fn new(options: &Options) -> Enabled {
        Enabled {
            include: options.rules.as_ref().map(|v| v.iter().cloned().collect()),
            ignore: options
                .ignore_rules
                .as_ref()
                .map(|v| v.iter().cloned().collect()),
            max_line_length: options.max_line_length,
        }
    }

    /// `isEnabled(ruleId)`。
    pub(crate) fn is(&self, rule_id: &str) -> bool {
        if let Some(include) = &self.include {
            if !include.contains(rule_id) {
                return false;
            }
        }
        if let Some(ignore) = &self.ignore {
            if ignore.contains(rule_id) {
                return false;
            }
        }
        if rule_id == "MDX090" && self.max_line_length == 0 {
            return false;
        }
        true
    }
}

/// `Options::validate`：依 `rules`、`ignore_rules` 的順序回報第一個未知 ID（與 JS `parseArgs` 相同）。
pub(crate) fn validate(options: &Options) -> Result<(), OptionsError> {
    let ids = options
        .rules
        .iter()
        .flatten()
        .chain(options.ignore_rules.iter().flatten());
    for id in ids {
        if find(id).is_none() {
            return Err(OptionsError::UnknownRule(id.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_shape() {
        assert_eq!(RULES.len(), 48);
        assert_eq!(RULES.iter().filter(|r| r.fixable).count(), 20);
        assert_eq!(RULES[0].id, "MDX001");
        assert_eq!(RULES[RULES.len() - 1].id, "MDX090");
    }

    #[test]
    fn enabled_semantics() {
        let all = Enabled::new(&Options::default());
        assert!(all.is("MDX001"));
        assert!(!all.is("MDX090"));
        let none = Enabled::new(&Options {
            rules: Some(vec![]),
            ..Options::default()
        });
        assert!(!none.is("MDX001"));
        let only = Enabled::new(&Options {
            rules: Some(vec!["MDX090".into()]),
            max_line_length: 80,
            ..Options::default()
        });
        assert!(only.is("MDX090"));
        assert!(!only.is("MDX001"));
    }

    #[test]
    fn validate_reports_first_unknown() {
        let options = Options {
            rules: Some(vec!["MDX001".into(), "NOPE".into()]),
            ignore_rules: Some(vec!["ALSO".into()]),
            max_line_length: 0,
            ..Default::default()
        };
        assert_eq!(
            validate(&options),
            Err(OptionsError::UnknownRule("NOPE".into()))
        );
    }

    #[test]
    fn make_problem_clamps() {
        let p = make_problem("MDX004", 0, 0, "m", Overrides::NOT_FIXABLE);
        assert_eq!((p.line, p.column, p.fixable), (1, 1, false));
        assert_eq!(p.name, "trailing-whitespace");
    }
}
