#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::api::format_bytes;
use cstyle::config::{BraceStyle, FormatOptions, IndentStyle, Mode, apply_command_line_args};

const ONE_TRUE_BRACE_C_ARGS: &[&str] = &[
    "--style=1tbs",
    "--mode=c",
    "--indent=spaces=4",
    "--indent-switches",
    "--indent-preprocessor",
    "--indent-preproc-define",
    "--indent-col1-comments",
    "--add-braces",
    "--pad-oper",
    "--pad-comma",
    "--pad-header",
    "--unpad-paren",
    "--break-one-line-headers",
    "--break-after-logical",
    "--align-pointer=name",
    "--align-reference=name",
    "--attach-closing-while",
    "--attach-return-type",
    "--attach-return-type-decl",
    "--min-conditional-indent=0",
    "--max-continuation-indent=80",
    "--max-code-length=109",
    "--convert-tabs",
];

fn one_true_brace_c_options() -> FormatOptions {
    let mut options = FormatOptions::default();
    let args = ONE_TRUE_BRACE_C_ARGS
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect::<Vec<_>>();
    apply_command_line_args(&mut options, &args).expect("valid C options");
    options
}

#[test]
fn preprocessor_guarded_braceless_body_keeps_header_body_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "#ifdef ANALYZER",
        "    if (value)",
        "#endif",
        "        call();",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn empty_else_if_blocks_keep_sibling_indent() {
    let actual = format_c(
        fixture!(
            "void f(int z){",
            "  if(z==0){",
            "    one();",
            "  }else if(z==1){",
            "  }else if(z==2){",
            "    two();",
            "  }else if(z==3){",
            "  }else if(z==4){",
            "    int v = 0;",
            "    use(v);",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int z) {",
            "    if(z==0) {",
            "        one();",
            "    } else if(z==1) {",
            "    } else if(z==2) {",
            "        two();",
            "    } else if(z==3) {",
            "    } else if(z==4) {",
            "        int v = 0;",
            "        use(v);",
            "    }",
            "}",
        )
    );
}

#[test]
fn else_after_one_line_else_if_keeps_header_column() {
    let actual = format_c(
        fixture!(
            "const char *find(const char *a, size_t alen,",
            "                         const char *b, size_t blen) {",
            "  if (blen == 0) return a;",
            "  else if (blen > alen) return 0;",
            "  else {",
            "    return a;",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "const char *find(const char *a, size_t alen,",
            "                 const char *b, size_t blen) {",
            "    if (blen == 0) return a;",
            "    else if (blen > alen) return 0;",
            "    else {",
            "        return a;",
            "    }",
            "}",
        )
    );
}

#[test]
fn else_if_after_trailing_multiline_comment_keeps_header_column() {
    let actual = format_c(
        fixture!(
            "void f(int n){",
            "  if( n ){",
            "    first();",
            "  }else if( other ){",
            "    for(i=0; i<n; i++){",
            "      if( skip ) continue;",
            "      call(i);",
            "    }",
            "    value = 1;  /* describe this assignment",
            "               ** across two lines */",
            "  }else if( next ){",
            "    enter_mode();",
            "    active = 1;",
            "  }else if( last ){",
            "    done();",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int n) {",
            "    if( n ) {",
            "        first();",
            "    } else if( other ) {",
            "        for(i=0; i<n; i++) {",
            "            if( skip ) continue;",
            "            call(i);",
            "        }",
            "        value = 1;  /* describe this assignment",
            "               ** across two lines */",
            "    } else if( next ) {",
            "        enter_mode();",
            "        active = 1;",
            "    } else if( last ) {",
            "        done();",
            "    }",
            "}",
        )
    );
}

#[test]
fn else_after_commented_same_line_control_body_keeps_header_indent() {
    assert_eq!(
        format_c(
            "void f(int value) {\n  while (value) {\n    if (value < 1) {\n      --value;\n    } else if (value < 2) return; /* note */\n    else {\n      ++value;\n    }\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int value) {\n    while (value) {\n        if (value < 1) {\n            --value;\n        } else if (value < 2) return; /* note */\n        else {\n            ++value;\n        }\n    }\n}\n",
    );
}

#[test]
fn wrapped_else_if_condition_keeps_condition_continuation_indent() {
    assert_eq!(
        format_c(
            "void f(int value) {\n  if (value) {\n  } else if (alpha(value)==0\n      && beta(value)) {\n    call();\n  } else if (gamma(value)==0\n      && delta(value)) {\n    done();\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int value) {\n    if (value) {\n    } else if (alpha(value)==0\n               && beta(value)) {\n        call();\n    } else if (gamma(value)==0\n               && delta(value)) {\n        done();\n    }\n}\n",
    );
}

#[test]
fn macro_call_in_for_header_does_not_make_loop_body_raw() {
    let source = "\nvoid foo(void)\n{\n    for (i = 0; i < NUM_VALUE(limit); i++) {\n        call();\n    }\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn braceless_if_with_multiline_condition_indents_body_one_level_deeper() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tfor (node = first; *node; node = next)\n\t\tif ((*node)->group == group &&\n\t\t    (*node)->index == index)\n\t\t\tbreak;\n\tx = 1;\n}\n",
            &options,
        ),
        "void f(void)\n{\n    for (node = first; *node; node = next)\n        if ((*node)->group == group &&\n            (*node)->index == index)\n            break;\n    x = 1;\n}\n",
    );
}

#[test]
fn else_after_multiline_braceless_body_aligns_to_if_not_continuation() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (cond)\n\t\tfoo(\"a\",\n\t\t\tn1,\n\t\t\tn2);\n\telse\n\t\tbar();\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (cond)\n        foo(\"a\",\n            n1,\n            n2);\n    else\n        bar();\n}\n",
    );
}

