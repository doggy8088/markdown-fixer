//! 表格偵測與分隔列修正（JS 1028–1138、1944–1954 行）。

use crate::jsregex::re;
use crate::jsstr::{self, JsString, trim};
use crate::rules::Enabled;
use crate::scan::{
    LineInfo, is_atx_heading, is_blank, is_indented_code_line, is_list_line, is_protected_line,
    is_thematic_break, parse_quote_prefix, scan_document,
};

const PIPE: u16 = b'|' as u16;
const BACKSLASH: u16 = b'\\' as u16;
const BACKTICK: u16 = b'`' as u16;

/// JS `splitTableCells(line)`：每個 cell 都已 `trim()`。
pub(crate) fn split_table_cells(line: &[u16]) -> Vec<JsString> {
    let mut value = trim(line);
    if value.first() == Some(&PIPE) {
        value = &value[1..];
    }
    if value.last() == Some(&PIPE) && !jsstr::ends_with(value, "\\|") {
        value = &value[..value.len() - 1];
    }

    let mut cells = Vec::new();
    let mut current: JsString = Vec::new();
    let mut code_run = 0;
    let mut i = 0;
    while i < value.len() {
        let ch = value[i];
        if ch == BACKSLASH && i + 1 < value.len() {
            current.push(ch);
            current.push(value[i + 1]);
            i += 2;
            continue;
        }
        if ch == BACKTICK {
            let mut run = 1;
            while jsstr::at(value, i + run) == Some(BACKTICK) {
                run += 1;
            }
            if code_run == 0 {
                code_run = run;
            } else if code_run == run {
                code_run = 0;
            }
            current.extend(std::iter::repeat_n(BACKTICK, run));
            i += run;
            continue;
        }
        if ch == PIPE && code_run == 0 {
            cells.push(trim(&current).to_vec());
            current.clear();
            i += 1;
            continue;
        }
        current.push(ch);
        i += 1;
    }
    cells.push(trim(&current).to_vec());
    cells
}

/// JS `isTableDelimiterRow(line)`。
pub(crate) fn is_table_delimiter_row(line: &[u16]) -> bool {
    if !line.contains(&PIPE) {
        return false;
    }
    let cells = split_table_cells(line);
    !cells.is_empty() && cells.iter().all(|cell| re!(r"^:?-+:?$").test(cell))
}

fn strip_up_to_3_spaces(line: &[u16]) -> &[u16] {
    let n = line
        .iter()
        .take(3)
        .take_while(|&&c| c == b' ' as u16)
        .count();
    &line[n..]
}

/// JS `isTableHeaderCandidate(line)`。
pub(crate) fn is_table_header_candidate(line: &[u16]) -> bool {
    if line.is_empty() || is_indented_code_line(line) {
        return false;
    }
    let content = strip_up_to_3_spaces(line);
    if content.first() == Some(&(b'>' as u16)) {
        return false;
    }
    if is_atx_heading(content)
        || is_list_line(content)
        || is_thematic_break(content)
        || jsstr::starts_with(content, "```")
        || jsstr::starts_with(content, "~~~")
    {
        return false;
    }
    content.contains(&PIPE)
}

/// JS `isTableRow(line, lineInfo)`。
pub(crate) fn is_table_row(line: &[u16], info: Option<&LineInfo>) -> bool {
    match info {
        Some(info) if !is_protected_line(info) => {}
        _ => return false,
    }
    if is_blank(line) || is_indented_code_line(line) {
        return false;
    }
    let content = strip_up_to_3_spaces(line);
    if content.first() == Some(&(b'>' as u16)) {
        return false;
    }
    if is_atx_heading(content) || is_list_line(content) || is_thematic_break(content) {
        return false;
    }
    if jsstr::starts_with(content, "```") || jsstr::starts_with(content, "~~~") {
        return false;
    }
    content.contains(&PIPE)
}

/// `findTables` 找到的一個表格（行索引從 0 起算，`end` 含）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Table {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) columns: usize,
}

/// JS `findTables(lines, info)`。
pub(crate) fn find_tables(lines: &[JsString], info: &[LineInfo]) -> Vec<Table> {
    let mut tables = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        let protected = info.get(i).is_none_or(is_protected_line)
            || info.get(i + 1).is_none_or(is_protected_line);
        if protected
            || !is_table_header_candidate(&lines[i])
            || !is_table_delimiter_row(&lines[i + 1])
        {
            i += 1;
            continue;
        }
        let columns = split_table_cells(&lines[i + 1]).len();
        let mut end = i + 1;
        while end + 1 < lines.len() && is_table_row(&lines[end + 1], info.get(end + 1)) {
            end += 1;
        }
        tables.push(Table {
            start: i,
            end,
            columns,
        });
        i = end + 1;
    }
    tables
}

