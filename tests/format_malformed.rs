#![allow(clippy::field_reassign_with_default)]

mod common;

use common::format_c;
use cstyle::api::format_bytes;
use cstyle::config::{BraceStyle, FormatOptions, apply_command_line_args};

#[test]
fn non_ascii_before_unmatched_open_bracket_formats_without_panic() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-comma",
        "--align-pointer=name",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\u{e9}\u{e9} [m//";

    assert_eq!(
        format_bytes(source.as_bytes(), &options).expect("format bytes"),
        source.as_bytes(),
    );
}

#[test]
fn non_ascii_before_unmatched_open_paren_continuation_formats_without_panic() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-comma",
        "--align-pointer=name",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_bytes("\u{e9} (\\\n'".as_bytes(), &options).expect("format bytes"),
        "\u{e9} (\\\n    '".as_bytes(),
    );
}

#[test]
fn non_ascii_before_padded_open_paren_formats_without_panic() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=lisp",
        "--indent=force-tab=4",
        "--pad-paren",
        "--fill-empty-lines",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_bytes("\u{e9}\u{e9}(r\na".as_bytes(), &options).expect("format bytes"),
        "\u{e9}\u{e9} ( r\n\t   a".as_bytes(),
    );
}

#[test]
fn non_ascii_line_before_unmatched_open_paren_keeps_char_count_consistent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_bytes("\u{a9}> \ntch (".as_bytes(), &options).expect("format bytes"),
        "\u{a9}>\ntch (".as_bytes(),
    );
}