#[test]
fn nested_do_while_closes_outer_loop_at_header_indent() {
    let actual = format_c(
        fixture!(
            "void f(void) {",
            "  do{",
            "    for(i=0; i<count; i++){",
            "      call();",
            "    }",
            "    if( term==sep ){",
            "      do{",
            "        read();",
            "        i++;",
            "      }while( term==sep );",
            "      print();",
            "    }",
            "    if( i>=count ){",
            "      if( rc!=OK ){",
            "        if( bail ) break;",
            "      }else{",
            "        row++;",
            "      }",
            "    }",
            "  }while( term!=EOF );",
            "  done();",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "    do {",
            "        for(i=0; i<count; i++) {",
            "            call();",
            "        }",
            "        if( term==sep ) {",
            "            do {",
            "                read();",
            "                i++;",
            "            } while( term==sep );",
            "            print();",
            "        }",
            "        if( i>=count ) {",
            "            if( rc!=OK ) {",
            "                if( bail ) break;",
            "            } else {",
            "                row++;",
            "            }",
            "        }",
            "    } while( term!=EOF );",
            "    done();",
            "}",
        )
    );
}

#[test]
fn multiline_else_if_closing_paren_uses_continuation_indent() {
    let actual = format_c(
        fixture!(
            "void f(char *pattern) {",
            "  if( one ){",
            "    call();",
            "  }else if( same(pattern,\"-a\")==0",
            "         || same(pattern,\"-all\")==0",
            "         || same(pattern,\"--all\")==0",
            "  ){",
            "    pattern = \".\";",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(char *pattern) {",
            "    if( one ) {",
            "        call();",
            "    } else if( same(pattern,\"-a\")==0",
            "               || same(pattern,\"-all\")==0",
            "               || same(pattern,\"--all\")==0",
            "             ) {",
            "        pattern = \".\";",
            "    }",
            "}",
        )
    );
}

#[test]
fn multiline_condition_after_pointer_signature_uses_header_closing_indent() {
    let actual = format_c(
        fixture!(
            "static char *f(const char *text, Db *db){",
            "  int offset;",
            "  if( db==0",
            "   || text==0",
            "   || (offset = get_offset(db))<0",
            "   || offset>=(int)strlen(text)",
            "  ){",
            "    return empty();",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "static char *f(const char *text, Db *db) {",
            "    int offset;",
            "    if( db==0",
            "            || text==0",
            "            || (offset = get_offset(db))<0",
            "            || offset>=(int)strlen(text)",
            "      ) {",
            "        return empty();",
            "    }",
            "}",
        )
    );
}

#[test]
fn force_tab_x_nested_condition_closer_uses_visual_open_column() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;
    options.tab_width = 4;

    assert_eq!(
        format_c(
            fixture!("void f(){", "if((alpha", " || beta", ")){}", "}",),
            &options,
        ),
        fixture!(
            "void f() {",
            "\tif((alpha",
            "\t\t\t|| beta",
            "\t   )) {}",
            "}",
        ),
    );
}

#[test]
fn formats_configured_control_headers() {
    let mut options = FormatOptions::default();
    options.pad_header = true;
    options.control_headers = vec!["FOR_EACH".to_string()];
    options.non_paren_headers = vec!["FOREVER".to_string()];
    let actual = format_with(
        fixture!(
            "void f(){",
            "FOR_EACH(item, items){",
            "call(item);",
            "}",
            "FOREVER{",
            "tick();",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    FOR_EACH (item, items)",
            "    {",
            "        call(item);",
            "    }",
            "    FOREVER",
            "    {",
            "        tick();",
            "    }",
            "}",
        )
    );
}

#[test]
fn breaks_do_while_closing_header_by_default() {
    let actual = format(fixture!("void f(){do{x++;}while(x<3);while(x<5){x++;}}"));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    do",
            "    {",
            "        x++;",
            "    }",
            "    while (x < 3);",
            "    while (x < 5)",
            "    {",
            "        x++;",
            "    }",
            "}",
        )
    );
}

#[test]
fn closing_brace_keeps_source_space_before_else_and_while() {
    let source = fixture!(
        "",
        "void foo()",
        "{",
        "    if (x) {",
        "        bar();",
        "    } else {",
        "        baz();",
        "    }",
        "    do {",
        "        bar();",
        "    } while (x);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn breaks_closing_else_when_requested() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.break_closing_braces = true;
    let actual = format_with(
        fixture!("int f(int x){if(x){return 1;} else {return 0;}}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x) {",
            "        return 1;",
            "    }",
            "    else {",
            "        return 0;",
            "    }",
            "}",
        )
    );
}
#[test]
fn ratliff_attached_do_while_closer_uses_body_brace_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Ratliff;
    options.indent_braces = true;
    options.indent_classes = true;
    options.attach_closing_while = true;

    assert_eq!(
        format_c("void run(){\ndo{one();}while(ready);\n}\n", &options),
        fixture!(
            "void run() {",
            "    do {",
            "        one();",
            "        } while(ready);",
            "    }",
        )
    );
}

#[test]
fn ratliff_do_while_breaks_closing_brace_before_while() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void f(int n) {\ndo {\nn++;\n} while(n<5);\n}\n", &options,),
        "void f(int n) {\n    do {\n        n++;\n        }\n    while(n<5);\n    }\n",
    );
}

#[test]
fn pico_attaches_requested_do_while_closer() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.attach_closing_while = true;

    assert_eq!(
        format_c("void run(){\ndo{one();}while(ready);\n}\n", &options),
        fixture!("void run()", "{   do {one();} while(ready); }")
    );
}

