use crate::formatter::lexer::{Token, line_comments, token_text, tokenize};
use crate::formatter::structure::{LineComments, TokenSpan};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{
    ContainsAnyByte, find_byte, line_brace_imbalance, line_paren_imbalance, preprocessor_directive,
    trailing_comment_split_limit,
};
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{is_identifier_continue, is_identifier_start};
use std::borrow::Cow;
use std::cell::{Cell, OnceCell, RefCell};
use std::ops::Deref;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenBraceShape {
    Isolated,
    Label,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LineBraceMeta {
    pub(crate) code_starts_with_hash: bool,
    closes: u32,
    opens: u32,
    pub(crate) open_shape: OpenBraceShape,
    trim_start_byte: u32,
    trim_end_byte: u32,
    code_end_byte: u32,
    paren_closes: u32,
    paren_open_count: u32,
    paren_last_open_column: Option<u32>,
    /// End of the code before a trailing comment, as
    /// `trailing_comment_split_limit` finds it.
    comment_split_limit: u32,
    /// The same end with the blanks before it dropped.
    comment_code_end: u32,
    /// Unmatched `{` that `line_brace_imbalance` finds in `code`.
    code_opens: u32,
    /// Which of `{`, `}`, `:`, `(`, and `)` the line's code holds, one bit
    /// each.
    code_marks: u8,
    /// Whether the trimmed line is `else` or ends with `} else`.
    else_line: bool,
    /// The same for the line's code without comments.
    code_else_line: bool,
}

impl LineBraceMeta {
    pub(crate) fn closes(&self) -> usize {
        self.closes as usize
    }

    pub(crate) fn opens(&self) -> usize {
        self.opens as usize
    }

    pub(crate) fn paren_closes(&self) -> usize {
        self.paren_closes as usize
    }

    pub(crate) fn paren_open_count(&self) -> usize {
        self.paren_open_count as usize
    }

    pub(crate) fn paren_last_open_column(&self) -> Option<usize> {
        self.paren_last_open_column.map(|column| column as usize)
    }
}

/// A line's token span in half the room; `u32::MAX` marks a line with
/// none.
#[derive(Clone, Copy)]
struct PackedSpan {
    first: u32,
    last: u32,
}

impl PackedSpan {
    const NONE: Self = Self {
        first: u32::MAX,
        last: u32::MAX,
    };

    fn pack(span: Option<TokenSpan>) -> Self {
        span.map_or(Self::NONE, |span| Self {
            first: narrow_index(span.first),
            last: narrow_index(span.last),
        })
    }

    fn get(self) -> Option<TokenSpan> {
        (self.first != u32::MAX).then_some(TokenSpan {
            first: self.first as usize,
            last: self.last as usize,
        })
    }
}

/// A line's block comments in half the room; `u32::MAX` marks none.
#[derive(Clone, Copy)]
struct PackedComments {
    lead: u32,
    last: u32,
}

impl PackedComments {
    fn pack(comments: LineComments) -> Self {
        let pack = |token: Option<usize>| token.map_or(u32::MAX, narrow_index);
        Self {
            lead: pack(comments.lead),
            last: pack(comments.last),
        }
    }

    fn get(self) -> LineComments {
        let get = |token: u32| (token != u32::MAX).then_some(token as usize);
        LineComments {
            lead: get(self.lead),
            last: get(self.last),
        }
    }
}

/// A token index, which the source's size bounds below `u32::MAX`.
fn narrow_index(index: usize) -> u32 {
    u32::try_from(index)
        .ok()
        .filter(|&index| index != u32::MAX)
        .expect("a token index fits in u32")
}

/// A length or column of one line, which the source's size bounds.
fn narrow(value: usize) -> u32 {
    u32::try_from(value).expect("a line is shorter than 4 GiB")
}

fn is_else_line(trimmed: &str) -> bool {
    trimmed == "else" || trimmed.ends_with("} else")
}

fn is_raw_literal(token: &Token) -> bool {
    matches!(token, Token::StringLiteral(literal) if ["u8R\"", "LR\"", "uR\"", "UR\"", "R\""]
        .into_iter()
        .any(|prefix| literal.starts_with(prefix)))
}

fn structural_line(line: &str) -> Cow<'_, str> {
    let bytes = line.as_bytes();
    if find_byte(bytes, b'/').is_none()
        && (find_byte(bytes, b'"').is_none() || !line.contains("R\""))
    {
        return Cow::Borrowed(line);
    }
    match line_comments(line) {
        Some(comments) if !comments.held => return Cow::Borrowed(line),
        // Comments that only end the line leave its code and blanks.
        Some(comments) if !comments.inner => {
            if let Some(start) = comments.trailing_start {
                let mut structural = String::with_capacity(line.len());
                structural.push_str(&line[..start]);
                structural.extend(std::iter::repeat_n(' ', line.len() - start));
                return Cow::Owned(structural);
            }
        }
        _ => {}
    }
    let tokens = tokenize(line);
    if !tokens
        .iter()
        .any(|token| is_raw_literal(token) || matches!(token, Token::Comment(_, _)))
    {
        return Cow::Borrowed(line);
    }

    let mut structural = String::with_capacity(line.len());
    for token in &tokens {
        let text = token_text(token);
        if is_raw_literal(token) || matches!(token, Token::Comment(_, _)) {
            structural.extend(std::iter::repeat_n(' ', text.len()));
        } else {
            structural.push_str(&text);
        }
    }
    Cow::Owned(structural)
}

/// The metadata of `line`; `parens` holds the line's paren imbalance, read
/// once for both when its code is all of it.
fn compute_line_brace_meta(
    line: &str,
    parens: Option<&OnceCell<(usize, Vec<usize>)>>,
) -> LineBraceMeta {
    let line_start = line.trimmed_start();
    let structural = structural_line(line);
    let code = structural.trimmed_end();
    let trimmed = code.trimmed_start();
    let (closes, opens) = line_brace_imbalance(code);
    let scanned;
    let (paren_closes, paren_opens) = match (&structural, parens) {
        (Cow::Borrowed(_), Some(parens)) => {
            let (closes, opens) = parens.get_or_init(|| line_paren_imbalance(code));
            (*closes, opens.as_slice())
        }
        _ => {
            scanned = line_paren_imbalance(code);
            (scanned.0, scanned.1.as_slice())
        }
    };
    let open_shape = if trimmed == "{" {
        OpenBraceShape::Isolated
    } else if code.ends_with('{')
        && trimmed.split_once(':').is_some_and(|(label, _)| {
            let mut chars = label.chars();
            chars.next().is_some_and(is_identifier_start) && chars.all(is_identifier_continue)
        })
    {
        OpenBraceShape::Label
    } else {
        OpenBraceShape::Other
    };
    let (comment_split_limit, comment_code_end) = comment_split(line);
    LineBraceMeta {
        code_starts_with_hash: trimmed.starts_with('#'),
        closes: narrow(closes),
        opens: narrow(opens),
        open_shape,
        trim_start_byte: narrow(line.len() - line_start.len()),
        trim_end_byte: narrow(line.trimmed_end().len()),
        code_end_byte: narrow(code.len()),
        code_marks: code_marks(&line.as_bytes()[..code.len()]),
        code_opens: match structural {
            Cow::Borrowed(_) => narrow(opens),
            Cow::Owned(_) => narrow(line_brace_imbalance(&line[..code.len()]).1),
        },
        paren_closes: narrow(paren_closes),
        paren_open_count: narrow(paren_opens.len()),
        paren_last_open_column: (paren_opens.last().copied()).map(narrow),
        comment_split_limit,
        comment_code_end,
        else_line: is_else_line(line.trimmed()),
        code_else_line: is_else_line(
            &line[(line.len() - line_start.len()).min(code.len())..code.len()],
        ),
    }
}

/// The bit `code_marks` gives `byte`, one of `{`, `}`, `:`, `(`, and `)`.
fn code_mark(byte: u8) -> u8 {
    match byte {
        b'{' => 1,
        b'}' => 1 << 1,
        b':' => 1 << 2,
        b'(' => 1 << 3,
        b')' => 1 << 4,
        _ => unreachable!("only braces, colons, and parens are marked"),
    }
}

/// Which of `{`, `}`, `:`, `(`, and `)` `text` holds.
fn code_marks(text: &[u8]) -> u8 {
    const MARKS: [u8; 256] = {
        let mut marks = [0; 256];
        marks[b'{' as usize] = 1;
        marks[b'}' as usize] = 1 << 1;
        marks[b':' as usize] = 1 << 2;
        marks[b'(' as usize] = 1 << 3;
        marks[b')' as usize] = 1 << 4;
        marks
    };
    text.iter()
        .fold(0, |marks, &byte| marks | MARKS[usize::from(byte)])
}

/// Where `line`'s code before a trailing comment ends, before and after
/// the blanks ahead of the comment.
fn comment_split(line: &str) -> (u32, u32) {
    let limit = trailing_comment_split_limit(line);
    (narrow(limit), narrow(line[..limit].trimmed_end().len()))
}