/// 分隔列 cell 的樣式（`fixTableDelimiter` 與 `computeTableStructure` 共用）。
pub(crate) fn short_delimiter_cell_re() -> &'static crate::jsregex::Regex {
    re!(r"^([ \t]*)(:?)(-{1,2})(:?)([ \t]*)$")
}

/// `fixTableDelimiter` 的結果：修正後的行與第一個修改的欄號（UTF-16，從 1 起算）。
pub(crate) struct DelimiterFix {
    pub(crate) line: JsString,
    /// JS 也會算出這個欄號，但 `applyTableDelimiterFixes` 不使用（MDX060 的回報來自
    /// `computeTableStructure`）。保留以便對照 oracle，只在測試中讀取。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) column: usize,
}

/// JS `fixTableDelimiter(line, isEnabled)`。
pub(crate) fn fix_table_delimiter(line: &[u16], enabled: &Enabled) -> Option<DelimiterFix> {
    if !enabled.is("MDX060") || !line.contains(&PIPE) {
        return None;
    }
    let target = parse_quote_prefix(line).map_or(line, |q| q.rest);
    if !is_table_delimiter_row(target) {
        return None;
    }
    let mut parts = jsstr::split_by(line, PIPE);
    let mut changed = false;
    let mut first_column = 1;
    let mut offset = 0;
    for part in parts.iter_mut() {
        // `offset` 是 `parts.slice(0, i).join('|').length + (i > 0 ? 1 : 0)`；只在第一次修改前讀取，前面的 part 都還沒改，所以等於原始長度。
        if let Some(caps) = short_delimiter_cell_re().exec(part) {
            if !changed {
                first_column = offset + caps.str(part, 1).len() + 1;
            }
            let hyphens = caps.str(part, 3);
            let mut fixed = caps.str(part, 1).to_vec();
            fixed.extend_from_slice(caps.str(part, 2));
            fixed.extend(std::iter::repeat_n(
                b'-' as u16,
                3usize.saturating_sub(hyphens.len()),
            ));
            fixed.extend_from_slice(hyphens);
            fixed.extend_from_slice(caps.str(part, 4));
            fixed.extend_from_slice(caps.str(part, 5));
            *part = fixed;
            changed = true;
        }
        offset += part.len() + 1;
    }
    if !changed {
        return None;
    }
    Some(DelimiterFix {
        line: jsstr::join_by(&parts, PIPE),
        column: first_column,
    })
}

/// JS `normalizeLabel(label)`：`trim`、連續 `\s` 壓成一個空格、`toLowerCase`。
pub(crate) fn normalize_label(label: &[u16]) -> JsString {
    let collapsed = re!(r"\s+").replace(trim(label), true, &[b' ' as u16]);
    jsstr::to_lower_case(&collapsed)
}

/// JS `applyTableDelimiterFixes(text, isEnabled)`。
pub(crate) fn apply_table_delimiter_fixes(text: &[u16], enabled: &Enabled) -> JsString {
    if !enabled.is("MDX060") {
        return text.to_vec();
    }
    let scan = scan_document(text);
    let mut lines = scan.lines.clone();
    for table in find_tables(&scan.lines, &scan.info) {
        let index = table.start + 1;
        if let Some(fix) = lines
            .get(index)
            .and_then(|l| fix_table_delimiter(l, enabled))
        {
            lines[index] = fix.line;
        }
    }
    jsstr::join_lines(&lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsstr::{to_string, u};

    fn cells(s: &str) -> Vec<String> {
        split_table_cells(&u(s))
            .iter()
            .map(|c| to_string(c))
            .collect()
    }

    #[test]
    fn split_cells() {
        assert_eq!(cells("| a | b |"), vec!["a", "b"]);
        assert_eq!(cells("| `a|b` | c \\| d |"), vec!["`a|b`", "c \\| d"]);
        assert_eq!(cells("a | b \\|"), vec!["a", "b \\|"]);
    }

    #[test]
    fn delimiter_fix() {
        let e = Enabled::new(&crate::Options::default());
        let fix = fix_table_delimiter(&u("| - | :--: |"), &e).expect("fix");
        assert_eq!(to_string(&fix.line), "| --- | :---: |");
        assert_eq!(fix.column, 3);
    }
}