#[test]
fn breaks_closing_do_while_when_requested() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.break_closing_braces = true;
    let actual = format_with(fixture!("void f(){do{x++;}while(x<3);}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    do {",
            "        x++;",
            "    }",
            "    while (x < 3);",
            "}",
        )
    );
}
#[test]
fn attaches_closing_do_while_for_attached_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(fixture!("void f(){do{x++;}while(x<3);}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    do {",
            "        x++;",
            "    } while (x < 3);",
            "}",
        )
    );
}
#[test]
fn aligns_multiline_for_header_clauses() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_header = true;
    let actual = format_with(
        fixture!(
            "void f(){",
            "for (pos = (byte *) base + size;",
            "     pos < (byte *) base + count * size;",
            "     pos += size) {",
            "work();",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    for (pos = (byte *) base + size;",
            "            pos < (byte *) base + count * size;",
            "            pos += size) {",
            "        work();",
            "    }",
            "}",
        )
    );
}
#[test]
fn keeps_leading_block_comment_inside_for_header() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!("void f(){", "for(/* void */; n < end; n++) {}", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    for (/* void */; n < end; n++) {}",
            "}",
        )
    );
}
#[test]
fn indented_styles_keep_broken_else_if_bodies_with_their_braces() {
    let source = "void run(){\nif(alpha){one();}else if(beta){two();}else{three();}\n}\n";

    let mut whitesmith = FormatOptions::default();
    whitesmith.brace_style = BraceStyle::Whitesmith;
    whitesmith.indent_braces = true;
    whitesmith.indent_classes = true;
    whitesmith.indent_switches = true;
    whitesmith.break_else_ifs = true;
    assert_eq!(
        format_c(source, &whitesmith),
        fixture!(
            "void run()",
            "    {",
            "    if(alpha)",
            "        {",
            "        one();",
            "        }",
            "    else",
            "        if(beta)",
            "            {",
            "            two();",
            "            }",
            "        else",
            "            {",
            "            three();",
            "            }",
            "    }",
        )
    );

    let mut vtk = FormatOptions::default();
    vtk.brace_style = BraceStyle::Vtk;
    vtk.break_else_ifs = true;
    assert_eq!(
        format_c(source, &vtk),
        fixture!(
            "void run()",
            "{",
            "    if(alpha)",
            "        {",
            "        one();",
            "        }",
            "    else",
            "        if(beta)",
            "            {",
            "            two();",
            "            }",
            "        else",
            "            {",
            "            three();",
            "            }",
            "}",
        )
    );
}

#[test]
fn break_else_ifs_keeps_final_else_nested_in_allman_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.break_else_ifs = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha){one();}else if(beta){two();}else{three();}\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(alpha)",
            "    {",
            "        one();",
            "    }",
            "    else",
            "        if(beta)",
            "        {",
            "            two();",
            "        }",
            "        else",
            "        {",
            "            three();",
            "        }",
            "}",
        )
    );
}

#[test]
fn gnu_break_else_ifs_keeps_nested_braces_with_their_headers() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    options.break_else_ifs = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha){one();}else if(beta){two();}else{three();}\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(alpha)",
            "        {",
            "            one();",
            "        }",
            "    else",
            "        if(beta)",
            "            {",
            "                two();",
            "            }",
            "        else",
            "            {",
            "                three();",
            "            }",
            "}",
        )
    );
}

#[test]
fn break_else_ifs_breaks_and_nests_chained_else_if() {
    let mut options = FormatOptions::default();
    options.break_else_ifs = true;
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "        x();",
            "    } else if (b) {",
            "        y();",
            "    } else if (c) {",
            "        z();",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "        x();",
            "    } else",
            "        if (b) {",
            "            y();",
            "        } else",
            "            if (c) {",
            "                z();",
            "            }",
            "}",
        )
    );
}
#[test]
fn no_indent_if_after_else_aligns_broken_else_if_chain() {
    let mut options = FormatOptions::default();
    options.break_else_ifs = true;
    options.no_indent_if_after_else = true;
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "        x();",
            "    } else if (b) {",
            "        y();",
            "    } else if (c) {",
            "        z();",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "        x();",
            "    } else",
            "    if (b) {",
            "        y();",
            "    } else",
            "    if (c) {",
            "        z();",
            "    }",
            "}",
        )
    );
}

#[test]
fn no_indent_if_after_else_requires_break_else_ifs() {
    let mut options = FormatOptions::default();
    options.no_indent_if_after_else = true;
    let source = fixture!(
        "void f(void)",
        "{",
        "    if (a) {",
        "        x();",
        "    } else if (b) {",
        "        y();",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn no_indent_if_after_else_leaves_braceless_else_body_nested() {
    let mut options = FormatOptions::default();
    options.break_else_ifs = true;
    options.no_indent_if_after_else = true;
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    if (a)",
            "        x();",
            "    else",
            "        y();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    if (a)",
            "        x();",
            "    else",
            "        y();",
            "}",
        )
    );
}

#[test]
fn break_else_ifs_unwinds_indent_between_separate_chains() {
    let mut options = FormatOptions::default();
    options.break_else_ifs = true;
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "        x();",
            "    } else if (b) {",
            "        y();",
            "    }",
            "    g();",
            "    if (c) {",
            "        z();",
            "    } else if (d) {",
            "        w();",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "        x();",
            "    } else",
            "        if (b) {",
            "            y();",
            "        }",
            "    g();",
            "    if (c) {",
            "        z();",
            "    } else",
            "        if (d) {",
            "            w();",
            "        }",
            "}",
        )
    );
}
#[test]
fn attach_closing_while_attaches_broken_do_while() {
    let mut options = FormatOptions::default();
    options.attach_closing_while = true;
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    do {",
            "        x();",
            "    }",
            "    while (a);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    do {",
            "        x();",
            "    } while (a);",
            "}",
        )
    );
}
#[test]
fn preserves_braceless_control_body_on_its_own_line() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (alpha == beta)", "    helper();"), &options),
        fixture!("if (alpha == beta)", "    helper();")
    );
    assert_eq!(
        format_c(
            fixture!("for (i = 0; i < n; i++)", "    helper();"),
            &options
        ),
        fixture!("for (i = 0; i < n; i++)", "    helper();")
    );
    assert_eq!(
        format_c(fixture!("while (alpha)", "    helper();"), &options),
        fixture!("while (alpha)", "    helper();")
    );
}
#[test]
fn recomputes_braceless_control_body_indent_ignoring_source_indent() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (alpha)", "result();"), &options),
        fixture!("if (alpha)", "    result();")
    );
    assert_eq!(
        format_c(fixture!("if (alpha)", "        result();"), &options),
        fixture!("if (alpha)", "    result();")
    );
}
#[test]
fn keeps_braceless_if_else_bodies_separate() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("if (alpha)", "    one();", "else", "    two();"),
            &options
        ),
        fixture!("if (alpha)", "    one();", "else", "    two();")
    );
}
#[test]
fn inline_nested_if_indents_dangling_else_under_inner_header() {
    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &FormatOptions::default(),
        ),
        fixture!(
            "void run() {",
            "    if(alpha) if(beta) one();",
            "        else two();",
            "}",
        )
    );
}

