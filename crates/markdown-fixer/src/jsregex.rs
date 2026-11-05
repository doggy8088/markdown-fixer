//! 迷你 JS 正規表示式引擎（非 `u` 模式，以 UTF-16 code unit 比對）。
//!
//! 為什麼不用 `regex` crate：oracle 大量使用 lookbehind／lookahead、惰性量詞
//! 與回溯語意，`\s`、`\w`、`\b`、`i` 旗標也都是 JS 的定義（`\w`、`\b` 只有
//! ASCII，`\s` 含 U+FEFF），而且比對單位是 UTF-16 code unit。`regex` crate
//! 不支援 lookaround、以 UTF-8 運作且預設 Unicode 語意，逐條手寫又容易漏掉
//! 回溯細節。所以這裡實作一個只涵蓋 oracle 用到的語法子集的回溯式 VM，讓 JS
//! 的正規表示式原文可以直接移植，優先順序（貪婪／惰性、選擇的先後）與 JS 相同。
//!
//! 支援：字面字元、`.`、字元類別（含範圍、否定、`\s\S\w\W\d\D`）、`^`、`$`
//! （非 multiline）、`\b`、`\B`、群組（捕獲與 `(?:)`）、`|`、量詞
//! `* + ? {n} {n,} {n,m}` 與惰性版本、lookahead、固定寬度 lookbehind、
//! `i` 旗標（只做 ASCII 大小寫折疊，與 JS 非 `u` 模式下這些 ASCII 樣式的行為相同）。
//!
//! VM 用明確的回溯堆疊，不遞迴，長行不會造成 stack overflow。
//! 不支援：backreference、named group、可匹配空字串的無上限迴圈（編譯時回傳錯誤）。

use crate::jsstr::{is_digit, is_js_space, is_word};

const NONE: usize = usize::MAX;

#[derive(Debug, Clone)]
enum ClassItem {
    Range(u16, u16),
    Space(bool),
    Word(bool),
    Digit(bool),
}

#[derive(Debug, Clone)]
struct Class {
    items: Vec<ClassItem>,
    negated: bool,
}

impl Class {
    fn raw(&self, c: u16) -> bool {
        self.items.iter().any(|item| match *item {
            ClassItem::Range(lo, hi) => lo <= c && c <= hi,
            ClassItem::Space(neg) => is_js_space(c) != neg,
            ClassItem::Word(neg) => is_word(c) != neg,
            ClassItem::Digit(neg) => is_digit(c) != neg,
        })
    }

    fn matches(&self, c: u16, icase: bool) -> bool {
        let mut found = self.raw(c);
        if !found && icase {
            if let Some(other) = swap_ascii_case(c) {
                found = self.raw(other);
            }
        }
        found != self.negated
    }
}

fn swap_ascii_case(c: u16) -> Option<u16> {
    match c {
        0x41..=0x5A => Some(c + 32),
        0x61..=0x7A => Some(c - 32),
        _ => None,
    }
}

fn fold(c: u16) -> u16 {
    match c {
        0x61..=0x7A => c - 32,
        _ => c,
    }
}

#[derive(Debug, Clone)]
enum Node {
    Empty,
    Lit(u16),
    Class(Class),
    Dot,
    Start,
    End,
    WordB(bool),
    Group(Box<Node>, Option<usize>),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat(Box<Node>, u32, Option<u32>, bool),
    Look(Box<Node>, bool, bool),
}

/// 正規表示式語法錯誤（只會發生在程式內的常數樣式寫錯時）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegexError(pub(crate) &'static str);

struct Parser {
    p: Vec<u16>,
    i: usize,
    ncap: usize,
}

impl Parser {
    fn peek(&self) -> Option<u16> {
        self.p.get(self.i).copied()
    }
    fn peek_at(&self, k: usize) -> Option<u16> {
        self.p.get(self.i + k).copied()
    }
    fn eat(&mut self, c: u16) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn parse_alt(&mut self) -> Result<Node, RegexError> {
        let mut alts = vec![self.parse_concat()?];
        while self.eat(b'|' as u16) {
            alts.push(self.parse_concat()?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().unwrap_or(Node::Empty)
        } else {
            Node::Alt(alts)
        })
    }