fn compute_raw_literal_line_meta(line: &str, structural_start: usize) -> LineBraceMeta {
    let line_start = line.trimmed_start();
    let suffix = line.get(structural_start..).unwrap_or("");
    let structural = structural_line(suffix);
    let code = structural.trimmed_end();
    let (closes, opens) = line_brace_imbalance(code);
    let (paren_closes, paren_opens) = line_paren_imbalance(code);
    let (comment_split_limit, comment_code_end) = comment_split(line);
    LineBraceMeta {
        code_starts_with_hash: code.trimmed_start().starts_with('#'),
        closes: narrow(closes),
        opens: narrow(opens),
        open_shape: OpenBraceShape::Other,
        trim_start_byte: narrow(line.len() - line_start.len()),
        trim_end_byte: narrow(line.trimmed_end().len()),
        code_end_byte: narrow(line.len()),
        code_marks: code_marks(line.as_bytes()),
        code_opens: narrow(line_brace_imbalance(line).1),
        paren_closes: narrow(paren_closes),
        paren_open_count: narrow(paren_opens.len()),
        paren_last_open_column: (None).map(narrow),
        comment_split_limit,
        comment_code_end,
        else_line: is_else_line(line.trimmed()),
        code_else_line: is_else_line(line_start),
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct OutputLineHints {
    has_colon: bool,
    has_else: bool,
    has_hash: bool,
    has_slash: bool,
    has_question: bool,
    has_at: bool,
    has_new: bool,
    has_asm: bool,
    starts_star: bool,
}

fn output_line_hints(
    line: &str,
    find_else: bool,
    find_new: bool,
    find_asm: bool,
) -> OutputLineHints {
    const COLON: u8 = 1;
    const HASH: u8 = 1 << 1;
    const SLASH: u8 = 1 << 2;
    const QUESTION: u8 = 1 << 3;
    const AT: u8 = 1 << 4;
    const N: u8 = 1 << 5;
    const E: u8 = 1 << 6;
    const A: u8 = 1 << 7;
    const CLASSES: [u8; 256] = {
        let mut classes = [0; 256];
        classes[b':' as usize] = COLON;
        classes[b'#' as usize] = HASH;
        classes[b'/' as usize] = SLASH;
        classes[b'?' as usize] = QUESTION;
        classes[b'@' as usize] = AT;
        classes[b'n' as usize] = N;
        classes[b'e' as usize] = E;
        classes[b'a' as usize] = A;
        classes
    };
    let bytes = line.as_bytes();
    let found = bytes
        .iter()
        .fold(0, |found, &byte| found | CLASSES[usize::from(byte)]);
    OutputLineHints {
        has_colon: found & COLON != 0,
        has_else: find_else && found & E != 0 && line.contains("else"),
        has_hash: found & HASH != 0,
        has_slash: found & SLASH != 0,
        has_question: found & QUESTION != 0,
        has_at: found & AT != 0,
        has_new: find_new && found & N != 0 && line.contains("new "),
        has_asm: find_asm && found & A != 0 && line.contains("asm"),
        starts_star: bytes.iter().find(|&&byte| byte != b' ' && byte != b'\t') == Some(&b'*'),
    }
}

// Reads go through `Deref`; mutations stay on this type so cached line metadata cannot go stale.
#[derive(Default)]
pub(crate) struct OutputBuffer {
    lines: Vec<String>,
    meta: Vec<OnceCell<LineBraceMeta>>,
    /// Unmatched `)`/`]` and the columns of unmatched `(`/`[` of each
    /// line, read once a layout rule asks.
    parens: Vec<OnceCell<(usize, Vec<usize>)>>,
    /// Source tokens of each line, when the line came from one current line.
    tokens: Vec<PackedSpan>,
    /// Tokens of the current line just taken, for the next pushed line.
    pending_tokens: Option<TokenSpan>,
    /// Block comments of each line.
    comments: Vec<PackedComments>,
    /// Lines kept as the source wrote them: disabled regions, raw lines,
    /// and multi-line literal rows. Their whitespace is content.
    verbatim: Vec<bool>,
    /// Continued lines of a directive that the formatter indented itself.
    indented_directive_continuation: Vec<bool>,
    /// Comments of the current line just taken, for the next pushed line.
    pending_comments: Option<LineComments>,
    /// Block comment token being pushed; lines pushed without a current line
    /// meanwhile hold its text.
    active_comment: Option<usize>,
    /// First line of the current top-level construct: the line closing the
    /// previous function body.
    scope_start: usize,
    /// Line that starts the next construct once a later line is pushed, so
    /// the closing line's own layout still sees its function.
    pending_scope_start: Option<usize>,
    /// The lines whose `{`s the lines after them leave open, read on as
    /// lines come.
    open_brace_lines: RefCell<OpenBraceLines>,
    /// The same, reading every line as code.
    plain_open_brace_lines: RefCell<OpenBraceLines>,
    /// The first line changed in place since each [`OpenBraceLines`] read
    /// on.
    lowest_change: [Cell<usize>; 2 + LOOK_BACKS],
    /// The last [`Self::comment_run`], with the scope start, version, and
    /// line count it read.
    comment_run_cache: Cell<Option<(usize, u64, usize, CommentRun)>>,
    /// The last [`Self::last_line_with_tokens`], with the version and line
    /// count it read.
    token_line_cache: Cell<Option<(u64, usize, Option<usize>)>>,
    /// The last [`Self::last_lines_where`] of each kind, with the scope
    /// start, version, and line count it read.
    last_lines_cache: [Cell<Option<LastLinesRead>>; 2],
    may_have_label_open: bool,
    may_have_else: bool,
    may_have_hash: bool,
    may_have_comment: bool,
    may_have_question: bool,
    may_have_at: bool,
    may_have_new: bool,
    may_have_asm: bool,
    /// The last look back for a line that holds `new `.
    mentions_new_cache: Cell<Option<RecentMatch>>,
    /// The last answer of [`Self::designator_since_closed_row`]: the scope
    /// start, version and line count it read, and the answer.
    designator_cache: Cell<Option<(usize, u64, usize, bool)>>,
    last_non_empty_index: Cell<Option<usize>>,
    last_non_empty_dirty: Cell<bool>,
    /// Counts changes to lines already pushed and to the scope.
    version: u64,
    /// The version each of the last changes to lines made and the lowest
    /// line it changed, at the version modulo their count; a version not
    /// recorded here, as a change of scope, clears all a look back read.
    recent_changes: [(u64, usize); RECENT_CHANGES],
    /// The lines the walk for the open brace a closing brace would close
    /// finds open.
    closing_brace_blocks: RefCell<OpenBlockLines>,
    /// The lines the walk for the switch the output stands in finds open.
    switch_blocks: RefCell<OpenBlockLines>,
    /// The last line outside comments, with the line count and version it
    /// was found for.
    last_outside_comment_cache: Cell<Option<(usize, u64, Option<usize>)>>,
    /// The last line [`Self::code_trimmed_of`] found among the lines, by
    /// address and length at a version, and the length of its code.
    code_trimmed_cache: Cell<Option<(usize, usize, u64, usize)>>,
    /// The last look back for a line that is `else` or ends with `} else`.
    recent_else_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line whose code is `else` or ends with
    /// `} else`.
    recent_code_else_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line whose code holds `#if`.
    recent_if_directive_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line led by `#`.
    recent_hash_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line that starts with `if ` or `while `.
    recent_if_while_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line that starts with `typedef `.
    recent_typedef_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line that is `(` alone.
    recent_open_paren_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line whose code is `enum`.
    recent_enum_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line whose code starts with `else,`.
    recent_else_comma_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line whose code is led by `#`.
    recent_code_hash_cache: Cell<Option<RecentMatch>>,
    recent_code_has_hash_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line that leads with a case label, a
    /// `switch` or a `}`, or ends with `{`.
    recent_case_edge_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line that is `{` alone.
    recent_brace_alone_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line that holds `@ {`.
    recent_at_brace_cache: Cell<Option<RecentMatch>>,
    /// The last look back for a line that is `else` or ends with `} else`.
    recent_scoped_else_cache: Cell<Option<RecentMatch>>,
    /// Largest first token of a line pushed so far.
    largest_first_token: Option<usize>,
    /// Whether a line ever recorded a first token before that of an earlier
    /// line; until then lines can be searched by token.
    first_tokens_unordered: bool,
    /// The lines [`Self::line_with_token`] found lately, most recent first;
    /// lookups of nearby tokens start from the nearest.
    token_line_hints: Cell<[usize; TOKEN_LINE_HINTS]>,
}

/// The comment lines that end the lines in scope, blank lines aside: the
/// line before them, the farthest that is no `//` comment, and the nearest
/// `//` comment.
#[derive(Clone, Copy, Default)]
pub(crate) struct CommentRun {
    pub(crate) before: Option<usize>,
    farthest_block: Option<usize>,
    nearest_line: Option<usize>,
}

impl CommentRun {
    /// The line whose indent the comments take: the farthest that is no
    /// `//` comment, or else the nearest.
    pub(crate) fn indent_line(&self) -> Option<usize> {
        self.farthest_block.or(self.nearest_line)
    }
}

/// A [`OutputBuffer::last_lines_where`] answer with the scope start,
/// version, and line count it read.
type LastLinesRead = (usize, u64, usize, [Option<usize>; 2]);

/// Which lines [`OutputBuffer::last_lines_where`] looks for.
#[derive(Clone, Copy)]
pub(crate) enum LineFilter {
    /// Lines with text that is no `//` comment.
    NoLineComment = 0,
    /// Lines that are neither a directive nor a `//` comment, blank lines
    /// among them.
    NoDirectiveOrLineComment = 1,
}

impl LineFilter {
    fn admits(self, line: &str) -> bool {
        let head = line.trimmed_start();
        match self {
            LineFilter::NoLineComment => !head.is_empty() && !head.starts_with("//"),
            LineFilter::NoDirectiveOrLineComment => {
                !head.starts_with('#') && !head.starts_with("//")
            }
        }
    }
}

/// Whether trimmed `text` is a comment line or a block comment row.
fn is_comment_row(text: &str) -> bool {
    text.starts_with("//")
        || text.starts_with("/*")
        || text.starts_with("*/")
        || text == "*"
        || text.starts_with("* ")
        || text.starts_with("*\t")
        || text.starts_with("**")
}

/// An entry of [`OpenBraceLines`]: a line with `{`s still open, and how
/// many, or a directive line no brace after it closed past.
#[derive(Clone, Copy)]
enum OpenBraceEntry {
    Line { index: u32, open: u32 },
    Directive,
}

/// Look backs that keep what they read by the line, and so read again the
/// lines changed since they last asked.
#[derive(Clone, Copy)]
pub(crate) enum LookBack {
    OpenLambda,
    ActiveCase,
    MacroBlock,
    OpenSwitch,
    ClosingBrace,
}

const LOOK_BACKS: usize = 5;

/// How a walk back from the last line for an open block counts a line's
/// braces. Either walk closes the line's `{`s by its own `}`s before
/// earlier ones.
#[derive(Clone, Copy, Default)]
enum BlockWalk {
    /// The walk for the switch the output stands in: the line stays open
    /// until later lines close as many `{`s as it has.
    #[default]
    Switch,
    /// The walk for the brace a closing brace closes: the line is open
    /// while its own `}`s leave a `{` that later lines do not close, or,
    /// led by `} else`, until a later line closes a brace of it.
    ClosingBrace,
}

/// The lines a [`BlockWalk`] back from the last line finds open, read on as
/// lines come; the reads of the last lines are kept to read a changed line
/// again.
#[derive(Default)]
struct OpenBlockLines {
    /// Lines read.
    read: usize,
    blocks: Vec<OpenBlock>,
    /// How reading each of the last lines changed `blocks`, oldest first,
    /// each line's changes after its mark.
    undo: std::collections::VecDeque<BlockUndo>,
    /// The lines `undo` holds.
    undo_lines: usize,
}

#[derive(Clone, Copy)]
struct OpenBlock {
    line: usize,
    /// The line is open while fewer `}`s of later lines reach it.
    open_below: usize,
    /// The `}`s of later lines the line's `{`s take: those its own `}`s
    /// leave.
    takes: usize,
    /// The `}`s of later lines that reached the line.
    reached: usize,
}

#[derive(Clone, Copy)]
enum BlockUndo {
    /// Reading the line began.
    Line(usize),
    Pushed,
    Changed(usize, OpenBlock),
    Removed(usize, OpenBlock),
}

/// How many of the last lines an [`OpenBlockLines`] can read again.
const UNDONE_LINES: usize = 8;

impl OpenBlockLines {
    /// Reads the lines of `output` on, again from line `changed` on.
    fn read_on(&mut self, output: &OutputBuffer, walk: BlockWalk, changed: usize) {
        if changed < self.read && !self.rewind(changed) {
            *self = Self::default();
        }
        while self.read < output.len() {
            self.read_line(output, walk, self.read);
            self.read += 1;
        }
    }

    fn read_line(&mut self, output: &OutputBuffer, walk: BlockWalk, index: usize) {
        if self.undo_lines == UNDONE_LINES {
            self.undo_lines -= 1;
            self.undo.pop_front();
            while self
                .undo
                .front()
                .is_some_and(|undo| !matches!(undo, BlockUndo::Line(_)))
            {
                self.undo.pop_front();
            }
        }
        self.undo.push_back(BlockUndo::Line(index));
        self.undo_lines += 1;
        // The body of a block comment holds no braces.
        if matches!(walk, BlockWalk::ClosingBrace) && output.comment_start_index(index) != index {
            return;
        }
        let meta = output.brace_meta(index);
        let (opens, closes) = (meta.opens(), meta.closes());
        let takes = opens.saturating_sub(closes);
        let open_below = match walk {
            BlockWalk::Switch => opens,
            BlockWalk::ClosingBrace => {
                let code = output.code_trimmed(index);
                let else_line =
                    opens > 0 && (code.starts_with("} else") || code.starts_with("}else"));
                takes.max(usize::from(else_line))
            }
        };
        let mut passing = closes.saturating_sub(opens);
        let mut at = self.blocks.len();
        while passing > 0 && at > 0 {
            at -= 1;
            let block = self.blocks[at];
            let reached = block.reached + passing;
            passing -= passing.min(block.takes.saturating_sub(block.reached));
            if reached >= block.open_below {
                self.blocks.remove(at);
                self.undo.push_back(BlockUndo::Removed(at, block));
            } else {
                self.blocks[at].reached = reached;
                self.undo.push_back(BlockUndo::Changed(at, block));
            }
        }
        if open_below > 0 {
            self.blocks.push(OpenBlock {
                line: index,
                open_below,
                takes,
                reached: 0,
            });
            self.undo.push_back(BlockUndo::Pushed);
        }
    }

    /// Undoes the reads of the lines from `line` on; `false` when they are
    /// no longer kept.
    fn rewind(&mut self, line: usize) -> bool {
        while self.read > line {
            loop {
                match self.undo.pop_back() {
                    None => return false,
                    Some(BlockUndo::Line(index)) => {
                        self.undo_lines -= 1;
                        self.read = index;
                        break;
                    }
                    Some(BlockUndo::Pushed) => {
                        self.blocks.pop();
                    }
                    Some(BlockUndo::Changed(at, block)) => self.blocks[at] = block,
                    Some(BlockUndo::Removed(at, block)) => self.blocks.insert(at, block),
                }
            }
        }
        true
    }
}

/// The answers of a look back by the line it starts from, from line
/// `start` on: an answer holds while the lines up to its line stay as they
/// were.
pub(crate) struct LookBackAnswers<T> {
    start: usize,
    answers: Vec<Option<T>>,
}

impl<T> Default for LookBackAnswers<T> {
    fn default() -> Self {
        Self {
            start: 0,
            answers: Vec::new(),
        }
    }
}

/// Lines read in order with a stack of what they leave open, as the look
/// back from the last line for its innermost open brace finds it: each
/// `}` closes the nearest `{` open, and a directive no brace closed past
/// ends that look.
#[derive(Default)]
struct OpenBraceLines {
    read: usize,
    stack: Vec<OpenBraceEntry>,
    /// How each of the last lines read changed the stack, to read a changed
    /// line again.
    undo: std::collections::VecDeque<OpenBraceUndo>,
    /// A later branch of a conditional group was read: the look back then
    /// counts its braces apart.
    branched: bool,
}

/// What reading line `index` took off an [`OpenBraceLines`] stack that
/// stood `len` entries high: its top entries as they were, when no more
/// than four.
#[derive(Clone, Copy)]
struct OpenBraceUndo {
    index: usize,
    len: usize,
    taken: Option<([OpenBraceEntry; 4], usize)>,
}

/// How many of the last changes to lines a look back can read past.
const RECENT_CHANGES: usize = 16;

/// How many lines found lately a token's line lookup starts from: a line's
/// layout looks up its own tokens, its statement's, and its block's.
const TOKEN_LINE_HINTS: usize = 4;

/// A look back for the last line a test admits, kept to read on from.
#[derive(Default)]
pub(crate) struct LineLook(Cell<Option<RecentMatch>>);

/// The last line from `floor` on that a look back found, for a buffer of
/// `len` lines at `version`.
#[derive(Clone, Copy)]
struct RecentMatch {
    len: usize,
    version: u64,
    floor: usize,
    found: Option<usize>,
}

impl OutputBuffer {
    /// The hints of `line`, without the searches for words whose flags are
    /// set: the flags only ever turn on.
    pub(super) fn line_hints(&self, line: &str) -> OutputLineHints {
        output_line_hints(
            line,
            !self.may_have_else,
            !self.may_have_new,
            !self.may_have_asm,
        )
    }

    fn record_hints(&mut self, line: &str, hints: OutputLineHints) {
        if !self.may_have_label_open && hints.has_colon && line.trimmed_end().ends_with('{') {
            let meta = compute_line_brace_meta(line, None);
            self.may_have_label_open |= meta.open_shape == OpenBraceShape::Label;
        }
        self.may_have_else |= hints.has_else;
        self.may_have_hash |= hints.has_hash;
        self.may_have_comment |= hints.has_slash || hints.starts_star;
        self.may_have_question |= hints.has_question;
        self.may_have_at |= hints.has_at;
        self.may_have_new |= hints.has_new;
        self.may_have_asm |= hints.has_asm;
    }

    pub(crate) fn push(&mut self, line: String) {
        let hints = self.line_hints(&line);
        self.push_with_hints(line, hints);
    }

    pub(super) fn push_with_hints(&mut self, line: String, hints: OutputLineHints) {
        self.record_hints(&line, hints);
        let index = self.lines.len();
        let blank = line.trimmed().is_empty();
        if !blank {
            self.last_non_empty_index.set(Some(index));
            self.last_non_empty_dirty.set(false);
        }
        self.lines.push(line);
        self.meta.push(OnceCell::new());
        self.parens.push(OnceCell::new());
        self.push_pending_sources(blank);
    }

    pub(super) fn push_raw_literal(&mut self, line: String, structural_start: usize) {
        let suffix = line.get(structural_start..).unwrap_or("");
        self.record_hints(suffix, self.line_hints(suffix));
        self.may_have_at |= line.contains('@');
        self.may_have_new |= line.contains("new ");
        self.may_have_asm |= line.contains("asm");
        let meta = compute_raw_literal_line_meta(&line, structural_start);
        let index = self.lines.len();
        let blank = line.trimmed().is_empty();
        if !blank {
            self.last_non_empty_index.set(Some(index));
            self.last_non_empty_dirty.set(false);
        }
        self.lines.push(line);
        self.meta.push(OnceCell::from(meta));
        self.parens.push(OnceCell::new());
        self.push_pending_sources(blank);
        self.mark_last_verbatim();
    }

    /// Keeps the last pushed line as written.
    pub(crate) fn mark_last_verbatim(&mut self) {
        if let Some(last) = self.verbatim.last_mut() {
            *last = true;
        }
    }

    pub(crate) fn is_verbatim(&self, index: usize) -> bool {
        self.verbatim.get(index).copied().unwrap_or(false)
    }

    pub(crate) fn mark_last_indented_directive_continuation(&mut self) {
        if let Some(last) = self.indented_directive_continuation.last_mut() {
            *last = true;
        }
    }

    pub(crate) fn is_indented_directive_continuation(&self, index: usize) -> bool {
        self.indented_directive_continuation
            .get(index)
            .copied()
            .unwrap_or(false)
    }

    /// Gives the pending sources to the line just pushed; a blank line, such
    /// as one inserted before the line they belong to, holds none.
    /// Makes room for `lines` more lines.
    pub(crate) fn reserve(&mut self, lines: usize) {
        self.lines.reserve(lines);
        self.meta.reserve(lines);
        self.parens.reserve(lines);
        self.tokens.reserve(lines);
        self.comments.reserve(lines);
        self.verbatim.reserve(lines);
        self.indented_directive_continuation.reserve(lines);
    }

    fn push_pending_sources(&mut self, blank: bool) {
        if let Some(start) = self.pending_scope_start
            && self.lines.len() > start + 1
        {
            self.scope_start = start;
            self.pending_scope_start = None;
        }
        self.verbatim.push(false);
        self.indented_directive_continuation.push(false);
        if blank {
            self.tokens.push(PackedSpan::NONE);
            self.comments
                .push(PackedComments::pack(LineComments::default()));
            return;
        }
        let tokens = self.pending_tokens.take();
        if let Some(span) = tokens {
            self.first_tokens_unordered |= self
                .largest_first_token
                .is_some_and(|largest| span.first < largest);
            self.largest_first_token = Some(
                self.largest_first_token
                    .map_or(span.first, |largest| largest.max(span.first)),
            );
        }
        self.tokens.push(PackedSpan::pack(tokens));
        let comments = self.pending_comments.take().unwrap_or(LineComments {
            lead: self.active_comment,
            last: self.active_comment,
        });
        self.comments.push(PackedComments::pack(comments));
    }

    /// Removes the last line with the code tokens it held.
    pub(crate) fn pop_with_tokens(&mut self) -> Option<(String, Option<TokenSpan>)> {
        let tokens = self.tokens.last().and_then(|span| span.get());
        self.pop().map(|line| (line, tokens))
    }

    pub(crate) fn pop(&mut self) -> Option<String> {
        self.meta.pop();
        self.parens.pop();
        self.tokens.pop();
        self.comments.pop();
        self.verbatim.pop();
        self.indented_directive_continuation.pop();
        let line = self.lines.pop();
        if line.is_some() {
            self.last_non_empty_dirty.set(true);
            self.record_change(self.lines.len());
        }
        line
    }

    pub(crate) fn last_mut(&mut self) -> Option<&mut String> {
        if let Some(slot) = self.meta.last_mut() {
            *slot = OnceCell::new();
            if let Some(parens) = self.parens.last_mut() {
                *parens = OnceCell::new();
            }
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.may_have_at = true;
            self.may_have_new = true;
            self.may_have_asm = true;
            self.last_non_empty_dirty.set(true);
            self.record_change(self.lines.len() - 1);
        }
        self.lines.last_mut()
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut String> {
        if let Some(slot) = self.meta.get_mut(index) {
            *slot = OnceCell::new();
            self.parens[index] = OnceCell::new();
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.may_have_at = true;
            self.may_have_new = true;
            self.may_have_asm = true;
            self.last_non_empty_dirty.set(true);
            self.record_change(index);
        }
        self.lines.get_mut(index)
    }

    pub(crate) fn remove(&mut self, index: usize) -> String {
        self.meta.remove(index);
        self.parens.remove(index);
        self.tokens.remove(index);
        self.comments.remove(index);
        self.verbatim.remove(index);
        self.indented_directive_continuation.remove(index);
        self.last_non_empty_dirty.set(true);
        self.record_change(index);
        self.lines.remove(index)
    }

    /// Appends the text of line `from` to line `into`, after `separator`,
    /// and removes line `from`; `into` then holds the tokens of both.
    /// The lines, without the tables kept about them.
    pub(crate) fn into_lines(self) -> Vec<String> {
        self.lines
    }

    pub(crate) fn join_into(&mut self, into: usize, from: usize, separator: &str) {
        let text = self.lines[from].trimmed().to_string();
        let span = match (self.tokens[into].get(), self.tokens[from].get()) {
            (Some(a), Some(b)) => Some(TokenSpan {
                first: a.first.min(b.first),
                last: a.last.max(b.last),
            }),
            (a, b) => a.or(b),
        };
        self.first_tokens_unordered |=
            span.map(|span| span.first) != self.tokens[into].get().map(|span| span.first);
        self.remove(from);
        let mut line = self.lines[into].trimmed_end().to_string();
        line.push_str(separator);
        line.push_str(&text);
        self.set(into, line);
        self.tokens[into] = PackedSpan::pack(span);
    }

    pub(crate) fn set(&mut self, index: usize, line: String) {
        let hints = self.line_hints(&line);
        self.record_hints(&line, hints);
        self.meta[index] = OnceCell::new();
        self.parens[index] = OnceCell::new();
        self.lines[index] = line;
        self.last_non_empty_dirty.set(true);
        self.record_change(index);
    }

    /// Makes `tokens` the source tokens of the next pushed line.
    pub(crate) fn set_pending_tokens(&mut self, tokens: Option<TokenSpan>) {
        self.pending_tokens = tokens;
    }

    /// Makes `comments` the block comments of the next pushed line.
    pub(crate) fn set_pending_comments(&mut self, comments: LineComments) {
        self.pending_comments = Some(comments);
    }

    pub(crate) fn clear_pending_sources(&mut self) {
        self.pending_tokens = None;
        self.pending_comments = None;
    }

    /// Makes lines pushed without pending comments hold the text of the
    /// block comment token `index`.
    pub(crate) fn set_active_comment(&mut self, index: Option<usize>) {
        self.active_comment = index;
    }

    /// Whether the line about to be pushed continues a block comment that
    /// an earlier line opened.
    pub(crate) fn pending_line_continues_comment(&self) -> bool {
        let comment = self
            .pending_comments
            .map_or(self.active_comment, |comments| comments.lead);
        comment.is_some_and(|comment| {
            self.last_non_empty_index()
                .is_some_and(|index| self.comments[index].get().mentions(comment))
        })
    }

    /// Source tokens of the line being finished, before it is pushed.
    pub(crate) fn pending_tokens(&self) -> Option<TokenSpan> {
        self.pending_tokens
    }

    /// Source tokens of line `index`, when known. `first` is the first code
    /// token on the line; `last` can run past the line when the line was
    /// split after it was formed.
    pub(crate) fn line_tokens(&self, index: usize) -> Option<TokenSpan> {
        self.tokens.get(index).and_then(|span| span.get())
    }

    /// Changes with every edit of a line already pushed; with the line
    /// count, it identifies the buffer's content.
    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn as_slice(&self) -> &[String] {
        &self.lines
    }

    pub(crate) fn range_mut(&mut self, range: std::ops::Range<usize>) -> &mut [String] {
        for slot in &mut self.meta[range.clone()] {
            *slot = OnceCell::new();
        }
        for slot in &mut self.parens[range.clone()] {
            *slot = OnceCell::new();
        }
        if !range.is_empty() {
            self.may_have_label_open = true;
            self.may_have_else = true;
            self.may_have_hash = true;
            self.may_have_comment = true;
            self.may_have_question = true;
            self.may_have_at = true;
            self.may_have_new = true;
            self.may_have_asm = true;
            self.last_non_empty_dirty.set(true);
            self.record_change(range.start);
        }
        &mut self.lines[range]
    }

    pub(crate) fn brace_meta(&self, index: usize) -> &LineBraceMeta {
        self.meta[index].get_or_init(|| {
            let line = &self.lines[index];
            // A row of a block comment holds no code.
            if self.tokens[index].get().is_none() && self.comments[index].get().lead.is_some() {
                let (comment_split_limit, comment_code_end) = comment_split(line);
                LineBraceMeta {
                    code_starts_with_hash: false,
                    closes: narrow(0),
                    opens: narrow(0),
                    open_shape: OpenBraceShape::Other,
                    trim_start_byte: narrow(line.len() - line.trimmed_start().len()),
                    trim_end_byte: narrow(line.trimmed_end().len()),
                    code_end_byte: narrow(0),
                    code_marks: 0,
                    code_opens: narrow(0),
                    paren_closes: narrow(0),
                    paren_open_count: narrow(0),
                    paren_last_open_column: (None).map(narrow),
                    comment_split_limit,
                    comment_code_end,
                    else_line: is_else_line(line.trimmed()),
                    code_else_line: false,
                }
            } else {
                compute_line_brace_meta(line, Some(&self.parens[index]))
            }
        })
    }

    /// Unmatched `)`/`]` of line `index` and the columns of its unmatched
    /// `(`/`[`, outside literals and comments.
    pub(crate) fn paren_imbalance(&self, index: usize) -> (usize, &[usize]) {
        let (closes, opens) = self.parens[index]
            .get_or_init(|| line_paren_imbalance(self.lines[index].trimmed_end()));
        (*closes, opens)
    }

    /// The recent line whose code `text` is, from at most its indent to its
    /// code's end with or without trailing blanks, and the offset of `text`
    /// in it.
    pub(crate) fn code_line_of(&self, text: &str) -> Option<(usize, usize)> {
        let address = text.as_ptr() as usize;
        let recent = self.lines.len().saturating_sub(16);
        let index = (recent..self.lines.len()).rev().find(|&index| {
            let start = self.lines[index].as_ptr() as usize;
            (start..=start + self.lines[index].len()).contains(&address)
        })?;
        let line = &self.lines[index];
        let offset = address - line.as_ptr() as usize;
        let code = self.code_before_comment(index);
        let end = offset + text.len();
        (line.as_bytes()[..offset]
            .iter()
            .all(u8::is_ascii_whitespace)
            && (end == code.len() || end == code.trimmed_end().len()))
        .then_some((index, offset))
    }

    /// The last unmatched `(`/`[` of line `index` with code after it, as
    /// `unmatched_open_paren_column` finds it in the line's code.
    pub(crate) fn unmatched_open_paren_column(&self, index: usize) -> Option<usize> {
        let code = self.code_before_comment(index);
        self.paren_imbalance(index)
            .1
            .iter()
            .rev()
            .copied()
            .filter(|&column| column < code.len())
            .find(|&column| code[column + 1..].chars().any(|ch| !ch.is_whitespace()))
    }

    /// `code_before_comment_trimmed(index)` without its leading whitespace.
    pub(crate) fn code_body(&self, index: usize) -> &str {
        let code = self.code_before_comment_trimmed(index);
        &code[self.lead_bytes(index).min(code.len())..]
    }

    /// The length of line `index`'s leading whitespace.
    pub(crate) fn lead_bytes(&self, index: usize) -> usize {
        self.brace_meta(index).trim_start_byte as usize
    }

    pub(crate) fn trimmed(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index]
            [meta.trim_start_byte as usize..meta.trim_end_byte.max(meta.trim_start_byte) as usize]
    }

    pub(crate) fn code(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index][..meta.code_end_byte as usize]
    }

    /// Line `index` up to its trailing comment, as
    /// `trailing_comment_split_limit` cuts it.
    pub(crate) fn code_before_comment(&self, index: usize) -> &str {
        &self.lines[index][..self.brace_meta(index).comment_split_limit as usize]
    }

    /// `code_before_comment` without the blanks at its end.
    pub(crate) fn code_before_comment_trimmed(&self, index: usize) -> &str {
        &self.lines[index][..self.brace_meta(index).comment_code_end as usize]
    }

    /// `trimmed(index)`, `code(index)` and
    /// `code_before_comment_trimmed(index)`, read from the metadata once.
    pub(crate) fn trimmed_and_codes(&self, index: usize) -> (&str, &str, &str) {
        let meta = self.brace_meta(index);
        let line = &self.lines[index];
        (
            &line[meta.trim_start_byte as usize
                ..meta.trim_end_byte.max(meta.trim_start_byte) as usize],
            &line[..meta.code_end_byte as usize],
            &line[..meta.comment_code_end as usize],
        )
    }

    /// `line` up to its trailing comment, as `trailing_comment_split_limit`
    /// cuts it; one of the last lines is cut from its cached metadata.
    pub(crate) fn code_of<'a>(&'a self, line: &'a str) -> &'a str {
        let recent = self.lines.len().saturating_sub(8);
        // The same bytes in memory are that line, borrowed unchanged.
        if let Some(index) = (recent..self.lines.len()).rev().find(|&index| {
            let held = &self.lines[index];
            held.as_ptr() == line.as_ptr() && held.len() == line.len()
        }) {
            return self.code_before_comment(index);
        }
        &line[..trailing_comment_split_limit(line)]
    }

    /// `code_of(line)` without its trailing blanks.
    pub(crate) fn code_trimmed_of<'a>(&'a self, line: &'a str) -> &'a str {
        let address = line.as_ptr() as usize;
        // A held line changes only with the version.
        if let Some((held, len, version, code)) = self.code_trimmed_cache.get()
            && held == address
            && len == line.len()
            && version == self.version
        {
            return &line[..code];
        }
        let recent = self.lines.len().saturating_sub(8);
        if let Some(index) = (recent..self.lines.len()).rev().find(|&index| {
            let held = &self.lines[index];
            held.as_ptr() as usize == address && held.len() == line.len()
        }) {
            let code = self.code_before_comment_trimmed(index);
            self.code_trimmed_cache
                .set(Some((address, line.len(), self.version, code.len())));
            return code;
        }
        line[..trailing_comment_split_limit(line)].trimmed_end()
    }

    /// Whether `code(index)` leaves a `{` open, as `has_unmatched_open_brace`
    /// finds.
    pub(crate) fn code_has_unmatched_open_brace(&self, index: usize) -> bool {
        self.brace_meta(index).code_opens > 0
    }

    /// Whether `code(index)` holds `byte`, one of `{`, `}`, `:`, `(`, and
    /// `)`.
    pub(crate) fn code_has(&self, index: usize, byte: u8) -> bool {
        self.brace_meta(index).code_marks & code_mark(byte) != 0
    }

    pub(crate) fn code_trimmed(&self, index: usize) -> &str {
        let meta = self.brace_meta(index);
        &self.lines[index]
            [meta.trim_start_byte.min(meta.code_end_byte) as usize..meta.code_end_byte as usize]
    }

    pub(crate) fn lead_width(&self, index: usize, tab_width: usize) -> usize {
        leading_visual_width(&self.lines[index], tab_width)
    }

    /// Whether lines record their tokens in order and the lines after `line`
    /// start past `token`, so that none of them holds `token` or a token
    /// before it.
    pub(crate) fn lines_after_start_past(&self, line: usize, token: usize) -> bool {
        !self.first_tokens_unordered
            && self.tokens.get(line + 1..).is_some_and(|after| {
                after
                    .iter()
                    .find_map(|span| span.get())
                    .is_none_or(|span| span.first > token)
            })
    }

    /// The tokens of line `index` and the first token of the next line
    /// that records tokens, when lines record their tokens in order:
    /// [`Self::line_with_token`] finds this line for a token from the
    /// span's first up to before that next one, if the span holds it.
    pub(crate) fn ordered_line_reach(&self, index: usize) -> Option<(TokenSpan, usize)> {
        if self.first_tokens_unordered {
            return None;
        }
        let span = self.tokens.get(index)?.get()?;
        let next = self.tokens[index + 1..]
            .iter()
            .find_map(|span| span.get())
            .map_or(usize::MAX, |span| span.first);
        Some((span, next))
    }

    /// Index of the output line that holds the source token `token`, among
    /// lines that recorded their tokens.
    pub(crate) fn line_with_token(&self, token: usize) -> Option<usize> {
        if !self.first_tokens_unordered
            && let Some(found) = self.line_with_token_near_hint(token)
        {
            return found;
        }
        let index = if self.first_tokens_unordered {
            self.tokens
                .iter()
                .rposition(|span| span.get().is_some_and(|span| span.first <= token))
        } else {
            self.last_line_starting_by(token)
        };
        if let Some(index) = index {
            self.keep_token_line_hint(TOKEN_LINE_HINTS - 1, index);
        }
        index.filter(|&index| {
            self.tokens[index]
                .get()
                .is_some_and(|span| span.contains(token))
        })
    }

    /// Makes `line` the most recent hint in place of the one in `slot`.
    fn keep_token_line_hint(&self, slot: usize, line: usize) {
        let mut hints = self.token_line_hints.get();
        let mut carried = line;
        for hint in &mut hints[..=slot] {
            carried = std::mem::replace(hint, carried);
        }
        self.token_line_hints.set(hints);
    }

    /// `line_with_token` read on from the nearest line found lately that
    /// starts by `token`, when a later line soon starts past it; `None`
    /// when the hints do not settle the answer.
    fn line_with_token_near_hint(&self, token: usize) -> Option<Option<usize>> {
        let mut nearest: Option<(usize, usize)> = None;
        for (slot, &hint) in self.token_line_hints.get().iter().enumerate() {
            if nearest.is_none_or(|(_, nearest)| hint > nearest)
                && self
                    .tokens
                    .get(hint)
                    .and_then(|span| span.get())
                    .is_some_and(|span| span.first <= token)
            {
                nearest = Some((slot, hint));
            }
        }
        let (slot, hint) = nearest?;
        let mut found = hint;
        let mut lines_read = 0;
        for (index, span) in self.tokens.iter().enumerate().skip(hint + 1) {
            if let Some(span) = span.get() {
                if span.first > token {
                    break;
                }
                found = index;
            }
            lines_read += 1;
            if lines_read == 8 {
                return None;
            }
        }
        self.keep_token_line_hint(slot, found);
        Some(
            self.tokens[found]
                .get()
                .is_some_and(|span| span.contains(token))
                .then_some(found),
        )
    }

    /// The last line whose first token is at most `token`, by binary search
    /// over lines recorded in token order.
    fn last_line_starting_by(&self, token: usize) -> Option<usize> {
        // The first token of the last line at or before `index` that
        // recorded its tokens, with that line.
        let first_at = |index: usize| {
            self.tokens[..=index]
                .iter()
                .rposition(|span| span.get().is_some())
                .map(|line| (self.tokens[line].get().map_or(0, |span| span.first), line))
        };
        let (mut low, mut high) = (0, self.tokens.len());
        while low < high {
            let middle = low + (high - low) / 2;
            if first_at(middle).is_none_or(|(first, _)| first <= token) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low.checked_sub(1).and_then(first_at).map(|(_, line)| line)
    }

    pub(crate) fn current_closing_brace_open(
        &self,
        tab_width: usize,
    ) -> Option<(usize, OpenBraceShape, &str)> {
        let index = self.open_block_lines(
            &self.closing_brace_blocks,
            BlockWalk::ClosingBrace,
            LookBack::ClosingBrace,
            0,
            Some,
        )?;
        let meta = self.brace_meta(index);
        Some((
            self.lead_width(index, tab_width),
            meta.open_shape,
            self.code_trimmed(index),
        ))
    }

    /// The first answer `wanted` gives for a line from `floor` on that the
    /// walk for the switch the output stands in finds open, innermost
    /// first.
    pub(crate) fn open_switch_walk_line<T>(
        &self,
        floor: usize,
        wanted: impl FnMut(usize) -> Option<T>,
    ) -> Option<T> {
        self.open_block_lines(
            &self.switch_blocks,
            BlockWalk::Switch,
            LookBack::OpenSwitch,
            floor,
            wanted,
        )
    }

    fn open_block_lines<T>(
        &self,
        lines: &RefCell<OpenBlockLines>,
        walk: BlockWalk,
        look_back: LookBack,
        floor: usize,
        wanted: impl FnMut(usize) -> Option<T>,
    ) -> Option<T> {
        let mut lines = lines.borrow_mut();
        lines.read_on(self, walk, self.take_lowest_change(look_back));
        lines
            .blocks
            .iter()
            .rev()
            .map(|block| block.line)
            .take_while(|&line| line >= floor)
            .find_map(wanted)
    }

    /// Lines from the start of the current top-level construct on: layout
    /// looking back never needs the inside of an earlier function.
    pub(crate) fn scoped(&self) -> &[String] {
        &self.lines[self.scope_start.min(self.lines.len())..]
    }

    /// Indices of the lines [`Self::scoped`] holds.
    pub(crate) fn scoped_range(&self) -> std::ops::Range<usize> {
        self.scope_start.min(self.lines.len())..self.lines.len()
    }

    /// Whether one of the last `count` lines in scope is `else` or ends
    /// with `} else`.
    pub(crate) fn recent_scoped_else_line(&self, count: usize) -> bool {
        let range = self.scoped_range();
        let start = range.start.max(range.end.saturating_sub(count));
        self.has_line_from(&self.recent_scoped_else_cache, start, |index| {
            self.brace_meta(index).else_line
        })
    }

    /// Whether one of the last `count` lines has the code `else` or code
    /// ending with `} else`.
    pub(crate) fn recent_code_else_line(&self, count: usize) -> bool {
        self.has_line_from(
            &self.recent_code_else_cache,
            self.lines.len().saturating_sub(count),
            |index| self.brace_meta(index).code_else_line,
        )
    }

    /// Whether one of the last `count` lines in scope holds `new `.
    pub(crate) fn recent_scoped_line_mentions_new(&self, count: usize) -> bool {
        if !self.may_have_new {
            return false;
        }
        let range = self.scoped_range();
        let start = range.start.max(range.end.saturating_sub(count));
        self.has_line_from(&self.mentions_new_cache, start, |index| {
            self.lines[index].contains("new ")
        })
    }

    /// Whether a line in scope led by `[` follows the last one led by `},`.
    pub(crate) fn designator_since_closed_row(&self) -> bool {
        let range = self.scoped_range();
        let key = (range.start, self.version, range.end);
        let designator = match self.designator_cache.get() {
            Some((start, version, len, designator))
                if (start, version) == (key.0, key.1) && len <= range.end =>
            {
                // The lines come since read on from the last answer.
                self.lines[len..range.end]
                    .iter()
                    .fold(designator, |designator, line| {
                        let trimmed = line.trimmed_start();
                        if trimmed.starts_with("},") {
                            false
                        } else {
                            designator || trimmed.starts_with('[')
                        }
                    })
            }
            _ => self.lines[range]
                .iter()
                .rev()
                .map(|line| line.trimmed_start())
                .take_while(|trimmed| !trimmed.starts_with("},"))
                .any(|trimmed| trimmed.starts_with('[')),
        };
        self.designator_cache
            .set(Some((key.0, key.1, key.2, designator)));
        designator
    }

    pub(crate) fn clear_scope(&mut self) {
        self.scope_start = 0;
        self.pending_scope_start = None;
        // What layout reads back depends on the scope.
        self.version += 1;
    }

    /// Starts a new top-level construct at line `index` once a line after
    /// it is pushed.
    pub(crate) fn set_scope_start(&mut self, index: usize) {
        self.pending_scope_start = Some(index);
    }

    pub(crate) fn last_non_empty_index(&self) -> Option<usize> {
        if self.last_non_empty_dirty.get() {
            self.last_non_empty_index.set(
                self.lines
                    .iter()
                    .rposition(|line| !line.trimmed().is_empty()),
            );
            self.last_non_empty_dirty.set(false);
        }
        self.last_non_empty_index.get()
    }

    /// The last line that is not blank.
    pub(crate) fn last_non_empty(&self) -> Option<&String> {
        self.last_non_empty_index().map(|index| &self.lines[index])
    }

    /// The last line in scope that is not blank.
    pub(crate) fn last_non_empty_scoped(&self) -> Option<&String> {
        self.last_non_empty_index()
            .filter(|&index| index >= self.scope_start)
            .map(|index| &self.lines[index])
    }

    /// Whether a line from `start` on is `else` or ends with `} else`.
    pub(crate) fn has_else_line_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_else_cache, start, |index| {
            self.brace_meta(index).else_line
        })
    }

    /// Whether the code of a line from `start` on holds `#if`.
    pub(crate) fn has_if_directive_code_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_if_directive_cache, start, |index| {
            self.code(index).contains_from_first_byte("#if")
        })
    }

    /// Whether a line from `start` on is led by `#`.
    pub(crate) fn has_hash_led_line_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_hash_cache, start, |index| {
            self.trimmed(index).starts_with('#')
        })
    }

    /// Whether a line from `start` on starts with `if ` or `while `.
    pub(crate) fn has_if_or_while_line_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_if_while_cache, start, |index| {
            let trimmed = self.lines[index].trimmed_start();
            trimmed.starts_with("if ") || trimmed.starts_with("while ")
        })
    }

    /// Whether a line from `start` on starts with `typedef `.
    pub(crate) fn has_typedef_line_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_typedef_cache, start, |index| {
            self.trimmed(index).starts_with("typedef ")
        })
    }

    /// Whether a line from `start` on is `(` alone.
    pub(crate) fn has_open_paren_line_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_open_paren_cache, start, |index| {
            self.trimmed(index) == "("
        })
    }

    /// Whether the code of a line from `start` on is `enum`.
    pub(crate) fn has_enum_code_line_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_enum_cache, start, |index| {
            self.code_trimmed(index) == "enum"
        })
    }

    /// Whether the code of a line from `start` on starts with `else,`.
    pub(crate) fn has_else_comma_code_line_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_else_comma_cache, start, |index| {
            self.code_trimmed(index).starts_with("else,")
        })
    }

    /// Whether the code of a line from `start` on holds `#`.
    pub(crate) fn has_hash_code_line_from(&self, start: usize) -> bool {
        self.has_line_from(&self.recent_code_has_hash_cache, start, |index| {
            self.code(index).contains('#')
        })
    }

    /// Whether the code of a line from `start` on is led by `#`.
    pub(crate) fn has_hash_led_code_line_from(&self, start: usize) -> bool {
        self.last_hash_led_code_line_from(start).is_some()
    }

    /// The last line from `start` on whose code is led by `#`.
    pub(crate) fn last_hash_led_code_line_from(&self, start: usize) -> Option<usize> {
        self.last_line_from(&self.recent_code_hash_cache, start, |index| {
            self.code_trimmed(index).starts_with('#')
        })
    }

    /// Whether a line from `start` on `matches`; lines pushed since the
    /// last look back are read first, and the lines it read are not read
    /// again while none changed.
    fn has_line_from(
        &self,
        cache: &Cell<Option<RecentMatch>>,
        start: usize,
        matches: impl Fn(usize) -> bool,
    ) -> bool {
        self.last_line_from(cache, start, matches).is_some()
    }

    /// The last line from `start` on that `matches` admits, read on from the
    /// answer `cache` holds.
    fn last_line_from(
        &self,
        cache: &Cell<Option<RecentMatch>>,
        start: usize,
        matches: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        self.last_line_in(cache, start, self.lines.len(), matches)
    }

    /// The last line `look`'s test admits from `start` to before `end`;
    /// each look holds to one test.
    pub(crate) fn last_line_looked(
        &self,
        look: &LineLook,
        start: usize,
        end: usize,
        matches: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        self.last_line_in(&look.0, start, end, matches)
    }

    /// The last line from `start` to before `end` that `matches` admits,
    /// read on from the answer `cache` holds.
    fn last_line_in(
        &self,
        cache: &Cell<Option<RecentMatch>>,
        start: usize,
        end: usize,
        matches: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let len = end.min(self.lines.len());
        let start = start.min(len);
        // The cache holds the last match among the lines from its floor on.
        let (floor, found) = match cache
            .get()
            .and_then(|cached| self.recent_match_now(cached, len))
        {
            Some(cached) => {
                let found = (cached.len..len)
                    .rev()
                    .find(|&index| matches(index))
                    .or(cached.found);
                if start < cached.floor {
                    let found =
                        found.or_else(|| (start..cached.floor).rev().find(|&index| matches(index)));
                    (start, found)
                } else {
                    (cached.floor, found)
                }
            }
            None => (start, (start..len).rev().find(|&index| matches(index))),
        };
        cache.set(Some(RecentMatch {
            len,
            version: self.version,
            floor,
            found,
        }));
        found.filter(|&index| index >= start)
    }

    /// Whether one of the last `count` lines in scope holds `@ {`.
    pub(crate) fn recent_scoped_at_brace_line(&self, count: usize) -> bool {
        if !self.may_have_at {
            return false;
        }
        let range = self.scoped_range();
        let start = range.start.max(range.end.saturating_sub(count));
        self.has_line_from(&self.recent_at_brace_cache, start, |index| {
            self.lines[index].contains_from_first_byte("@ {")
        })
    }

    /// The last line that is `{` alone.
    pub(crate) fn last_open_brace_alone_line(&self) -> Option<usize> {
        self.last_line_from(&self.recent_brace_alone_cache, 0, |index| {
            self.trimmed(index) == "{"
        })
    }

    /// The last line from `start` on whose code leads with `case `,
    /// `default:`, `switch` or `}`, or ends with `{`.
    pub(crate) fn last_case_or_block_edge_line_from(&self, start: usize) -> Option<usize> {
        self.last_line_from(&self.recent_case_edge_cache, start, |index| {
            let code = self.code_before_comment_trimmed(index);
            let trimmed = code.trimmed_start();
            trimmed.starts_with("case ")
                || trimmed.starts_with("default:")
                || trimmed.starts_with("switch")
                || code.ends_with('{')
                || trimmed.starts_with('}')
        })
    }

    /// The last non-empty line, unless it continues a block comment: the
    /// tail of a comment is no code, whatever its words.
    #[inline(never)]
    pub(crate) fn last_line_outside_comment(&self) -> Option<&String> {
        self.last_line_outside_comment_index()
            .map(|index| &self.lines[index])
    }

    /// `last_line_outside_comment` with its code before a trailing comment,
    /// trimmed at the end.
    #[inline(never)]
    pub(crate) fn last_code_outside_comment(&self) -> Option<(&String, &str)> {
        self.last_line_outside_comment_index()
            .map(|index| (&self.lines[index], self.code_before_comment_trimmed(index)))
    }

    /// The last non-blank line of the scope when no comment holds it.
    pub(crate) fn last_code_line_scoped(&self) -> Option<&String> {
        let index = self
            .last_non_empty_index()
            .filter(|&index| index >= self.scope_start)?;
        (self.last_line_outside_comment_index() == Some(index)).then(|| &self.lines[index])
    }

    #[inline]
    fn last_line_outside_comment_index(&self) -> Option<usize> {
        match self.last_outside_comment_cache.get() {
            Some((len, version, index)) if len == self.lines.len() && version == self.version => {
                index
            }
            _ => self.read_last_line_outside_comment(),
        }
    }

    #[inline(never)]
    fn read_last_line_outside_comment(&self) -> Option<usize> {
        let index = self.find_last_line_outside_comment();
        self.last_outside_comment_cache
            .set(Some((self.lines.len(), self.version, index)));
        index
    }

    fn find_last_line_outside_comment(&self) -> Option<usize> {
        let index = self.last_non_empty_index()?;
        // A backslash-continued macro body is no code of the lines after it.
        if self
            .directive_of_continuation(index)
            .is_some_and(|directive| {
                preprocessor_directive(self.trimmed(directive)) == Some("define")
            })
        {
            return None;
        }
        (self.comment_start_index(index) == index).then_some(index)
    }

    /// The last non-empty line in the current scope that is not only a
    /// comment: comments take no part in the layout of the code around them.
    pub(crate) fn last_code_line_in_scope(&self) -> Option<&String> {
        (self.scope_start.min(self.lines.len())..self.lines.len())
            .rev()
            .find(|&index| {
                let text = self.trimmed(index);
                !text.is_empty()
                    && (self.line_tokens(index).is_some()
                        || self.comment_start_index(index) == index
                            && !text.starts_with("/*")
                            && !text.starts_with("//"))
            })
            .map(|index| &self.lines[index])
    }

    /// The directive line that line `index` continues through
    /// backslash-newlines.
    pub(crate) fn directive_of_continuation(&self, index: usize) -> Option<usize> {
        let mut line = index;
        // A comment row of the body continues the directive too.
        while line > 0 && self.trimmed(line - 1).ends_with('\\') {
            line -= 1;
            if self.trimmed(line).starts_with('#') {
                return Some(line);
            }
        }
        None
    }

    /// Whether line `index` belongs to a directive, on its first line or on
    /// a backslash-continued one.
    pub(crate) fn is_directive_line(&self, index: usize) -> bool {
        self.trimmed(index).starts_with('#') || self.directive_of_continuation(index).is_some()
    }

    /// The first comment token recorded on line `index`.
    pub(crate) fn comment_token(&self, index: usize) -> Option<usize> {
        let comments = self.comments.get(index)?.get();
        comments.lead.or(comments.last)
    }

    /// Index of the line that opens the comment on line `index`: a block
    /// comment continuation line maps to the line holding its `/*`.
    pub(crate) fn comment_start_index(&self, index: usize) -> usize {
        let comments = self.comments[index].get();
        if let Some(comment) = comments.lead {
            return self.comment_opening_line(index, comment);
        }
        if comments.last.is_some() || self.tokens[index].get().is_some() {
            return index;
        }
        self.text_comment_start_index(index)
    }

    /// First line up to `index` holding text of the block comment token
    /// `comment`; blank lines inside the comment record no tokens.
    fn comment_opening_line(&self, index: usize, comment: usize) -> usize {
        let mut start = index;
        for line in (0..index).rev() {
            if self.comments[line].get().mentions(comment) {
                start = line;
            } else if !self.lines[line].trimmed().is_empty() {
                break;
            }
        }
        start
    }

    /// [`Self::comment_start_index`] for lines that recorded no tokens.
    fn text_comment_start_index(&self, index: usize) -> usize {
        if !self.lines[index].trimmed_start().starts_with('*') {
            return index;
        }
        self.lines[..index]
            .iter()
            .rposition(|line| line.contains("/*") || line.contains("*/"))
            .filter(|&start| {
                let line = &self.lines[start];
                line.rfind("/*") > line.rfind("*/")
            })
            .unwrap_or(index)
    }

    /// Leading width of the comment on line `index`, measured at the line
    /// that opens it.
    pub(crate) fn comment_indent_width(&self, index: usize, tab_width: usize) -> usize {
        leading_visual_width(&self.lines[self.comment_start_index(index)], tab_width)
    }

    /// The innermost line whose `{`s the lines after it leave open, looking
    /// back from the last line past block comment bodies and stopping at a
    /// directive outside braces; `None` when conditional branches leave
    /// that to a look back of its own.
    pub(crate) fn innermost_open_brace_line(&self) -> Option<Option<usize>> {
        self.read_open_brace_lines(&self.open_brace_lines, &self.lowest_change[0], false)
    }

    /// The innermost line whose `{`s the lines after it leave open, every
    /// line read as code.
    pub(crate) fn innermost_open_brace_line_plain(&self) -> Option<usize> {
        self.read_open_brace_lines(&self.plain_open_brace_lines, &self.lowest_change[1], true)
            .expect("no directive branches a plain read")
    }

    /// The innermost line from `floor` on whose `{`s the lines after it
    /// leave open and that `wanted` admits, every line read as code.
    pub(crate) fn innermost_open_brace_line_plain_where(
        &self,
        floor: usize,
        mut wanted: impl FnMut(usize) -> bool,
    ) -> Option<usize> {
        self.innermost_open_brace_line_plain();
        let lines = self.plain_open_brace_lines.borrow();
        lines
            .stack
            .iter()
            .rev()
            .map_while(|entry| match *entry {
                OpenBraceEntry::Line { index, .. } => Some(index as usize),
                OpenBraceEntry::Directive => None,
            })
            .take_while(|&index| index >= floor)
            .find(|&index| wanted(index))
    }

    fn read_open_brace_lines(
        &self,
        lines: &RefCell<OpenBraceLines>,
        lowest_change: &Cell<usize>,
        plain: bool,
    ) -> Option<Option<usize>> {
        let mut lines = lines.borrow_mut();
        let changed = lowest_change.replace(usize::MAX).min(self.lines.len());
        if changed < lines.read {
            // Undo the lines read from the change on, newest first.
            while let Some(undo) = lines.undo.back().copied()
                && undo.index >= changed
            {
                lines.undo.pop_back();
                let Some((taken, count)) = undo.taken else {
                    break;
                };
                let keep = undo.len - count;
                lines.stack.truncate(keep);
                lines.stack.extend_from_slice(&taken[..count]);
                lines.read = undo.index;
            }
            if lines.read == changed {
                lines.branched = false;
            } else {
                *lines = OpenBraceLines::default();
            }
        }
        while !lines.branched && lines.read < self.lines.len() {
            let index = lines.read;
            lines.read += 1;
            if lines.undo.len() == 4 {
                lines.undo.pop_front();
            }
            if !plain && self.comment_start_index(index) != index {
                let len = lines.stack.len();
                lines.undo.push_back(OpenBraceUndo {
                    index,
                    len,
                    taken: Some(([OpenBraceEntry::Directive; 4], 0)),
                });
                continue;
            }
            let meta = self.brace_meta(index);
            let mut closes = meta.closes();
            // A line's `}`s close its own `{`s first, as the look back
            // counts them before the `{`s; the entries before the line they
            // reach are kept to undo them.
            let len = lines.stack.len();
            let mut reach = 0;
            let mut left = closes.saturating_sub(meta.opens());
            while left > 0 && reach < len {
                if let OpenBraceEntry::Line { open, .. } = lines.stack[len - 1 - reach] {
                    left = left.saturating_sub(open as usize);
                }
                reach += 1;
            }
            let taken = (reach <= 4).then(|| {
                let mut taken = [OpenBraceEntry::Directive; 4];
                taken[..reach].copy_from_slice(&lines.stack[len - reach..]);
                (taken, reach)
            });
            lines.undo.push_back(OpenBraceUndo { index, len, taken });
            let directive = !plain && meta.code_starts_with_hash;
            if directive
                && matches!(
                    preprocessor_directive(self.trimmed(index)),
                    Some("else" | "elif" | "elifdef" | "elifndef")
                )
            {
                lines.branched = true;
                break;
            }
            if meta.opens() > 0 {
                lines.stack.push(OpenBraceEntry::Line {
                    index: index as u32,
                    open: meta.opens() as u32,
                });
            }
            while closes > 0 {
                match lines.stack.last_mut() {
                    Some(OpenBraceEntry::Directive) => {
                        lines.stack.pop();
                    }
                    Some(OpenBraceEntry::Line { open, .. }) => {
                        let closed = (*open as usize).min(closes);
                        *open -= closed as u32;
                        closes -= closed;
                        if *open == 0 {
                            lines.stack.pop();
                        }
                    }
                    None => break,
                }
            }
            // The look back stops at a directive before its own braces.
            if directive {
                lines.stack.push(OpenBraceEntry::Directive);
            }
        }
        if lines.branched {
            return None;
        }
        Some(match lines.stack.last() {
            Some(OpenBraceEntry::Line { index, .. }) => Some(*index as usize),
            _ => None,
        })
    }

    /// The last two lines in scope that `filter` admits, the last first.
    pub(crate) fn last_lines_where(&self, filter: LineFilter) -> [Option<usize>; 2] {
        let range = self.scoped_range();
        let key = (range.start, self.version);
        let cache = &self.last_lines_cache[filter as usize];
        let found = match cache.get() {
            Some((start, version, len, found)) if (start, version) == key && len <= range.end => {
                let mut found = found;
                for index in len..range.end {
                    if filter.admits(&self.lines[index]) {
                        found = [Some(index), found[0]];
                    }
                }
                found
            }
            _ => {
                let mut found = [None; 2];
                let mut matches = range
                    .clone()
                    .rev()
                    .filter(|&index| filter.admits(&self.lines[index]));
                found[0] = matches.next();
                found[1] = matches.next();
                found
            }
        };
        cache.set(Some((key.0, key.1, range.end, found)));
        found
    }

    /// The last line in scope before `end` that `filter` admits.
    pub(crate) fn last_line_before_where(&self, end: usize, filter: LineFilter) -> Option<usize> {
        let [last, before] = self.last_lines_where(filter);
        match (last?, before) {
            (last, _) if last < end => Some(last),
            (_, None) => None,
            (_, Some(before)) if before < end => Some(before),
            _ => {
                let range = self.scoped_range();
                (range.start..end.min(range.end))
                    .rev()
                    .find(|&index| filter.admits(&self.lines[index]))
            }
        }
    }

    /// The last line that holds source tokens.
    pub(crate) fn last_line_with_tokens(&self) -> Option<usize> {
        let len = self.lines.len();
        if len > 0 && self.tokens[len - 1].get().is_some() {
            return Some(len - 1);
        }
        let from = match self.token_line_cache.get() {
            Some((version, read, found)) if version == self.version && read <= len => {
                // Only lines come since can hold later tokens.
                (read..len)
                    .rev()
                    .find(|&index| self.tokens[index].get().is_some())
                    .or(found)
            }
            _ => (0..len)
                .rev()
                .find(|&index| self.tokens[index].get().is_some()),
        };
        self.token_line_cache.set(Some((self.version, len, from)));
        from
    }

    /// The comment rows that end the lines in scope; see [`CommentRun`].
    pub(crate) fn comment_run(&self) -> CommentRun {
        let range = self.scoped_range();
        let key = (range.start, self.version);
        let run = match self.comment_run_cache.get() {
            Some((start, version, len, run)) if (start, version) == key && len <= range.end => {
                // Lines come after the run read last: each comment row
                // extends it and any other line starts it afresh.
                let mut run = run;
                for index in len..range.end {
                    let trimmed = self.lines[index].trimmed_start();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if !is_comment_row(trimmed) {
                        run = CommentRun {
                            before: Some(index),
                            ..CommentRun::default()
                        };
                    } else if trimmed.starts_with("//") {
                        run.nearest_line = Some(index);
                    } else {
                        run.farthest_block = run.farthest_block.or(Some(index));
                    }
                }
                run
            }
            _ => {
                let mut run = CommentRun::default();
                for index in range.clone().rev() {
                    let trimmed = self.lines[index].trimmed_start();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if !is_comment_row(trimmed) {
                        run.before = Some(index);
                        break;
                    }
                    if trimmed.starts_with("//") {
                        run.nearest_line = run.nearest_line.or(Some(index));
                    } else {
                        run.farthest_block = Some(index);
                    }
                }
                run
            }
        };
        self.comment_run_cache
            .set(Some((key.0, key.1, range.end, run)));
        run
    }

    /// The answers `look_back` keeps for the lines from `start` on, each
    /// at its line less `start`: those for changed lines are dropped.
    pub(crate) fn look_back_answers<'a, T: Copy>(
        &self,
        look_back: LookBack,
        start: usize,
        answers: &'a mut LookBackAnswers<T>,
    ) -> &'a mut [Option<T>] {
        let changed = self.lowest_change[2 + look_back as usize].replace(usize::MAX);
        if answers.start != start {
            answers.start = start;
            answers.answers.clear();
        }
        answers.answers.truncate(changed.saturating_sub(start));
        answers
            .answers
            .resize(self.lines.len().saturating_sub(start), None);
        &mut answers.answers
    }

    /// The lowest line changed since `look_back` last asked.
    fn take_lowest_change(&self, look_back: LookBack) -> usize {
        self.lowest_change[2 + look_back as usize].replace(usize::MAX)
    }

    fn record_change(&mut self, index: usize) {
        self.version += 1;
        self.recent_changes[(self.version % RECENT_CHANGES as u64) as usize] =
            (self.version, index);
        self.note_change(index);
    }

    /// The lowest line the changes since `version` changed, when all of
    /// them are recorded.
    pub(crate) fn lowest_change_since(&self, version: u64) -> Option<usize> {
        if self.version - version > RECENT_CHANGES as u64 {
            return None;
        }
        (version + 1..=self.version).try_fold(usize::MAX, |lowest, changed| {
            let (at, index) = self.recent_changes[(changed % RECENT_CHANGES as u64) as usize];
            (at == changed).then_some(lowest.min(index))
        })
    }

    /// What `cached` still holds of the lines before `len` now: the lines
    /// before the lowest one changed since it was read.
    fn recent_match_now(&self, cached: RecentMatch, len: usize) -> Option<RecentMatch> {
        if cached.version == self.version {
            return (cached.len <= len).then_some(cached);
        }
        let unchanged = self
            .lowest_change_since(cached.version)?
            .min(cached.len)
            .min(len);
        Some(
            if cached.floor <= unchanged && cached.found.is_none_or(|found| found < unchanged) {
                RecentMatch {
                    len: unchanged,
                    version: self.version,
                    ..cached
                }
            } else {
                RecentMatch {
                    len: unchanged,
                    version: self.version,
                    floor: unchanged,
                    found: None,
                }
            },
        )
    }

    fn note_change(&self, index: usize) {
        for lowest in &self.lowest_change {
            lowest.set(lowest.get().min(index));
        }
    }

    pub(crate) fn may_have_label_open(&self) -> bool {
        self.may_have_label_open
    }

    pub(crate) fn may_have_else(&self) -> bool {
        self.may_have_else
    }

    pub(crate) fn may_have_hash(&self) -> bool {
        self.may_have_hash
    }

    pub(crate) fn may_have_comment(&self) -> bool {
        self.may_have_comment
    }

    pub(crate) fn may_have_question(&self) -> bool {
        self.may_have_question
    }

    /// Whether a line may hold `@`; false while no line ever did.
    /// Whether a line may hold `new `.
    /// Whether some line may hold `asm`.
    pub(crate) fn may_have_asm(&self) -> bool {
        self.may_have_asm
    }

    pub(crate) fn may_have_new(&self) -> bool {
        self.may_have_new
    }

    pub(crate) fn may_have_at(&self) -> bool {
        self.may_have_at
    }
}