#[test]
fn nested_braceless_bodies_indent_cumulatively() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (alpha)", "if (beta)", "gamma();"), &options),
        fixture!("if (alpha)", "    if (beta)", "        gamma();")
    );
}
#[test]
fn following_statement_returns_to_header_indent() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (alpha)", "    one();", "two();"), &options),
        fixture!("if (alpha)", "    one();", "two();")
    );
}
#[test]
fn preserves_blank_line_between_header_and_braceless_body() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (alpha)", "", "    one();"), &options),
        fixture!("if (alpha)", "", "    one();")
    );
}
#[test]
fn keeps_braceless_else_if_joined_but_breaks_its_body() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "if (alpha) {",
                "    one();",
                "}",
                "else",
                "if (beta) two();"
            ),
            &options
        ),
        fixture!("if (alpha) {", "    one();", "}", "else if (beta) two();")
    );
    assert_eq!(
        format_c(
            fixture!(
                "if (alpha) {",
                "    one();",
                "}",
                "else if (beta)",
                "two();"
            ),
            &options
        ),
        fixture!(
            "if (alpha) {",
            "    one();",
            "}",
            "else if (beta)",
            "    two();"
        )
    );
}

#[test]
fn else_if_after_comment_only_block_keeps_chain_indent() {
    assert_eq!(
        format_c(
            "void f(char *s) {\n  int i, j, c, stop = 0;\n  for (i = j = 0; (c = s[i]) != 0; i++) { /* Scan */\n    if (c == stop) {\n      stop = 0;\n    } else if (stop != 0) {\n      /* No-op */\n    } else if (c == '\"' || c == '\\\'' || c == '`') {\n      stop = c;\n    } else if (c == '[') {\n      stop = ']';\n    }\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(char *s) {\n    int i, j, c, stop = 0;\n    for (i = j = 0; (c = s[i]) != 0; i++) { /* Scan */\n        if (c == stop) {\n            stop = 0;\n        } else if (stop != 0) {\n            /* No-op */\n        } else if (c == '\"' || c == '\\\'' || c == '`') {\n            stop = c;\n        } else if (c == '[') {\n            stop = ']';\n        }\n    }\n}\n",
    );
}

#[test]
fn else_after_nested_loop_in_if_keeps_if_indent() {
    assert_eq!(
        format_c(
            "void f(int quote) {\n  if( quote ){\n    for(i=0; i<n; i++){\n      if( value[i]==quote ) len++;\n    }\n  }\n\n  if( quote ){\n    char *out = text;\n    for(i=0; i<n; i++){\n      *out++ = value[i];\n      if( value[i]==quote ) *out++ = quote;\n    }\n    result = out - text;\n    *out = '\\0';\n  }else{\n    copy();\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int quote) {\n    if( quote ) {\n        for(i=0; i<n; i++) {\n            if( value[i]==quote ) len++;\n        }\n    }\n\n    if( quote ) {\n        char *out = text;\n        for(i=0; i<n; i++) {\n            *out++ = value[i];\n            if( value[i]==quote ) *out++ = quote;\n        }\n        result = out - text;\n        *out = '\\0';\n    } else {\n        copy();\n    }\n}\n",
    );
}

#[test]
fn first_statement_after_comment_in_else_if_block_keeps_body_indent() {
    assert_eq!(
        format_c(
            "void f(int value) {\n  if( value==1 ){\n    one();\n  }else if( value==0 ){\n    /* text */\n    int show = 0;\n    call(show);\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int value) {\n    if( value==1 ) {\n        one();\n    } else if( value==0 ) {\n        /* text */\n        int show = 0;\n        call(show);\n    }\n}\n",
    );
}

#[test]
fn attached_else_if_logical_condition_continuation_aligns_to_condition() {
    assert_eq!(
        format_c(
            "void f(void) {\n  if( first ){\n    one();\n  }else if( compare(name,\"value\")==0\n    && enabled() ){\n    two();\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void) {\n    if( first ) {\n        one();\n    } else if( compare(name,\"value\")==0\n               && enabled() ) {\n        two();\n    }\n}\n",
    );
}

#[test]
fn linux_one_line_return_else_if_chain_keeps_header_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = fixture!(
        "int f(int value)",
        "{",
        "    int first = 0;",
        "    int second = 1; /* note */",
        "",
        "    if (value == 0) return first;",
        "    else if (value == 1) return second;",
        "    else if (value == 2) {",
        "        return 2;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn linux_preserves_source_break_before_control_header_condition_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tif",
                "\t(cond())",
                "\t\tx();",
                "\tfor",
                "\t(;;)",
                "\t\tx();",
                "\twhile",
                "\t\t(cond())",
                "\t\tx();",
                "\tdo {",
                "\t\tx();",
                "\t} while",
                "\t(cond());",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    if",
            "    (cond())",
            "        x();",
            "    for",
            "    (;;)",
            "        x();",
            "    while",
            "    (cond())",
            "        x();",
            "    do {",
            "        x();",
            "    } while",
            "    (cond());",
            "}",
        )
    );
}

#[test]
fn catch_like_macro_after_block_stays_attached() {
    let source = fixture!(
        "void f()",
        "{",
        "    TRY {",
        "        call();",
        "    } CATCH(int) {",
        "        handle();",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn catch_member_name_does_not_start_control_header() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "static inline void init(struct s *p,",
                "\t\t\t\t\tfn_t try,",
                "\t\t\t\t\tfn_t catch,",
                "\t\t\t\t\tunsigned long timeout)",
                "{",
                "\tp->try = try;",
                "\tp->catch = catch;",
                "\tp->timeout = timeout;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "static inline void init(struct s *p,",
            "                        fn_t try,",
            "                        fn_t catch,",
            "                        unsigned long timeout)",
            "{",
            "    p->try = try;",
            "    p->catch = catch;",
            "    p->timeout = timeout;",
            "}",
        )
    );
}

#[test]
fn if_keyword_inside_macro_argument_does_not_indent_following_definition() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "DEFINE_FREE(p, T *, if (x) put(y))",
                "",
                "static int f(void)",
                "{",
                "\treturn 0;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "DEFINE_FREE(p, T *, if (x) put(y))",
            "",
            "static int f(void)",
            "{",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn macro_arguments_named_like_headers_do_not_leak_statement_indent() {
    let source = "ENABLE_RULE(switch)\nENABLE_RULE(switch-default)\nENABLE_RULE(switch-enum)\nENABLE_RULE(synth)\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn preserves_braceless_do_while_body_and_empty_for_body() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("do", "    work();", "while (alpha);"), &options),
        fixture!("do", "    work();", "while (alpha);")
    );
    assert_eq!(
        format_c(fixture!("for (i = 0; i < n; i++)", ";"), &options),
        fixture!("for (i = 0; i < n; i++)", "    ;")
    );
}
#[test]
fn keeps_brace_body_at_header_indent() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (alpha)", "{", "    one();", "}"), &options),
        fixture!("if (alpha)", "{", "    one();", "}")
    );
}