    fn parse_concat(&mut self) -> Result<Node, RegexError> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == b'|' as u16 || c == b')' as u16 {
                break;
            }
            let atom = self.parse_atom()?;
            let atom = self.parse_quantifier(atom)?;
            items.push(atom);
        }
        Ok(match items.len() {
            0 => Node::Empty,
            1 => items.pop().unwrap_or(Node::Empty),
            _ => Node::Concat(items),
        })
    }

    fn parse_number(&mut self) -> Option<u32> {
        let start = self.i;
        let mut n: u32 = 0;
        while let Some(c) = self.peek() {
            if !is_digit(c) {
                break;
            }
            n = n.saturating_mul(10).saturating_add(u32::from(c - 0x30));
            self.i += 1;
        }
        if self.i == start { None } else { Some(n) }
    }

    fn parse_quantifier(&mut self, atom: Node) -> Result<Node, RegexError> {
        let (min, max) = match self.peek() {
            Some(0x2A) => {
                self.i += 1;
                (0, None)
            }
            Some(0x2B) => {
                self.i += 1;
                (1, None)
            }
            Some(0x3F) => {
                self.i += 1;
                (0, Some(1))
            }
            Some(0x7B) => {
                let save = self.i;
                self.i += 1;
                let parsed = (|| {
                    let min = self.parse_number()?;
                    let max = if self.eat(b',' as u16) {
                        if self.peek() == Some(b'}' as u16) {
                            None
                        } else {
                            Some(self.parse_number()?)
                        }
                    } else {
                        Some(min)
                    };
                    if self.eat(b'}' as u16) {
                        Some((min, max))
                    } else {
                        None
                    }
                })();
                match parsed {
                    Some(q) => q,
                    None => {
                        // Annex B：不是合法量詞的 `{` 視為字面字元。
                        self.i = save;
                        return Ok(atom);
                    }
                }
            }
            _ => return Ok(atom),
        };
        let greedy = !self.eat(b'?' as u16);
        if matches!(atom, Node::Start | Node::End | Node::WordB(_)) {
            return Err(RegexError("nothing to repeat"));
        }
        Ok(Node::Repeat(Box::new(atom), min, max, greedy))
    }

    fn parse_atom(&mut self) -> Result<Node, RegexError> {
        let c = self.peek().ok_or(RegexError("unexpected end"))?;
        self.i += 1;
        match c {
            0x28 => {
                // (
                let node = if self.peek() == Some(b'?' as u16) {
                    match (self.peek_at(1), self.peek_at(2)) {
                        (Some(0x3A), _) => {
                            self.i += 2;
                            let inner = self.parse_alt()?;
                            Node::Group(Box::new(inner), None)
                        }
                        (Some(0x3D), _) | (Some(0x21), _) => {
                            let negate = self.peek_at(1) == Some(0x21);
                            self.i += 2;
                            let inner = self.parse_alt()?;
                            Node::Look(Box::new(inner), false, negate)
                        }
                        (Some(0x3C), Some(0x3D)) | (Some(0x3C), Some(0x21)) => {
                            let negate = self.peek_at(2) == Some(0x21);
                            self.i += 3;
                            let inner = self.parse_alt()?;
                            Node::Look(Box::new(inner), true, negate)
                        }
                        _ => return Err(RegexError("unsupported group")),
                    }
                } else {
                    self.ncap += 1;
                    let index = self.ncap;
                    let inner = self.parse_alt()?;
                    Node::Group(Box::new(inner), Some(index))
                };
                if !self.eat(b')' as u16) {
                    return Err(RegexError("unterminated group"));
                }
                Ok(node)
            }
            0x5B => self.parse_class(),
            0x2E => Ok(Node::Dot),
            0x5E => Ok(Node::Start),
            0x24 => Ok(Node::End),
            0x5C => self.parse_escape(false).map(|e| match e {
                Esc::Unit(u) => Node::Lit(u),
                Esc::Item(item) => Node::Class(Class {
                    items: vec![item],
                    negated: false,
                }),
                Esc::WordB(neg) => Node::WordB(neg),
            }),
            0x29 => Err(RegexError("unmatched )")),
            0x2A | 0x2B | 0x3F => Err(RegexError("nothing to repeat")),
            _ => Ok(Node::Lit(c)),
        }
    }

    fn parse_hex(&mut self, digits: usize) -> Option<u16> {
        let mut v: u16 = 0;
        for k in 0..digits {
            let c = self.peek_at(k)?;
            let d = char::from_u32(u32::from(c))?.to_digit(16)?;
            v = v.wrapping_mul(16).wrapping_add(d as u16);
        }
        self.i += digits;
        Some(v)
    }

    fn parse_escape(&mut self, in_class: bool) -> Result<Esc, RegexError> {
        let c = self.peek().ok_or(RegexError("trailing backslash"))?;
        self.i += 1;
        Ok(match c {
            0x64 => Esc::Item(ClassItem::Digit(false)),
            0x44 => Esc::Item(ClassItem::Digit(true)),
            0x77 => Esc::Item(ClassItem::Word(false)),
            0x57 => Esc::Item(ClassItem::Word(true)),
            0x73 => Esc::Item(ClassItem::Space(false)),
            0x53 => Esc::Item(ClassItem::Space(true)),
            0x62 if in_class => Esc::Unit(0x08),
            0x62 => Esc::WordB(false),
            0x42 if !in_class => Esc::WordB(true),
            0x74 => Esc::Unit(0x09),
            0x6E => Esc::Unit(0x0A),
            0x72 => Esc::Unit(0x0D),
            0x76 => Esc::Unit(0x0B),
            0x66 => Esc::Unit(0x0C),
            0x30 => Esc::Unit(0x00),
            0x75 => match self.parse_hex(4) {
                Some(v) => Esc::Unit(v),
                None => Esc::Unit(c),
            },
            0x78 => match self.parse_hex(2) {
                Some(v) => Esc::Unit(v),
                None => Esc::Unit(c),
            },
            0x31..=0x39 => return Err(RegexError("backreference unsupported")),
            _ => Esc::Unit(c),
        })
    }

    fn parse_class(&mut self) -> Result<Node, RegexError> {
        let negated = self.eat(b'^' as u16);
        let mut items = Vec::new();
        loop {
            let c = self.peek().ok_or(RegexError("unterminated class"))?;
            if c == b']' as u16 {
                self.i += 1;
                break;
            }
            let lo = self.class_atom()?;
            if self.peek() == Some(b'-' as u16)
                && self.peek_at(1).is_some()
                && self.peek_at(1) != Some(b']' as u16)
            {
                self.i += 1;
                let hi = self.class_atom()?;
                match (lo, hi) {
                    (Esc::Unit(a), Esc::Unit(b)) if a <= b => items.push(ClassItem::Range(a, b)),
                    _ => return Err(RegexError("bad class range")),
                }
            } else {
                match lo {
                    Esc::Unit(a) => items.push(ClassItem::Range(a, a)),
                    Esc::Item(item) => items.push(item),
                    Esc::WordB(_) => return Err(RegexError("bad class escape")),
                }
            }
        }
        Ok(Node::Class(Class { items, negated }))
    }

    fn class_atom(&mut self) -> Result<Esc, RegexError> {
        let c = self.peek().ok_or(RegexError("unterminated class"))?;
        self.i += 1;
        if c == b'\\' as u16 {
            self.parse_escape(true)
        } else {
            Ok(Esc::Unit(c))
        }
    }
}