impl Deref for OutputBuffer {
    type Target = [String];

    fn deref(&self) -> &[String] {
        &self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenBraceShape, OutputBuffer};

    /// The look back from the last line for the innermost line whose `{`s
    /// the lines after it leave open.
    fn innermost_open_brace_line_by_walk(output: &OutputBuffer) -> Option<usize> {
        let mut depth = 0usize;
        for index in (0..output.len()).rev() {
            let meta = output.brace_meta(index);
            depth += meta.closes();
            if meta.opens() > depth {
                return Some(index);
            }
            depth -= meta.opens();
        }
        None
    }

    /// The look back past block comment bodies that stops at a directive
    /// outside braces, as the layout of a closing brace reads it.
    fn innermost_open_brace_line_past_directives(output: &OutputBuffer) -> Option<Option<usize>> {
        let mut depth = 0usize;
        let mut group_depths = Vec::new();
        for index in (0..output.len()).rev() {
            if output.comment_start_index(index) != index {
                continue;
            }
            let meta = output.brace_meta(index);
            if depth == 0 && meta.code_starts_with_hash {
                return Some(None);
            }
            if meta.code_starts_with_hash {
                match super::preprocessor_directive(output.trimmed(index)) {
                    Some("endif") => group_depths.push(depth),
                    Some("else" | "elif" | "elifdef" | "elifndef") => return None,
                    Some("if" | "ifdef" | "ifndef") => {
                        group_depths.pop();
                    }
                    _ => {}
                }
            }
            depth += meta.closes();
            if meta.opens() > depth {
                return Some(Some(index));
            }
            depth -= meta.opens();
        }
        Some(None)
    }