#[test]
fn keeps_braceless_body_at_one_level_after_own_line_comment() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (alpha)", "// note", "beta();"), &options),
        fixture!("if (alpha)", "// note", "    beta();")
    );
}

#[test]
fn indents_else_body_after_column_one_comments() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "void foo()",
                "{",
                "if (alpha)",
                "beta = 1;",
                "else",
                "//  if (gamma)",
                "//      beta = 3;",
                "//  else",
                "if (delta)",
                "beta = 4;",
                "}"
            ),
            &options
        ),
        fixture!(
            "void foo()",
            "{",
            "    if (alpha)",
            "        beta = 1;",
            "    else",
            "//  if (gamma)",
            "//      beta = 3;",
            "//  else",
            "        if (delta)",
            "            beta = 4;",
            "}"
        )
    );
}

#[test]
fn keeps_braceless_body_at_one_level_after_block_comment() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (alpha)", "/* note */", "beta();"), &options),
        fixture!("if (alpha)", "    /* note */", "    beta();")
    );
}

#[test]
fn keeps_brace_at_header_level_after_own_line_comment() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("if (alpha)", "// note", "{", "beta();", "}"),
            &options
        ),
        fixture!("if (alpha)", "// note", "{", "    beta();", "}")
    );
}

#[test]
fn braceless_else_body_do_while_block_keeps_extra_indent() {
    let actual = format(fixture!(
        "void foo()",
        "{",
        "    if (alpha)",
        "        beta;",
        "    else",
        "        do {",
        "            gamma();",
        "        } while (count < 9);",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "void foo()",
            "{",
            "    if (alpha)",
            "        beta;",
            "    else",
            "        do",
            "        {",
            "            gamma();",
            "        }",
            "        while (count < 9);",
            "}",
        )
    );
}

#[test]
fn else_while_braceless_body_gets_nested_body_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    if (a)",
        "        call();",
        "    else while (b)",
        "            next();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn braceless_else_while_header_body_keeps_nested_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo()",
                "{",
                "    if (count == 0)",
                "        value = 0;",
                "    else while (count != 0) {",
                "        value += next();",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "",
            "void foo()",
            "{",
            "    if (count == 0)",
            "        value = 0;",
            "    else while (count != 0) {",
            "            value += next();",
            "        }",
            "}",
        )
    );
}

#[test]
fn else_after_braceless_loop_inside_if_aligns_with_if() {
    let source = fixture!(
        "void f()",
        "{",
        "    if (ready)",
        "        foreach (int i, list)",
        "            call(i);",
        "    else",
        "        fail();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn nested_braceless_if_else_keeps_dangling_else_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    if (a)",
        "        if (b) x();",
        "        else y();",
        "    else z();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn braceless_nested_if_else_with_trailing_comment_keeps_else_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (a)\n    if (b)\n      stop (); /* c */\n    else\n      prev ();\n  else\n    focus ();\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (a)\n        if (b)\n            stop (); /* c */\n        else\n            prev ();\n    else\n        focus ();\n}\n",
    );
}

#[test]
fn nested_braceless_if_else_inside_foreach_keeps_body_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    if (false)",
        "        foreach (int i, list)",
        "            if (i) fail();",
        "            else fail();",
        "    else",
        "        foreach (int i, list)",
        "            if (false) { }",
        "            else call(i);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn braceless_header_body_while_block_indents_block_and_close() {
    let actual = format(fixture!(
        "void f(){",
        "if(alpha)",
        "beta;",
        "else",
        "while(gamma){",
        "delta();",
        "}",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (alpha)",
            "        beta;",
            "    else",
            "        while (gamma)",
            "        {",
            "            delta();",
            "        }",
            "}",
        )
    );
}

#[test]
fn for_header_continuation_applies_min_conditional_indent() {
    let options = FormatOptions::default();
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    for (int i = 0;",
            "     i < n; i++) {",
            "        x();",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    for (int i = 0;",
            "            i < n; i++) {",
            "        x();",
            "    }",
            "}",
        )
    );
}

#[test]
fn for_header_logical_tail_uses_minimum_conditional_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tfor (count = 0; (read() & FLAG) &&",
                "\t\t     (count < 256); count++)",
                "\t\tread();",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    for (count = 0; (read() & FLAG) &&",
            "            (count < 256); count++)",
            "        read();",
            "}",
        )
    );
}

#[test]
fn comment_after_braceless_else_uses_else_body_indent() {
    let source = fixture!(
        "void f(int n){",
        "  if( first ){",
        "    done();",
        "  }else",
        "",
        "  /* comment */",
        "  if( second ){",
        "    call();",
        "  }else",
        "",
        "  /* next comment",
        "  ** tail",
        "  */",
        "  if( third ){",
        "    go();",
        "  }",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "void f(int n) {",
            "    if( first ) {",
            "        done();",
            "    } else",
            "",
            "        /* comment */",
            "        if( second ) {",
            "            call();",
            "        } else",
            "",
            "            /* next comment",
            "            ** tail",
            "            */",
            "            if( third ) {",
            "                go();",
            "            }",
            "}",
        )
    );
}