enum Esc {
    Unit(u16),
    Item(ClassItem),
    WordB(bool),
}

fn nullable(node: &Node) -> bool {
    match node {
        Node::Empty | Node::Start | Node::End | Node::WordB(_) | Node::Look(..) => true,
        Node::Lit(_) | Node::Class(_) | Node::Dot => false,
        Node::Group(inner, _) => nullable(inner),
        Node::Concat(items) => items.iter().all(nullable),
        Node::Alt(items) => items.iter().any(nullable),
        Node::Repeat(inner, min, _, _) => *min == 0 || nullable(inner),
    }
}

fn fixed_width(node: &Node) -> Option<usize> {
    match node {
        Node::Empty | Node::Start | Node::End | Node::WordB(_) | Node::Look(..) => Some(0),
        Node::Lit(_) | Node::Class(_) | Node::Dot => Some(1),
        Node::Group(inner, _) => fixed_width(inner),
        Node::Concat(items) => items.iter().map(fixed_width).sum(),
        Node::Alt(items) => {
            let first = fixed_width(items.first()?)?;
            items
                .iter()
                .all(|n| fixed_width(n) == Some(first))
                .then_some(first)
        }
        Node::Repeat(inner, min, max, _) => {
            if Some(*min) == *max {
                Some(fixed_width(inner)? * (*min as usize))
            } else {
                None
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Inst {
    Lit(u16),
    Class(usize),
    Dot,
    Start,
    End,
    WordB(bool),
    Split(usize, usize),
    Jmp(usize),
    Save(usize),
    Look(Box<Prog>, bool, bool, usize),
    Match,
}

#[derive(Debug, Clone)]
struct Prog {
    insts: Vec<Inst>,
    classes: Vec<Class>,
}

struct Compiler {
    insts: Vec<Inst>,
    classes: Vec<Class>,
}

impl Compiler {
    fn emit(&mut self, inst: Inst) -> usize {
        self.insts.push(inst);
        self.insts.len() - 1
    }

    fn compile(&mut self, node: &Node) -> Result<(), RegexError> {
        match node {
            Node::Empty => {}
            Node::Lit(c) => {
                self.emit(Inst::Lit(*c));
            }
            Node::Class(class) => {
                self.classes.push(class.clone());
                let idx = self.classes.len() - 1;
                self.emit(Inst::Class(idx));
            }
            Node::Dot => {
                self.emit(Inst::Dot);
            }
            Node::Start => {
                self.emit(Inst::Start);
            }
            Node::End => {
                self.emit(Inst::End);
            }
            Node::WordB(neg) => {
                self.emit(Inst::WordB(*neg));
            }
            Node::Group(inner, cap) => {
                if let Some(k) = cap {
                    self.emit(Inst::Save(k * 2));
                    self.compile(inner)?;
                    self.emit(Inst::Save(k * 2 + 1));
                } else {
                    self.compile(inner)?;
                }
            }
            Node::Concat(items) => {
                for item in items {
                    self.compile(item)?;
                }
            }
            Node::Alt(items) => {
                let mut jumps = Vec::new();
                for (k, item) in items.iter().enumerate() {
                    if k + 1 < items.len() {
                        let split = self.emit(Inst::Split(0, 0));
                        self.compile(item)?;
                        jumps.push(self.emit(Inst::Jmp(0)));
                        let next = self.insts.len();
                        self.insts[split] = Inst::Split(split + 1, next);
                    } else {
                        self.compile(item)?;
                    }
                }
                let end = self.insts.len();
                for j in jumps {
                    self.insts[j] = Inst::Jmp(end);
                }
            }
            Node::Repeat(inner, min, max, greedy) => {
                for _ in 0..*min {
                    self.compile(inner)?;
                }
                match max {
                    None => {
                        if nullable(inner) {
                            return Err(RegexError("nullable loop body unsupported"));
                        }
                        let split = self.emit(Inst::Split(0, 0));
                        self.compile(inner)?;
                        self.emit(Inst::Jmp(split));
                        let out = self.insts.len();
                        self.insts[split] = if *greedy {
                            Inst::Split(split + 1, out)
                        } else {
                            Inst::Split(out, split + 1)
                        };
                    }
                    Some(max) => {
                        let optional = max.saturating_sub(*min);
                        let mut splits = Vec::new();
                        for _ in 0..optional {
                            splits.push(self.emit(Inst::Split(0, 0)));
                            self.compile(inner)?;
                        }
                        let out = self.insts.len();
                        for s in splits {
                            self.insts[s] = if *greedy {
                                Inst::Split(s + 1, out)
                            } else {
                                Inst::Split(out, s + 1)
                            };
                        }
                    }
                }
            }
            Node::Look(inner, behind, negate) => {
                let width = if *behind {
                    fixed_width(inner).ok_or(RegexError("variable-width lookbehind"))?
                } else {
                    0
                };
                let mut sub = Compiler {
                    insts: Vec::new(),
                    classes: Vec::new(),
                };
                sub.compile(inner)?;
                sub.emit(Inst::Match);
                let prog = Prog {
                    insts: sub.insts,
                    classes: sub.classes,
                };
                self.emit(Inst::Look(Box::new(prog), *behind, *negate, width));
            }
        }
        Ok(())
    }
}

enum Bt {
    Thread(usize, usize),
    Restore(usize, usize),
}

fn is_line_terminator(c: u16) -> bool {
    matches!(c, 0x0A | 0x0D | 0x2028 | 0x2029)
}

/// 回溯 VM 的執行狀態：回溯堆疊、步數預算與（必要時）已走過狀態的記憶。
///
/// 沒有 backreference 時，一個 `(pc, pos)` 狀態能不能走到 `Match` 與怎麼走到它無關，
/// 所以曾經走過（而且當時沒有成功）的狀態可以直接視為失敗。這不改變 JS 的優先順序：
/// 第一條成功路徑不會被略過，因為它經過的狀態在它之前都不曾被完整走過。
/// 跨起點共用記憶後，整個搜尋最多 `O(指令數 × 長度)`，避免 `(?: {0,3}>[ \t]?)*`
/// 這類樣式在病態輸入上的指數級回溯（oracle 本身會卡住，PRD〈7〉要求 Rust 不行）。
struct Exec {
    stack: Vec<Bt>,
    /// 尚未啟用記憶時剩餘的步數；用完就回報 `Budget`，由呼叫端改用記憶模式重跑。
    budget: Option<usize>,
    /// `visited[pc * (len + 1) + pos]`。
    visited: Option<Vec<u64>>,
    width: usize,
}

enum Outcome {
    Match,
    NoMatch,
    Budget,
}

impl Exec {
    fn new(budget: Option<usize>) -> Exec {
        Exec {
            stack: Vec::new(),
            budget,
            visited: None,
            width: 0,
        }
    }

    fn with_memo(prog_len: usize, len: usize) -> Exec {
        let width = len + 1;
        let bits = prog_len.saturating_mul(width);
        Exec {
            stack: Vec::new(),
            budget: None,
            visited: Some(vec![0u64; bits.div_ceil(64)]),
            width,
        }
    }

    /// 標記 `(pc, pos)`；已經走過時回傳 false。
    fn first_visit(&mut self, pc: usize, pos: usize) -> bool {
        let width = self.width;
        let Some(visited) = self.visited.as_mut() else {
            return true;
        };
        let bit = pc * width + pos;
        let (word, mask) = (bit / 64, 1u64 << (bit % 64));
        match visited.get_mut(word) {
            Some(w) if *w & mask != 0 => false,
            Some(w) => {
                *w |= mask;
                true
            }
            None => true,
        }
    }
}

/// lookaround 子程式：樣式都很短，直接以獨立、不限步數的 VM 執行。
fn run_sub(prog: &Prog, s: &[u16], start: usize, icase: bool) -> bool {
    let mut exec = Exec::new(None);
    matches!(
        run(prog, s, start, &mut [], icase, &mut exec),
        Outcome::Match
    )
}

fn run(
    prog: &Prog,
    s: &[u16],
    start: usize,
    slots: &mut [usize],
    icase: bool,
    exec: &mut Exec,
) -> Outcome {
    exec.stack.clear();
    exec.stack.push(Bt::Thread(0, start));
    while let Some(bt) = exec.stack.pop() {
        let (mut pc, mut pos) = match bt {
            Bt::Restore(k, v) => {
                if let Some(slot) = slots.get_mut(k) {
                    *slot = v;
                }
                continue;
            }
            Bt::Thread(pc, pos) => (pc, pos),
        };
        while let Some(inst) = prog.insts.get(pc) {
            if let Some(budget) = exec.budget.as_mut() {
                if *budget == 0 {
                    return Outcome::Budget;
                }
                *budget -= 1;
            }
            if !exec.first_visit(pc, pos) {
                break;
            }
            match inst {
                Inst::Lit(c) => match s.get(pos) {
                    Some(&x) if x == *c || (icase && fold(x) == fold(*c)) => {
                        pos += 1;
                        pc += 1;
                    }
                    _ => break,
                },
                Inst::Class(idx) => match (s.get(pos), prog.classes.get(*idx)) {
                    (Some(&x), Some(class)) if class.matches(x, icase) => {
                        pos += 1;
                        pc += 1;
                    }
                    _ => break,
                },
                Inst::Dot => match s.get(pos) {
                    Some(&x) if !is_line_terminator(x) => {
                        pos += 1;
                        pc += 1;
                    }
                    _ => break,
                },
                Inst::Start => {
                    if pos != 0 {
                        break;
                    }
                    pc += 1;
                }
                Inst::End => {
                    if pos != s.len() {
                        break;
                    }
                    pc += 1;
                }
                Inst::WordB(neg) => {
                    let a = pos > 0 && s.get(pos - 1).is_some_and(|&c| is_word(c));
                    let b = s.get(pos).is_some_and(|&c| is_word(c));
                    if (a != b) == *neg {
                        break;
                    }
                    pc += 1;
                }
                Inst::Split(a, b) => {
                    exec.stack.push(Bt::Thread(*b, pos));
                    pc = *a;
                }
                Inst::Jmp(a) => pc = *a,
                Inst::Save(k) => {
                    if let Some(slot) = slots.get_mut(*k) {
                        exec.stack.push(Bt::Restore(*k, *slot));
                        *slot = pos;
                    }
                    pc += 1;
                }
                Inst::Look(sub, behind, negate, width) => {
                    let matched = if *behind {
                        pos >= *width && run_sub(sub, s, pos - *width, icase)
                    } else {
                        run_sub(sub, s, pos, icase)
                    };
                    if matched == *negate {
                        break;
                    }
                    pc += 1;
                }
                Inst::Match => return Outcome::Match,
            }
        }
    }
    Outcome::NoMatch
}

/// 測試用：強制從第一步就使用記憶化回溯，用來驗證記憶模式與 oracle 結果相同。
#[cfg(test)]
pub(crate) static FORCE_MEMO: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
fn force_memo() -> bool {
    FORCE_MEMO.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(test))]
fn force_memo() -> bool {
    false
}

/// 編譯後的 JS 正規表示式。
#[derive(Debug, Clone)]
pub(crate) struct Regex {
    prog: Prog,
    nslots: usize,
    icase: bool,
    anchored: bool,
    first: Option<FirstSet>,
}

/// 比對成功時第一個被消耗的 code unit 必須屬於這個集合（只是必要條件，
/// 用來略過不可能的起點，不影響結果）。
#[derive(Debug, Clone)]
struct FirstSet {
    lits: Vec<u16>,
    classes: Vec<usize>,
}

impl FirstSet {
    /// 從 `pc = 0` 沿著零寬度指令展開，收集第一個消耗字元的指令。
    /// 可能匹配空字串或遇到 `.` 時回傳 `None`（不做預先過濾）。
    fn compute(prog: &Prog, icase: bool) -> Option<FirstSet> {
        let mut set = FirstSet {
            lits: Vec::new(),
            classes: Vec::new(),
        };
        let mut seen = vec![false; prog.insts.len()];
        let mut todo = vec![0usize];
        while let Some(pc) = todo.pop() {
            if std::mem::replace(seen.get_mut(pc)?, true) {
                continue;
            }
            match prog.insts.get(pc)? {
                Inst::Lit(c) => {
                    set.lits.push(*c);
                    if icase {
                        if let Some(o) = swap_ascii_case(*c) {
                            set.lits.push(o);
                        }
                    }
                }
                Inst::Class(idx) => set.classes.push(*idx),
                Inst::Dot | Inst::Match => return None,
                Inst::Start | Inst::End | Inst::WordB(_) | Inst::Save(_) | Inst::Look(..) => {
                    todo.push(pc + 1)
                }
                Inst::Jmp(a) => todo.push(*a),
                Inst::Split(a, b) => {
                    todo.push(*a);
                    todo.push(*b);
                }
            }
        }
        Some(set)
    }

    fn accepts(&self, prog: &Prog, c: u16, icase: bool) -> bool {
        self.lits.contains(&c)
            || self
                .classes
                .iter()
                .any(|&k| prog.classes.get(k).is_some_and(|cl| cl.matches(c, icase)))
    }
}

/// 一次比對的結果（group 0 是整個比對）。索引是 UTF-16 code unit。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Captures {
    slots: Vec<usize>,
}

impl Captures {
    /// 第 `i` 組的範圍；未參與比對時為 `None`（JS 的 `undefined`）。
    pub(crate) fn get(&self, i: usize) -> Option<(usize, usize)> {
        let a = *self.slots.get(i * 2)?;
        let b = *self.slots.get(i * 2 + 1)?;
        if a == NONE || b == NONE || a > b {
            None
        } else {
            Some((a, b))
        }
    }

    /// 整個比對的起點（`match.index`）。
    pub(crate) fn start(&self) -> usize {
        self.get(0).map_or(0, |r| r.0)
    }

    /// 整個比對的終點。
    pub(crate) fn end(&self) -> usize {
        self.get(0).map_or(0, |r| r.1)
    }

    /// 第 `i` 組的文字；未參與比對時回傳空字串。
    pub(crate) fn str<'a>(&self, s: &'a [u16], i: usize) -> &'a [u16] {
        self.opt(s, i).unwrap_or(&[])
    }

    /// 第 `i` 組的文字；未參與比對時回傳 `None`。
    pub(crate) fn opt<'a>(&self, s: &'a [u16], i: usize) -> Option<&'a [u16]> {
        let (a, b) = self.get(i)?;
        s.get(a..b)
    }
}