#[test]
fn gnu_return_with_inline_hash_after_malformed_close_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let expected = ",else..\n}\nreturn #if A\n       whilealpha?NULL\n";

    assert_eq!(
        format_c(",else..} return #if A\nwhilealpha?NULL\n", &options),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn gnu_keeps_operator_after_malformed_close_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let expected = "/* block */==||else helper||constexpr#if A\t*\n}~1\telsecatch\n>]if\n]&  1catch[casecase\n";

    assert_eq!(
        format_c(
            "/* block */==||else helper||constexpr#if A\t*}~1\telsecatch\n>]if\n]&  1catch[casecase\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn keeps_line_after_malformed_close_word_idempotent() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let expected =
        "beta& #else x\n} namespace&helper\nNULL  gamma10| ||:  value\n    ~class catch\n";

    assert_eq!(
        format_c(
            "beta& #else x}namespace&helper\nNULL  gamma10| ||:  value\n~class catch\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn keeps_block_comment_after_malformed_close_word_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let expected = "structdefault\treturn=\n} struct\n/* block */==%\n    auto|| class\n";

    assert_eq!(
        format_c(
            "structdefault\treturn=  }struct\n/* block */==%\nauto|| class\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn allman_keeps_adjacent_bracket_after_malformed_close_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let first = format_bytes(b"(}[f\nA", &options).expect("format bytes");

    assert_eq!(format_bytes(&first, &options).expect("format bytes"), first,);
}

#[test]
fn allman_malformed_close_clears_following_continuation_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let expected = b"(\n}\nf\nA";

    assert_eq!(
        format_bytes(b"(}\nf\nA", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn allman_malformed_close_before_bracket_line_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let expected = b"(\n}\n[f\n A";

    assert_eq!(
        format_bytes(b"(}\n[f\nA", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn allman_malformed_close_word_clears_following_continuation_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let expected = b"(\n} r\nf\nA";

    assert_eq!(
        format_bytes(b"(}r\nf\nA", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn allman_unmatched_close_inside_bracket_keeps_brace_at_column_zero() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let expected = b"[(\n}\n f\n A";

    assert_eq!(
        format_bytes(b"[(}\nf\nA", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn allman_malformed_close_number_clears_following_continuation_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let expected = b"(\n}\n2\nA";

    assert_eq!(
        format_bytes(b"(}\n2\nA", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn allman_unmatched_close_before_indented_bracket_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let expected = b"[(\n}\n [f\n  f";

    assert_eq!(
        format_bytes(b"[(}\n[f\nf", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn allman_malformed_close_identifier_clears_comma_continuation_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let expected = b"(\n}\nf\n,";

    assert_eq!(
        format_bytes(b"(}\nf\n,", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn allman_malformed_close_clears_following_ternary_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let expected = "(\n}\n::\n}\ngamma\n?\ncontinue\n<=\n42\n,#if A\n";

    assert_eq!(
        format_c("(}\n::}\ngamma\n?\ncontinue\n<=\n42\n,#if A\n", &options,),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn allman_jump_keyword_does_not_start_operator_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;

    for keyword in [
        "break",
        "continue",
        "throw",
        "goto",
        "co_return",
        "co_yield",
        "co_await",
    ] {
        let source = format!("{keyword}\n<=\n42\n");
        assert_eq!(format_c(&source, &options), source);
    }
}

#[test]
fn allman_standalone_operator_does_not_indent_following_line() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = "<=\n42\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn allman_malformed_else_comma_clears_following_indent_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let input = "&&else}!\nelse, continue\nclassgamma\n/helperclasselse\nwhile)#define X(x) \\==(\nenum\nalpha::beta;0  alphaItemItem!try!+\t::!#else\n!=helper#else/* block */#else\n!for[#define X(x) \\return]\nautoenum0\n// line  Configcase!=%call\n0\nxwhile// line\n||:(auto#endif  /* block */gamma\nConfigcall+Item#elsetry// line||\n)<=enum\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_header_after_blank_clears_continuation_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let input = "do-<elseItem// line=\n\n    constexpr18>while[catch7?value>\n\ncase&::\ncall)namespace/>\n\nvoid{constexprwhile21try!=+&&elseelse>=namespaceintstructgammavoidenum!=38catch+36~beta)helper23*alpha\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_leading_operator_header_before_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let input = "(ifItemthrow#define X(x) \\10forvoidcallreturn<<class<=,+&(elsevalueconstexpr)+continueifdowhiledo*trythrowauto18constexprconstexpr31for<=>{namespacetry-/&&**return!=<default||throw:\n/beta\n38betacontinuereturn[intdefault=// line10*forstruct[.else})for  switchclass->int!\n\n?]whilecall\tauto\n\n<=>int#if A~37void%-:try{,#if Aclass\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_word_after_scope_line_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let first = format_c("x{<=&&::do\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_unmatched_close_number_does_not_join_following_word() {
    let options = FormatOptions::default();
    let expected = "if\n} 1\nf\n";

    assert_eq!(format_c("if}1\nf\n", &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn operator_line_before_attached_unmatched_close_is_idempotent() {
    let options = FormatOptions::default();
    let first = format_c("-\ndo-if}\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_blank_line_after_unmatched_bracket_clears_operator_indent() {
    let options = FormatOptions::default();
    let input = "switch(zthrowswitch>=[}[1&&>alphadefaultItemcall+?namespace\tbeta==else?}&whilehelper=Itemswitch42&&alphahelper&  *&&constexprcase*]1},1if[value\n\n<>=<\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_colon_before_operator_line_is_idempotent() {
    let options = FormatOptions::default();
    let input = "case/* block */alphanamespace42  /* block */result<=>%beta||#else->/else->NULL&\t1if);<=->gamma,throw#elsecall:~&&|/#endifNULL&&==\n\n#endifNULL\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_stream_operator_after_block_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let input = ".   >=\t// comment\n\t] &\t / \t( ==x  result{\n\t>>NULL\tclass\tresult <=>   helper\t#define X(x) \\/* block */  NULL\n <=>  \t.\tz1  >=    <=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_identifier_after_embedded_define_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let input = ".   >=\t// comment\n\t] &\t / \t( ==x  result{\n\t>>NULL\tclass\tresult <=>   helper\t#define X(x) \\/* block */  NULL\n Itemnamespace if  \tcontinue1\tdefault  \t)   x#else\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_leading_operator_chain_after_block_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let input = ".   >=\t// comment\n\t] &\t / \t( ==x  result{\n  |\tconstexpr\t\tif1 for\t\tauto\n <=>  \t.\tz1  >=    <=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_return_colon_tail_uses_stable_continuation_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let expected = b"return +:\n       o";

    assert_eq!(
        format_bytes(b"return +:o", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn unmatched_close_recovery_overrides_whitesmith_return_continuation() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let expected = b"}\nreturn +:\no";

    assert_eq!(
        format_bytes(b"}return +:o", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn gnu_malformed_close_before_operator_chain_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let input = "else<=enum}\n!=-  ()%\t#endifwhiley\n1\tbreak/* block */else\t/catchcatchconstexpr ->struct\n||\n?\n=::doz\n% alpha\nelse else(/while\nydefault!=&&continue\nreturn,\nif{\t+namespace  betaItem#else\tswitchzConfig\ndo<=z->\n;(\nconstexpr<=\n{ do42>=struct?NULLConfig\ngamma\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}