#[test]
fn semicolonless_macro_body_does_not_indent_following_block() {
    let source = fixture!(
        "void f()",
        "{",
        "    if (a)",
        "        MACRO()",
        "    {",
        "        b();",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn else_line_with_trailing_comment_keeps_following_if_as_body() {
    let source = fixture!(
        "void f()",
        "{",
        "    if (a)",
        "        b();",
        "    else // comment",
        "        if (c)",
        "        {",
        "            d();",
        "        }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn try_identifier_in_condition_does_not_join_braceless_body() {
    let actual = format_c(
        fixture!(
            "int f(int *try)",
            "{",
            "\tif ((*try & 0x1) == 0)",
            "\t\tvalue = 16;",
            "",
            "\tif (size != table[(*try >> 1)])",
            "\t\treturn -1;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int *try)",
            "{",
            "    if ((*try & 0x1) == 0)",
            "        value = 16;",
            "",
            "    if (size != table[(*try >> 1)])",
            "        return -1;",
            "}",
        )
    );
}

#[test]
fn variable_named_try_in_for_loop_keeps_block_indent() {
    let source = "void f(void)\n{\n    for (run = 0; run < N; run++)\n    {\n        for (try = 0; try < n; try++)\n        {\n            Count id = g (0, M);\n        }\n    }\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn word_after_closing_brace_stays_on_next_line_when_not_closing_header() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "\tdo_each(item) {",
            "\t\tcall();",
            "\t} while_each(item);",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    do_each(item) {",
            "        call();",
            "    }",
            "    while_each(item);",
            "}",
        )
    );
}

#[test]
fn braceless_if_body_multiline_while_exits_before_else() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "\tif (first)",
            "\t\twhile (call() &",
            "\t\t\t\tFLAG)",
            "\t\t\t;",
            "\telse",
            "\t\twhile (call() &",
            "\t\t\t\tFLAG)",
            "\t\t\t;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (first)",
            "        while (call() &",
            "                FLAG)",
            "            ;",
            "    else",
            "        while (call() &",
            "                FLAG)",
            "            ;",
            "}",
        )
    );
}

#[test]
fn semicolonless_macro_call_after_if_indents_as_body() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    if ( a )",
            "        MACRO(a)",
            "    else if ( b )",
            "        MACRO(b)",
            "",
            "    return;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if ( a )",
            "        MACRO(a)",
            "        else if ( b )",
            "            MACRO(b)",
            "",
            "            return;",
            "}",
        )
    );
}

#[test]
fn if_condition_method_call_keeps_source_split_open_paren() {
    let source = fixture!(
        "void f()",
        "{",
        "    if ( dialog.SetLabels",
        "            (",
        "                yes,",
        "                no",
        "            ) )",
        "        call();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// `if consteval` and `if !consteval` use ordinary control-block indentation.
#[test]
fn if_consteval_is_indented_as_a_control_block() {
    let positive = fixture!(
        "int f() {",
        "    if consteval {",
        "        return 1;",
        "    }",
        "    return 2;",
        "}",
    );
    let negative = fixture!(
        "int f() {",
        "    if !consteval {",
        "        return 2;",
        "    }",
        "    return 1;",
        "}",
    );

    assert_eq!(format_c(positive, &FormatOptions::default()), positive);
    assert_eq!(format_c(negative, &FormatOptions::default()), negative);
}

#[test]
fn braceless_if_body_with_same_line_compound_literal_call_exits_before_else() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (show)\n    call(child, &(const Allocation) {\n      x, y\n    }, -1);\n  else\n    other();\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (show)\n        call(child, &(const Allocation) {\n            x, y\n        }, -1);\n    else\n        other();\n}\n",
    );
}

#[test]
fn else_after_braceless_ternary_with_nested_call_condition_matches_if_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (message == EVENT_A)\n    direction = (((short) DECODE (value)) > 0)\n                  ? UP\n                  : DOWN;\n  else if (message == EVENT_B)\n    direction = (((short) DECODE (value)) > 0)\n                  ? RIGHT\n                  : LEFT;\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (message == EVENT_A)\n        direction = (((short) DECODE (value)) > 0)\n                    ? UP\n                    : DOWN;\n    else if (message == EVENT_B)\n        direction = (((short) DECODE (value)) > 0)\n                    ? RIGHT\n                    : LEFT;\n}\n",
    );
}

#[test]
fn else_after_multiline_commented_braceless_if_matches_if_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (a)\t/* long note\n\t\t    more */\n\t\tg();\n\telse if (b) {\n\t\th();\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (a)\t/* long note\n\t\t    more */\n        g();\n    else if (b) {\n        h();\n    }\n}\n",
    );
}

#[test]
fn else_after_commented_braceless_if_matches_if_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (a) {\n\t\tif (b)\t\t// note\n\t\t\tg();\n\t\telse\n\t\t\th();\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (a) {\n        if (b)\t\t// note\n            g();\n        else\n            h();\n    }\n}\n",
    );
}

#[test]
fn else_after_nested_braceless_loop_matches_if_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (a)\n\t\tfor (i = 0; i < n; i++)\n\t\t\tcall(i);\n\telse\n\t\tgoto end;\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (a)\n        for (i = 0; i < n; i++)\n            call(i);\n    else\n        goto end;\n}\n",
    );
}

#[test]
fn braceless_body_call_paren_split_keeps_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (a)\n\t\thandle_change\n\t\t\t(&port, b);\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (a)\n        handle_change\n        (&port, b);\n}\n",
    );
}

#[test]
fn braceless_for_if_body_uses_structural_indent() {
    let mut options = FormatOptions::default();
    options.mode = Mode::C;

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tenum item i;\n\n\tfor (i = 0; i < MAX; i++)\n\t\tif (items[i] >= 0)\n\t\t\tcall(items[i], i);\n}\n",
            &options,
        ),
        "void f(void)\n{\n    enum item i;\n\n    for (i = 0; i < MAX; i++)\n        if (items[i] >= 0)\n            call(items[i], i);\n}\n",
    );
}

#[test]
fn linux_style_uses_short_conditional_continuation_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");
    let source = fixture!(
        "void helper(int value)",
        "{",
        "    if (value",
        "        && other) {",
        "        call();",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn linux_style_preserves_tab_before_nested_condition_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void helper(void)\n{\n\tif (alpha &&\n\t    (left ||\t(right && other)))\n\t\tcall(\"x\"\n\t\t     \"y\");\n}\n",
            &options,
        ),
        "void helper(void)\n{\n    if (alpha &&\n        (left ||\t(right && other)))\n        call(\"x\"\n             \"y\");\n}\n"
    );
}

#[test]
fn linux_style_keeps_nested_braceless_else_with_inner_if() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");
    let source = "void helper(void)\n{\n    for (i = 0; i < limit; i++)\n        if (value[i] == 0)\n            call();\n        else\n            other();\n    done();\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn allman_style_indents_else_while_opening_brace_under_while_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void helper(int value)\n{\n    if (value == 0)\n        call();\n    else while (value != 0) {\n            other();\n        }\n    done();\n}\n",
            &options,
        ),
        "void helper(int value)\n{\n    if (value == 0)\n        call();\n    else while (value != 0)\n        {\n            other();\n        }\n    done();\n}\n"
    );
}