impl Regex {
    /// 以 JS 語法編譯樣式。`flags` 只支援 `i`（`g` 由呼叫端以 `find_iter` 表達）。
    pub(crate) fn new(pattern: &str, flags: &str) -> Result<Regex, RegexError> {
        let mut parser = Parser {
            p: pattern.encode_utf16().collect(),
            i: 0,
            ncap: 0,
        };
        let node = parser.parse_alt()?;
        if parser.i != parser.p.len() {
            return Err(RegexError("unmatched )"));
        }
        let anchored = match &node {
            Node::Start => true,
            Node::Concat(items) => matches!(items.first(), Some(Node::Start)),
            _ => false,
        };
        let mut c = Compiler {
            insts: Vec::new(),
            classes: Vec::new(),
        };
        c.emit(Inst::Save(0));
        c.compile(&node)?;
        c.emit(Inst::Save(1));
        c.emit(Inst::Match);
        let icase = flags.contains('i');
        let prog = Prog {
            insts: c.insts,
            classes: c.classes,
        };
        let first = FirstSet::compute(&prog, icase);
        Ok(Regex {
            prog,
            nslots: (parser.ncap + 1) * 2,
            icase,
            anchored,
            first,
        })
    }

    /// 永遠不會比對成功的樣式（常數樣式編譯失敗時的保底，不 panic）。
    fn never() -> Regex {
        Regex {
            prog: Prog {
                insts: Vec::new(),
                classes: Vec::new(),
            },
            nslots: 2,
            icase: false,
            anchored: false,
            first: None,
        }
    }