    #[test]
    fn open_brace_lines_past_directives_match_a_look_back() {
        let lines = [
            "{",
            "}",
            "} else {",
            "a;",
            "{{",
            "}}",
            "f() {",
            "#if A",
            "#endif",
            "#define B {",
            "#define C }",
            "/* c {",
            " * }",
            " */",
            "#else",
            "x; /* { */",
        ];
        let mut state = 0x5be0_cd19_u32;
        for _ in 0..3000 {
            let mut output = OutputBuffer::default();
            for _ in 0..(state % 40) {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let line = lines[(state >> 4) as usize % lines.len()].to_string();
                match state % 6 {
                    0 if !output.is_empty() => {
                        output.pop();
                    }
                    1 if !output.is_empty() => {
                        *output.last_mut().expect("line") = line;
                    }
                    _ => output.push(line),
                }
                let expected = innermost_open_brace_line_past_directives(&output);
                let found = output.innermost_open_brace_line();
                if let Some(found) = found {
                    assert_eq!(Some(found), expected, "{:?}", output.as_slice());
                }
            }
            state = state.wrapping_add(1);
        }
    }

    #[test]
    fn open_brace_lines_read_on_and_undone_match_a_look_back() {
        let lines = [
            "{",
            "}",
            "} else {",
            "a;",
            "{{",
            "}}",
            "x = {1, {2}};",
            "} }",
            "f() {",
        ];
        let mut state = 0x3c6e_f372_u32;
        for _ in 0..2000 {
            let mut output = OutputBuffer::default();
            for _ in 0..(state % 40) {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let line = lines[(state >> 4) as usize % lines.len()].to_string();
                match state % 6 {
                    0 if !output.is_empty() => {
                        output.pop();
                    }
                    1 if !output.is_empty() => {
                        *output.last_mut().expect("line") = line;
                    }
                    2 if output.len() > 2 => {
                        let index = (state >> 9) as usize % output.len();
                        output.set(index, line);
                    }
                    _ => output.push(line),
                }
                assert_eq!(
                    output.innermost_open_brace_line_plain(),
                    innermost_open_brace_line_by_walk(&output),
                    "{:?}",
                    output.as_slice()
                );
            }
            state = state.wrapping_add(1);
        }
    }

    #[test]
    fn label_open_shape_accepts_extension_identifiers() {
        let mut output = OutputBuffer::default();
        output.push("α$value: {".to_string());

        assert_eq!(output.brace_meta(0).open_shape, OpenBraceShape::Label);
    }

    #[test]
    fn raw_literal_suffix_updates_output_hints() {
        let mut output = OutputBuffer::default();
        let line = "R\"(body)\" else".to_string();
        let structural_start = line.find("else").expect("suffix");

        output.push_raw_literal(line, structural_start);

        assert!(output.may_have_else());
    }

    #[test]
    fn lead_width_uses_the_requested_tab_width_after_metadata_is_cached() {
        let mut output = OutputBuffer::default();
        output.push("\tvalue();".to_string());
        output.code(0);

        assert_eq!(output.lead_width(0, 8), 8);
    }

    #[test]
    fn last_non_empty_index_skips_whitespace_only_tail() {
        let mut output = OutputBuffer::default();
        output.push("value();".to_string());
        output.push("    ".to_string());

        assert_eq!(output.last_non_empty_index(), Some(0));
    }
}