#[test]
fn allman_breaks_microsoft_try_header_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()])
        .expect("valid Allman style");

    assert_eq!(
        format_c(fixture!("void run() {", "    __try {"), &options),
        fixture!("void run()", "{", "    __try", "    {"),
    );
}

#[test]
fn java_attaches_microsoft_try_header_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid Java style");

    assert_eq!(
        format_c(fixture!("void run()", "{", "    __try", "    {"), &options,),
        fixture!("void run() {", "    __try {"),
    );
}

#[test]
fn gnu_else_block_uses_gnu_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void alpha() {\nif (ready) {\nrun();\n} else {\nstop();\n}\n}\n",
            &options,
        ),
        "void alpha()\n{\n    if (ready)\n        {\n            run();\n        }\n    else\n        {\n            stop();\n        }\n}\n"
    );
}

#[test]
fn whitesmith_do_while_tail_returns_to_do_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid Whitesmith style");

    assert_eq!(
        format_c(
            fixture!("void run() {", "do {", "work();", "} while (ready);", "}",),
            &options,
        ),
        fixture!(
            "void run()",
            "    {",
            "    do",
            "        {",
            "        work();",
            "        }",
            "    while (ready);",
            "    }",
        ),
    );
}

#[test]
fn vtk_do_while_tail_returns_to_do_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid VTK style");

    assert_eq!(
        format_c(
            fixture!("void run() {", "do {", "work();", "} while (ready);", "}",),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    do",
            "        {",
            "        work();",
            "        }",
            "    while (ready);",
            "}",
        ),
    );
}

#[test]
fn gnu_do_while_tail_returns_to_do_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void alpha() {\ndo {\nrun();\n} while (ready);\n}\n",
            &options,
        ),
        "void alpha()\n{\n    do\n        {\n            run();\n        }\n    while (ready);\n}\n"
    );
}

#[test]
fn gnu_style_indents_else_while_opening_brace_with_block_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void helper(int value)\n{\n    if (value == 0)\n        call();\n    else while (value != 0) {\n            other();\n        }\n}\n",
            &options,
        ),
        "void helper(int value)\n{\n    if (value == 0)\n        call();\n    else while (value != 0)\n            {\n                other();\n            }\n}\n"
    );
}

#[test]
fn ratliff_style_breaks_else_after_indented_closing_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void helper(void)\n{\n    if (ready) {\n        call();\n    } else {\n        stop();\n    }\n}\n",
            &options,
        ),
        "void helper(void) {\n    if (ready) {\n        call();\n        }\n    else {\n        stop();\n        }\n    }\n"
    );
}

#[test]
fn qt_foreach_body_indents_like_loop_header() {
    let source = "void f()\n{\n    foreach(Item item, items)\n        item.cancel();\n    Q_FOREACH(Item other, others)\n        other.cancel();\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn macro_like_loop_header_brace_attaches_under_one_true_brace() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--pad-comma",
        "--align-pointer=name",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\nvoid foo(void)\n{\n    item_t *node;\n    list_foreach(node, &list) {\n        call(node);\n    }\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn unbraced_header_with_nested_header_body_stays_split() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=kr",
        "--indent=tab",
        "--break-one-line-headers",
        "--add-braces",
        "--attach-return-type",
        "--attach-return-type-decl",
        "--pad-comma",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "void f()\n{\n\tif(list) {\n\t\tfor(i = 0; i < n; i++)\n\t\t\tif(!cmp(list[i], name)) {\n\t\t\t\tneeded = FALSE;\n\t\t\t\tbreak;\n\t\t\t}\n\n\t\tfreev(list);\n\t}\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn braceless_else_body_continuation_uses_body_indent_under_paren() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=kr",
        "--indent=tab",
        "--break-one-line-headers",
        "--add-braces",
        "--attach-return-type",
        "--attach-return-type-decl",
        "--pad-comma",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void helper()\n{\n\tplayer_map([&n](Item j) {\n\t\tif (j.alpha) {\n\t\t\tif (n==0) {\n\t\t\t\tformatted(buffer, length-1, \"%s\"\n\t\t\t\t          , j.name);\n\t\t\t\tn=1;\n\t\t\t} else\n\t\t\t\tformatted(buffer, length-1, \"/%s\"\n\t\t\t\t          , j.name);\n\t\t}\n\t});\n}\n",
            &options,
        ),
        "void helper()\n{\n\tplayer_map([&n](Item j) {\n\t\tif(j.alpha) {\n\t\t\tif(n==0) {\n\t\t\t\tformatted(buffer, length-1, \"%s\"\n\t\t\t\t          , j.name);\n\t\t\t\tn=1;\n\t\t\t} else\n\t\t\t\tformatted(buffer, length-1, \"/%s\"\n\t\t\t\t          , j.name);\n\t\t}\n\t});\n}\n",
    );
}

#[test]
fn else_switch_broken_brace_indents_under_else() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (a)\n    foo ();\n  else switch (x)\n    {\n    case 1:\n      bar ();\n      break;\n    default:\n      baz ();\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (a)\n        foo ();\n    else switch (x)\n        {\n        case 1:\n            bar ();\n            break;\n        default:\n            baz ();\n        }\n}\n",
    );
}

#[test]
fn else_for_broken_brace_indents_under_else() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (a)\n    foo ();\n  else for (;;)\n    {\n      bar ();\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (a)\n        foo ();\n    else for (;;)\n        {\n            bar ();\n        }\n}\n",
    );
}

#[test]
fn prefix_increment_after_braceless_while_body_returns_to_header_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n    while (i.key() != 5)\n        ++i;\n        ++i;\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    while (i.key() != 5)\n        ++i;\n    ++i;\n}\n",
    );
}
#[test]
fn else_after_commented_braceless_while_body_matches_if_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (children)\n    /* comment\n     */\n    while (ref > 1)\n      call(a,\n           b);\n  else\n    while (ref > 0)\n      call(a,\n           b);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (children)\n        /* comment\n         */\n        while (ref > 1)\n            call(a,\n                 b);\n    else\n        while (ref > 0)\n            call(a,\n                 b);\n}\n",
    );
}
#[test]
fn else_after_braceless_body_following_compound_literal_condition_matches_else_if() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (a)\n    n = 2;\n  else if (!equal (&colors[0], (&(Color) {\n    .x = 1\n  })))\n    n = 1;\n  else\n    n = 0;\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (a)\n        n = 2;\n    else if (!equal (&colors[0], (&(Color) {\n        .x = 1\n    })))\n    n = 1;\n    else\n        n = 0;\n}\n",
    );
}
#[test]
fn braceless_else_after_function_signature_comments_keeps_body_indent() {
    let source = "inline const X& f(const X& value,\n                  const X& domain = X(),\n                  const X& context = X())\n{\n    if ( value )\n        return value;\n    else\n        // comment\n        // comment\n        return g(value);\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn dereference_assignment_after_braceless_for_returns_to_loop_indent() {
    let source = "void f()\n{\n    for (i = input, o = output; *i != 0;)\n        *(o++) = (char)(*(i++));\n    *o = 0;\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn else_after_braceless_split_call_with_leading_comma_returns_to_if_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tif(alpha)\n\t\ttextmsg(one, two,\n\t\t        wrap(three)\n\t\t        , four);\n\telse {\n\t\tcall();\n\t}\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    if(alpha)\n        textmsg(one, two,\n                wrap(three)\n                , four);\n    else {\n        call();\n    }\n}\n",
    );
}