    /// 編譯常數樣式；失敗時在 debug build 觸發斷言，release 則退回永不匹配。
    pub(crate) fn constant(pattern: &str, flags: &str) -> Regex {
        match Regex::new(pattern, flags) {
            Ok(re) => re,
            Err(err) => {
                debug_assert!(false, "invalid constant regex {pattern:?}: {err:?}");
                Regex::never()
            }
        }
    }

    /// 從 `from`（`lastIndex`）開始搜尋第一個比對。
    pub(crate) fn exec_from(&self, s: &[u16], from: usize) -> Option<Captures> {
        let mut slots = vec![NONE; self.nslots];
        let last = if self.anchored { 0 } else { s.len() };
        if from > s.len() || (self.anchored && from > 0) {
            return None;
        }
        // 先以步數預算跑一般回溯；超過預算（病態輸入）才改用記憶模式從頭重跑，
        // 結果相同但保證多項式時間。
        let budget = if force_memo() {
            0
        } else {
            4096 + 64 * (s.len() + 1)
        };
        let mut exec = Exec::new(Some(budget));
        let mut start = from;
        while start <= last {
            if let Some(first) = &self.first {
                match s.get(start) {
                    Some(&c) if first.accepts(&self.prog, c, self.icase) => {}
                    _ => {
                        start += 1;
                        continue;
                    }
                }
            }
            slots.iter_mut().for_each(|x| *x = NONE);
            match run(&self.prog, s, start, &mut slots, self.icase, &mut exec) {
                Outcome::Match => return Some(Captures { slots }),
                Outcome::NoMatch => start += 1,
                Outcome::Budget => {
                    let mut memo = Exec::with_memo(self.prog.insts.len(), s.len());
                    let mut start = start;
                    while start <= last {
                        slots.iter_mut().for_each(|x| *x = NONE);
                        if let Outcome::Match =
                            run(&self.prog, s, start, &mut slots, self.icase, &mut memo)
                        {
                            return Some(Captures { slots });
                        }
                        start += 1;
                    }
                    return None;
                }
            }
        }
        None
    }

    /// `s.match(re)`（非全域）／`re.exec(s)`。
    pub(crate) fn exec(&self, s: &[u16]) -> Option<Captures> {
        self.exec_from(s, 0)
    }

    /// `re.test(s)`（非全域）。
    pub(crate) fn test(&self, s: &[u16]) -> bool {
        self.exec(s).is_some()
    }

    /// 全域比對（`matchAll`、`exec` 迴圈），空比對時 `lastIndex++`。
    pub(crate) fn find_iter(&self, s: &[u16]) -> Vec<Captures> {
        let mut out = Vec::new();
        let mut from = 0;
        while from <= s.len() {
            let Some(caps) = self.exec_from(s, from) else {
                break;
            };
            let end = caps.end();
            from = if end == caps.start() { end + 1 } else { end };
            out.push(caps);
        }
        out
    }

    /// `s.replace(re, fn)`：`global` 為 true 時取代全部（`/g`）。
    pub(crate) fn replace_with(
        &self,
        s: &[u16],
        global: bool,
        mut f: impl FnMut(&Captures) -> Vec<u16>,
    ) -> Vec<u16> {
        let matches = if global {
            self.find_iter(s)
        } else {
            self.exec(s).into_iter().collect()
        };
        let mut out = Vec::with_capacity(s.len());
        let mut last = 0;
        for caps in &matches {
            out.extend_from_slice(s.get(last..caps.start()).unwrap_or(&[]));
            out.extend(f(caps));
            last = caps.end();
        }
        out.extend_from_slice(s.get(last..).unwrap_or(&[]));
        out
    }