#[test]
fn braceless_else_comment_then_nested_if_keeps_else_body_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tif (alpha)\n\t\tone();\n\telse\n\t\t/* note */\n\t\tif (beta) {\n\t\t\tone();\n\t\t\ttwo();\n\t\t} else\n\t\t\tthree();\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    if (alpha)\n        one();\n    else\n        /* note */\n        if (beta) {\n            one();\n            two();\n        } else\n            three();\n}\n",
    );
}

#[test]
fn braceless_loop_macro_block_body_stays_structural() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tfor (i = 0; i < n; i++)\n\t\teach(item) {\n\t\t\tdo_one(item);\n\t\t\tdo_two(item);\n\t\t}\n\n\teach(item) {\n\t\tdo_one(item);\n\t\tdo_two(item);\n\t}\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    for (i = 0; i < n; i++)\n        each(item) {\n            do_one(item);\n            do_two(item);\n        }\n\n    each(item) {\n        do_one(item);\n        do_two(item);\n    }\n}\n",
    );
}

#[test]
fn nested_braceless_do_while_indents_each_closing_while() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("do do do x; while(a); while(b); while(c);\n", &options,),
        "do do do x;\n        while(a);\n    while(b);\nwhile(c);\n",
    );
}

#[test]
fn malformed_for_without_parens_does_not_open_header() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected =
        "for{ y{\n    {\n    #endif  case(\n        namespaceclass&#define X(x) \\if/* block */\n";

    assert_eq!(
        format_c(
            "for{ y{{ #endif  case(\nnamespaceclass&#define X(x) \\if/* block */\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn whitesmith_malformed_else_after_scope_line_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let first = format_c("x[*:\ny>|::else// line\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_else_after_colon_line_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let input = "&&default[*<=break[->return<=->value!do<=constexprvalueforItem>)struct\n\n(resultint%int]result==:else\n// linebreak\n\n[!=int|\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_close_else_word_clears_following_body_indent() {
    let options = FormatOptions::default();
    let input = "continue(  catch\t#endif#define X(x) \\\nforstruct\t-\n;\tbeta\t#if A   ]   [switch gammabeta   ::try\t]  do[  -if}:: }\telse1\nconstexpr\n?  continue/* block */\t-\nnamespace\n<=42x  !\n/* block */switch\t||)#define X(x) \\ /* block */switch,\n#if Atry\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_split_do_after_malformed_colon_owns_next_source_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let expected = b"+:\n    do\n        f";

    assert_eq!(
        format_bytes(b"+:do\nf", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn whitesmith_split_do_after_return_colon_uses_header_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let expected = b"return +:\n       do\n           ~";

    assert_eq!(
        format_bytes(b"return +:do\n~", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn malformed_header_bracket_operator_preserves_source_lines() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let source = "while\n]\n!\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn malformed_header_inside_unclosed_delimiter_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let first = format_c("({while\n]\n!\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn malformed_trailing_header_word_preserves_source_lines() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let source = "w while\n]\n!\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn else_if_condition_continuation_keeps_body_indent_after_source_indent() {
    assert_eq!(
        format_c(
            "int f(void) {\n  if (a)\n    call();\n  else if (unlikely(len > max - sep ||\n               cast(len + sep) > cast(max) / n))\n    return error();\n  else {\n    done();\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "int f(void) {\n    if (a)\n        call();\n    else if (unlikely(len > max - sep ||\n                      cast(len + sep) > cast(max) / n))\n        return error();\n    else {\n        done();\n    }\n}\n",
    );
}

#[test]
fn first_if_statement_in_attached_else_if_block_uses_body_indent() {
    assert_eq!(
        format_c(
            "int f(int c) {\n  if( c=='[' ){\n    if( value ) return 0;\n  }else if( c=='#' ){\n    if( (z[0]=='-' || z[0]=='+') && IsDigit(z[1]) ) z++;\n    if( !IsDigit(z[0]) ) return 0;\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "int f(int c) {\n    if( c=='[' ) {\n        if( value ) return 0;\n    } else if( c=='#' ) {\n        if( (z[0]=='-' || z[0]=='+') && IsDigit(z[1]) ) z++;\n        if( !IsDigit(z[0]) ) return 0;\n    }\n}\n",
    );
}

#[test]
fn gnu_configured_control_header_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=gnu".to_owned(),
            "--control-header=REPEAT".to_owned(),
        ],
    )
    .expect("valid options");
    let expected =
        "void run()\n{\n    REPEAT(alpha)\n        {\n            call();\n        }\n}\n";

    assert_eq!(
        format_c("void run(){REPEAT(alpha){call();}}\n", &options),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn lisp_breaks_before_control_header_after_statement() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=lisp".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void run(){call();if(alpha){first();}}\n", &options,),
        "void run() {\n    call();\n    if(alpha) {\n        first(); } }\n",
    );
}