    /// `s.replace(re, literal)`；`literal` 不含 `$` 替換樣式。
    pub(crate) fn replace(&self, s: &[u16], global: bool, literal: &[u16]) -> Vec<u16> {
        self.replace_with(s, global, |_| literal.to_vec())
    }
}

/// 宣告一個延遲編譯的常數正規表示式：`re!(r"^[ \t]*$")`、`re!(r"www\.", "i")`。
macro_rules! re {
    ($pat:expr) => {
        $crate::jsregex::re!($pat, "")
    };
    ($pat:expr, $flags:expr) => {{
        static RE: ::std::sync::OnceLock<$crate::jsregex::Regex> = ::std::sync::OnceLock::new();
        RE.get_or_init(|| $crate::jsregex::Regex::constant($pat, $flags))
    }};
}
pub(crate) use re;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsstr::{to_string, u};

    fn m(pat: &str, flags: &str, s: &str) -> Option<Vec<Option<String>>> {
        let re = Regex::new(pat, flags).expect("compile");
        let s16 = u(s);
        re.exec(&s16).map(|c| {
            (0..re.nslots / 2)
                .map(|i| c.opt(&s16, i).map(to_string))
                .collect()
        })
    }

    #[test]
    fn basics() {
        assert!(Regex::new(r"^[ \t]*$", "").unwrap().test(&u("  \t")));
        assert!(!Regex::new(r"^[ \t]*$", "").unwrap().test(&u(" x")));
        assert_eq!(
            m(r"^( {0,3})(#{1,6})[ \t]+(.*?)[ \t]*$", "", "## Title  "),
            Some(vec![
                Some("## Title  ".into()),
                Some("".into()),
                Some("##".into()),
                Some("Title".into())
            ])
        );
    }

    #[test]
    fn lazy_and_backtracking() {
        assert_eq!(
            m(r"^(.*?)[ \t]+(#+)[ \t]*$", "", "a # b ##"),
            Some(vec![
                Some("a # b ##".into()),
                Some("a # b".into()),
                Some("##".into())
            ])
        );
        assert_eq!(
            m(
                r"(?<![\w*])\*\*([ \t]+)([^*\n]*?\S)([ \t]*)\*\*(?!\*)",
                "",
                "x ** a ** y"
            ),
            Some(vec![
                Some("** a **".into()),
                Some(" ".into()),
                Some("a".into()),
                Some(" ".into())
            ])
        );
    }

    #[test]
    fn lookbehind_and_case() {
        assert!(Regex::new(r"(?<!\\)\*", "").unwrap().test(&u("a*")));
        assert!(!Regex::new(r"(?<!\\)\*", "").unwrap().test(&u(r"a\*")));
        assert!(
            Regex::new(r"^ {0,3}<(script|pre)(?=[\s>])", "i")
                .unwrap()
                .test(&u("<SCRIPT>"))
        );
        assert!(Regex::new(r"\bwww\.", "i").unwrap().test(&u("WWW.x")));
        assert!(!Regex::new(r"\bwww\.", "i").unwrap().test(&u("awww.x")));
    }

    #[test]
    fn alternation_and_optional() {
        assert_eq!(m(r"\r\n?", "", "a\r\nb"), Some(vec![Some("\r\n".into())]));
        assert!(
            Regex::new(
                r"^ {0,3}(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$",
                ""
            )
            .unwrap()
            .test(&u(" - - -"))
        );
        assert!(
            !Regex::new(r"^ {0,3}(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,})$", "")
                .unwrap()
                .test(&u("--"))
        );
    }

    #[test]
    fn unicode_escapes_and_surrogates() {
        assert!(
            Regex::new(r"[\u200B\u2060\uFEFF]", "")
                .unwrap()
                .test(&u("a\u{FEFF}"))
        );
        // `.` 比對單一 code unit：emoji 算兩個。
        assert_eq!(m(r"^.$", "", "😀"), None, "non-u regex sees two code units");
        assert!(Regex::new(r"^..$", "").unwrap().test(&u("😀")));
    }

    #[test]
    fn global_iteration() {
        let re = Regex::new(r"(?<!\*)\*\*(?!\*)", "").unwrap();
        assert_eq!(re.find_iter(&u("**a** ***b***")).len(), 2);
        let re = Regex::new(r"\n*$", "").unwrap();
        assert_eq!(to_string(&re.replace(&u("a\n\n"), false, &u("\n"))), "a\n");
    }

    #[test]
    fn long_input_does_not_overflow() {
        let s = u(&"a".repeat(200_000));
        assert!(Regex::new(r"^(.*?)b", "").unwrap().exec(&s).is_none());
        let s = u(&"* ".repeat(100_000));
        assert!(
            Regex::new(r"^ {0,3}(?:(?:\*[ \t]*){3,})$", "")
                .unwrap()
                .test(&s)
        );
    }

    #[test]
    fn rejects_unsupported() {
        assert!(Regex::new(r"(a*)*", "").is_err());
        assert!(Regex::new(r"(?<!a+)b", "").is_err());
    }
}
