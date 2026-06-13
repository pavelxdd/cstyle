#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, apply_command_line_args};

#[test]
fn unindents_case_brace_blocks_by_default() {
    let source = fixture!("int f(int x){switch(x){case 1:{return 1;}default:{return 0;}}}");
    let actual = format(source);

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "    {",
            "        return 1;",
            "    }",
            "    default:",
            "    {",
            "        return 0;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn default_label_attached_brace_closes_at_case_column() {
    let actual = format_c(
        fixture!(
            "int f(int value) {",
            "  switch (value) {",
            "    default: {",
            "      return value;",
            "    }",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int value) {",
            "    switch (value) {",
            "    default: {",
            "        return value;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn attached_case_brace_keeps_trailing_block_comment_gap() {
    let actual = format_c(
        fixture!(
            "int f(int value) {",
            "  switch (value) {",
            "    case 1: {  /* comment */",
            "      return value;",
            "    }",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int value) {",
            "    switch (value) {",
            "    case 1: {  /* comment */",
            "        return value;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn java_style_attaches_case_brace_before_trailing_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo()",
                "{   switch(req) {",
                "    case REQ_if:   // label",
                "    {   /* body",
                "         * more",
                "         */",
                "        c = c + j;",
                "        break;",
                "    }",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "",
            "void foo() {",
            "    switch(req) {",
            "    case REQ_if: { // label",
            "        /* body",
            "         * more",
            "         */",
            "        c = c + j;",
            "        break;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn multiline_condition_inside_case_brace_keeps_sibling_case_indent() {
    let actual = format_c(
        fixture!(
            "static int call(void) {",
            "  int rc = 0;",
            "  if( state<7 ) {",
            "    switch( state ) {",
            "    case 0: {",
            "      if( safe==0",
            "          && length(value)>=24",
            "          && match(value, expect)==0",
            "        ) {",
            "        state = 1;",
            "      } else {",
            "        state = 7;",
            "      }",
            "      break;",
            "    };",
            "",
            "    case 1: {",
            "      if( done ) {",
            "        state = 2;",
            "      } else {",
            "        state = 7;",
            "      }",
            "      break;",
            "    }",
            "",
            "    default: {",
            "      if( ready ) {",
            "        state = 3;",
            "      }",
            "      break;",
            "    }",
            "    }",
            "  }",
            "",
            "  return rc;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "static int call(void) {",
            "    int rc = 0;",
            "    if( state<7 ) {",
            "        switch( state ) {",
            "        case 0: {",
            "            if( safe==0",
            "                    && length(value)>=24",
            "                    && match(value, expect)==0",
            "              ) {",
            "                state = 1;",
            "            } else {",
            "                state = 7;",
            "            }",
            "            break;",
            "        };",
            "",
            "        case 1: {",
            "            if( done ) {",
            "                state = 2;",
            "            } else {",
            "                state = 7;",
            "            }",
            "            break;",
            "        }",
            "",
            "        default: {",
            "            if( ready ) {",
            "                state = 3;",
            "            }",
            "            break;",
            "        }",
            "        }",
            "    }",
            "",
            "    return rc;",
            "}",
        )
    );
}

#[test]
fn nested_else_closes_at_its_header_indent_inside_case_brace() {
    let actual = format_c(
        fixture!(
            "int f(int value) {",
            "  switch(value) {",
            "    case 1: {",
            "      if( value ) {",
            "        return 1;",
            "      } else {",
            "        return 2;",
            "      }",
            "    }",
            "  }",
            "  return 0;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int value) {",
            "    switch(value) {",
            "    case 1: {",
            "        if( value ) {",
            "            return 1;",
            "        } else {",
            "            return 2;",
            "        }",
            "    }",
            "    }",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn indent_cases_keeps_case_brace_blocks_indented() {
    let mut options = FormatOptions::default();
    options.indent_cases = true;
    let source = fixture!("int f(int x){switch(x){case 1:{return 1;}}}");
    let actual = format_with(source, &options);

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "        {",
            "            return 1;",
            "        }",
            "    }",
            "}",
        )
    );
}

#[test]
fn unindents_nested_switch_case_brace_blocks() {
    let source = fixture!("int f(int x,int y){switch(x){case 1:{switch(y){case 2:{return 2;}}}}}");
    let actual = format(source);

    assert_eq!(
        actual,
        fixture!(
            "int f(int x, int y)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "    {",
            "        switch (y)",
            "        {",
            "        case 2:",
            "        {",
            "            return 2;",
            "        }",
            "        }",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn nested_switch_inside_case_brace_keeps_case_body_column() {
    let actual = format_c(
        fixture!(
            "int f(int value, int other) {",
            "  if (value) {",
            "    switch (value) {",
            "      case 1: {",
            "        switch (other) {",
            "          case 2: {",
            "            return 2;",
            "          }",
            "        }",
            "        break;",
            "      }",
            "    }",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int value, int other) {",
            "    if (value) {",
            "        switch (value) {",
            "        case 1: {",
            "            switch (other) {",
            "            case 2: {",
            "                return 2;",
            "            }",
            "            }",
            "            break;",
            "        }",
            "        }",
            "    }",
            "}",
        )
    );
}

#[test]
fn split_case_label_comment_inside_case_block_keeps_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(int value)\n{\n\tswitch (state)\n\t{\n\tcase alpha:\n\t\t{   int result;\n\t\t\tswitch(value)\n\t\t\t{\n\t\t\tcase 0: case 2: default:   /* note */\n\t\t\t\tresult = 1;\n\t\t\t\tbreak;\n\t\t\t}\n\t\t}\n\t}\n}\n",
            &options,
        ),
        "void f(int value)\n{\n    switch (state)\n    {\n    case alpha:\n    {   int result;\n        switch(value)\n        {\n        case 0:\n        case 2:\n        default:   /* note */\n            result = 1;\n            break;\n        }\n    }\n    }\n}\n",
    );
}

#[test]
fn sibling_case_after_case_brace_keeps_switch_column() {
    let actual = format_c(
        fixture!(
            "int f(int value) {",
            "  switch (value) {",
            "    case 1: {",
            "      return 1;",
            "    }",
            "    case 2: {",
            "      return 2;",
            "    }",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int value) {",
            "    switch (value) {",
            "    case 1: {",
            "        return 1;",
            "    }",
            "    case 2: {",
            "        return 2;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn unindents_comments_between_case_label_and_case_brace() {
    let source = fixture!(
        "int f(int x){switch(x){case 1:",
        "// before brace",
        "{ return 1; }}}",
    );
    let actual = format(source);

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "// before brace",
            "    {",
            "        return 1;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn preprocessor_conditionals_preserve_pending_case_brace_unindent() {
    let source = fixture!(
        "int f(int x){switch(x){case 1:",
        "#if A",
        "{return 1;}",
        "#endif",
        "default:return 0;}}",
    );
    let actual = format(source);

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "#if A",
            "    {",
            "        return 1;",
            "    }",
            "#endif",
            "    default:",
            "        return 0;",
            "    }",
            "}",
        )
    );
}

#[test]
fn preprocessor_inside_case_brace_preserves_case_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()])
        .expect("valid Allman style");

    assert_eq!(
        format_c(
            fixture!(
                "void run() {",
                "    switch (mode()) {",
                "    case MODE_ALPHA: {",
                "        if (!disabled) {",
                "#if FEATURE_VERSION(2, 9, 0)",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    switch (mode())",
            "    {",
            "    case MODE_ALPHA:",
            "    {",
            "        if (!disabled)",
            "        {",
            "#if FEATURE_VERSION(2, 9, 0)",
        ),
    );
}

#[test]
fn preprocessor_between_case_label_and_brace_keeps_case_body_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (x) {",
                "    case 1:",
                "#line 1 \"x\"",
                "    {",
                "        a();",
                "    }",
                "    break;",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    switch (x) {",
            "    case 1:",
            "#line 1 \"x\"",
            "        {",
            "            a();",
            "        }",
            "        break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn attaches_case_brace_in_one_true_brace_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.indent_switches = true;
    options.pad_header = true;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(int x){switch(x){case 1: { y(); break; } default: return;}}",
            "void g(int x){",
            "switch(x){",
            "case 2:",
            "{",
            "y();",
            "break;",
            "}",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x)",
            "{",
            "    switch (x) {",
            "        case 1: {",
            "            y();",
            "            break;",
            "        }",
            "        default:",
            "            return;",
            "    }",
            "}",
            "void g(int x)",
            "{",
            "    switch (x) {",
            "        case 2: {",
            "            y();",
            "            break;",
            "        }",
            "    }",
            "}",
        )
    );
}

#[test]
fn default_style_preserves_case_label_brace_placement() {
    let actual = format_c(
        fixture!(
            "void f(int x) {",
            "    switch (x) {",
            "    case 1: {",
            "        a();",
            "        break;",
            "    }",
            "    case 2:",
            "    {",
            "        b();",
            "        break;",
            "    }",
            "    }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x) {",
            "    switch (x) {",
            "    case 1: {",
            "        a();",
            "        break;",
            "    }",
            "    case 2:",
            "    {",
            "        b();",
            "        break;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn block_after_case_brace_switch_keeps_function_body_indent() {
    let actual = format_c(
        fixture!(
            "static int f(int value) {",
            "  switch (value) {",
            "    case ONE: {",
            "      one();",
            "      break;",
            "    }",
            "    case TWO: {",
            "      two();",
            "      break;",
            "    }",
            "    default: {",
            "      three();",
            "      return 1;",
            "    }",
            "  }",
            "  if (!ok) {",
            "    clear();",
            "    append();",
            "    return 0;",
            "  }",
            "  else if (bad)",
            "    return fail();",
            "  else {",
            "    add();",
            "  }",
            "  return 1;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "static int f(int value) {",
            "    switch (value) {",
            "    case ONE: {",
            "        one();",
            "        break;",
            "    }",
            "    case TWO: {",
            "        two();",
            "        break;",
            "    }",
            "    default: {",
            "        three();",
            "        return 1;",
            "    }",
            "    }",
            "    if (!ok) {",
            "        clear();",
            "        append();",
            "        return 0;",
            "    }",
            "    else if (bad)",
            "        return fail();",
            "    else {",
            "        add();",
            "    }",
            "    return 1;",
            "}",
        )
    );
}

#[test]
fn function_close_after_case_brace_switch_keeps_function_indent() {
    let actual = format_c(
        fixture!(
            "static const char *f(int c) {",
            "  switch( c ) {",
            "    case 1:",
            "      return \"one\";",
            "    case 'v': {",
            "      if( ok ) {",
            "        return \"short\";",
            "      } else {",
            "        return \"long\";",
            "      }",
            "    }",
            "  }",
            "  return \"\";",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "static const char *f(int c) {",
            "    switch( c ) {",
            "    case 1:",
            "        return \"one\";",
            "    case 'v': {",
            "        if( ok ) {",
            "            return \"short\";",
            "        } else {",
            "            return \"long\";",
            "        }",
            "    }",
            "    }",
            "    return \"\";",
            "}",
        )
    );
}

#[test]
fn block_comment_inside_case_brace_keeps_switch_body_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_switches = true;
    let actual = format_with(
        fixture!(
            "void f(void) {",
            "    switch (x) {",
            "        case 2: {",
            "            uint32_t target = read();",
            "            /* line one",
            "             * line two */",
            "            handle();",
            "            break;",
            "        }",
            "    }",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "    switch (x) {",
            "        case 2: {",
            "            uint32_t target = read();",
            "            /* line one",
            "             * line two */",
            "            handle();",
            "            break;",
            "        }",
            "    }",
            "}"
        )
    );
}

#[test]
fn braceless_if_in_case_block_keeps_body_indent() {
    let source = fixture!(
        "void f(int value){",
        "  switch(value){",
        "    case 1:{",
        "      if( a )",
        "        call();",
        "      break;",
        "    }",
        "  }",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "void f(int value) {",
            "    switch(value) {",
            "    case 1: {",
            "        if( a )",
            "            call();",
            "        break;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn do_block_in_case_block_keeps_body_indent() {
    let source = fixture!(
        "void f(int value){",
        "  switch(value){",
        "    case 1:{",
        "      do{",
        "        if( a )",
        "          call();",
        "      }while( b );",
        "      break;",
        "    }",
        "  }",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "void f(int value) {",
            "    switch(value) {",
            "    case 1: {",
            "        do {",
            "            if( a )",
            "                call();",
            "        } while( b );",
            "        break;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn multiline_for_block_in_case_keeps_body_indent() {
    let source = fixture!(
        "void f(int value){",
        "  switch(value){",
        "    case 1:{",
        "      for( int i = 0;",
        "           i < n;",
        "           i++ ) {",
        "        call(i);",
        "      }",
        "      break;",
        "    }",
        "  }",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "void f(int value) {",
            "    switch(value) {",
            "    case 1: {",
            "        for( int i = 0;",
            "                i < n;",
            "                i++ ) {",
            "            call(i);",
            "        }",
            "        break;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn comment_inside_case_if_block_keeps_following_body_indent() {
    let source = fixture!(
        "void f(int value){",
        "  switch(value){",
        "    case 1: {",
        "      if (first) {",
        "        /* comment */",
        "        call();",
        "      }",
        "      else {",
        "        done();",
        "      }",
        "      break;",
        "    }",
        "  }",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "void f(int value) {",
            "    switch(value) {",
            "    case 1: {",
            "        if (first) {",
            "            /* comment */",
            "            call();",
            "        }",
            "        else {",
            "            done();",
            "        }",
            "        break;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn fallthrough_comment_in_case_block_keeps_body_indent() {
    let source = fixture!(
        "int f(int value) {",
        "  switch (value) {",
        "    case 1: {",
        "      if (value)",
        "        return value;",
        "      /* else */",
        "    }  /* fallthrough */",
        "    default:",
        "      return 0;",
        "  }",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "int f(int value) {",
            "    switch (value) {",
            "    case 1: {",
            "        if (value)",
            "            return value;",
            "        /* else */",
            "        }  /* fallthrough */",
            "    default:",
            "        return 0;",
            "    }",
            "}",
        )
    );
}

#[test]
fn fallthrough_after_braced_case_block_uses_case_label_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f(int value)",
                "{",
                "    switch (value)",
                "    {",
                "        default:",
                "            {",
                "                call();",
                "            }",
                "            FALLTHROUGH();",
                "        case 1:",
                "            use();",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f(int value)",
            "{",
            "    switch (value)",
            "    {",
            "    default:",
            "    {",
            "        call();",
            "    }",
            "    FALLTHROUGH();",
            "    case 1:",
            "        use();",
            "    }",
            "}",
        )
    );
}

#[test]
fn braced_case_return_ternary_at_return_column_stays_at_return_column() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (x)",
                "    {",
                "        case outer:",
                "        {",
                "            switch (y)",
                "            {",
                "                case inner:",
                "                    return (value == 0)",
                "                    ? positive()",
                "                    : negative();",
                "            }",
                "        }",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    switch (x)",
            "    {",
            "    case outer:",
            "    {",
            "        switch (y)",
            "        {",
            "        case inner:",
            "            return (value == 0)",
            "            ? positive()",
            "            : negative();",
            "        }",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn braced_case_call_argument_ternary_question_keeps_source_column() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (x)",
                "    {",
                "        case a:",
                "        {",
                "            return call((half & 0x8000u) != 0",
                "                        ? neg(val)",
                "                        : pos(val), \"\");",
                "        }",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    switch (x)",
            "    {",
            "    case a:",
            "    {",
            "        return call((half & 0x8000u) != 0",
            "                    ? neg(val)",
            "                    : pos(val), \"\");",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn braced_case_logical_return_chain_keeps_continuation_column() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (x)",
                "    {",
                "        case a:",
                "        {",
                "            return read_number(Format::Binary, length) &&",
                "                   read_number(Format::Binary, subtype) &&",
                "                   read_bytes(Format::Binary, length, result) &&",
                "                   store_subtype(subtype);",
                "        }",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    switch (x)",
            "    {",
            "    case a:",
            "    {",
            "        return read_number(Format::Binary, length) &&",
            "               read_number(Format::Binary, subtype) &&",
            "               read_bytes(Format::Binary, length, result) &&",
            "               store_subtype(subtype);",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn block_comment_between_braced_case_and_next_case_uses_case_label_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (x)",
                "    {",
                "        case array:",
                "        {",
                "            g();",
                "            break;",
                "        }",
                "",
                "        /*",
                "        text",
                "        */",
                "        case string:",
                "            break;",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    switch (x)",
            "    {",
            "    case array:",
            "    {",
            "        g();",
            "        break;",
            "    }",
            "",
            "    /*",
            "    text",
            "    */",
            "    case string:",
            "        break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn block_comment_after_nested_block_keeps_closed_block_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "int f(int x)",
                "{",
                "\tswitch (x) {",
                "\tcase 1: {",
                "\t\tif (x) {",
                "\t\t\treturn 0;",
                "\t\t}",
                "",
                "\t\t/*",
                "\t\t * note",
                "\t\t */",
                "\t\treturn 1;",
                "\t}",
                "\t}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x) {",
            "    case 1: {",
            "        if (x) {",
            "            return 0;",
            "        }",
            "",
            "        /*",
            "         * note",
            "         */",
            "        return 1;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn else_after_multiline_if_inside_case_brace_aligns_to_if() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(int value)\n{\n\tswitch (cmd) {\n\tcase X: {\n\t\tif (value == A ||\n\t\t    value == B)\n\t\t\tresult = one();\n\t\telse\n\t\t\tresult = two();\n\t}\n\t}\n}\n",
            &options,
        ),
        "void f(int value)\n{\n    switch (cmd) {\n    case X: {\n        if (value == A ||\n            value == B)\n            result = one();\n        else\n            result = two();\n    }\n    }\n}\n",
    );
}
#[test]
fn case_block_followed_by_break_splits_break_to_case_body_indent() {
    assert_eq!(
        format_c(
            "int f(int value)\n{\n    switch (value) {\n    case 1: {\n        value++;\n    } break;\n    default:\n        return value;\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "int f(int value)\n{\n    switch (value) {\n    case 1: {\n        value++;\n    }\n    break;\n    default:\n        return value;\n    }\n}\n",
    );
}

#[test]
fn macro_call_arg_continuation_in_nested_case_brace_keeps_paren_alignment() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n    switch (x)\n    {\n    case A:\n    case B:\n      {\n        int v = g (a);\n        TRACE (channel, DETAIL,\n               \"item %s:\\ttarget %ld\\n\"\n               \"\\tsource:%u\\n\"\n               \"\\tcode number: %u (%s)\\n\",\n               (it->status == KEY_PRESSED) ? \"active\" : \"idle\",\n               ctx->value,\n               format_key_name (key_id));\n      }\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    switch (x)\n    {\n    case A:\n    case B:\n    {\n        int v = g (a);\n        TRACE (channel, DETAIL,\n               \"item %s:\\ttarget %ld\\n\"\n               \"\\tsource:%u\\n\"\n               \"\\tcode number: %u (%s)\\n\",\n               (it->status == KEY_PRESSED) ? \"active\" : \"idle\",\n               ctx->value,\n               format_key_name (key_id));\n    }\n    }\n}\n",
    );
}

#[test]
fn arg_after_ternary_colon_branch_in_case_keeps_paren_alignment() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n    switch (x)\n    {\n    case A:\n      {\n        int v = g (a);\n        event = make_event (cond == VALUE\n                            ? PRESS\n                            : RELEASE,\n                            surface,\n                            device,\n                            NULL);\n      }\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    switch (x)\n    {\n    case A:\n    {\n        int v = g (a);\n        event = make_event (cond == VALUE\n                            ? PRESS\n                            : RELEASE,\n                            surface,\n                            device,\n                            NULL);\n    }\n    }\n}\n",
    );
}

#[test]
fn paren_aligned_ternary_colon_branch_in_case_aligns_to_question() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n    switch (x)\n    {\n    case A:\n      {\n        if (cond)\n          {\n            a = b;\n          }\n        else\n          {\n            a = c;\n          }\n        event = make_event_record (it->status == KEY_PRESSED\n                                     ? KEY_EVENT_HIT\n                                     : KEY_EVENT_CLEAR,\n                                   channel,\n                                   source,\n                                   NULL);\n      }\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    switch (x)\n    {\n    case A:\n    {\n        if (cond)\n        {\n            a = b;\n        }\n        else\n        {\n            a = c;\n        }\n        event = make_event_record (it->status == KEY_PRESSED\n                                   ? KEY_EVENT_HIT\n                                   : KEY_EVENT_CLEAR,\n                                   channel,\n                                   source,\n                                   NULL);\n    }\n    }\n}\n",
    );
}

#[test]
fn horstmann_case_block_closing_brace_uses_block_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void alpha() {\nswitch (value) {\ncase 1: {\nrun();\nbreak;\n}\ndefault:\nstop();\n}\n}\n",
            &options,
        ),
        "void alpha()\n{   switch (value)\n    {   case 1:\n        {   run();\n            break;\n        }\n        default:\n            stop();\n    }\n}\n",
    );
}

#[test]
fn ratliff_case_block_closing_brace_uses_case_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void alpha() {\nswitch (value) {\ncase 1: {\nrun();\nbreak;\n}\ndefault:\nstop();\n}\n}\n",
            &options,
        ),
        "void alpha() {\n    switch (value) {\n        case 1: {\n            run();\n            break;\n            }\n        default:\n            stop();\n        }\n    }\n",
    );
}

#[test]
fn whitesmith_indents_case_body_block_brace_to_body_level() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void f(int x)\n{\nswitch(x)\n{\ncase 1:\n{\nint y=2;\nbreak;\n}\ndefault: bar();\n}\n}\n",
            &options,
        ),
        "void f(int x)\n    {\n    switch(x)\n        {\n        case 1:\n            {\n            int y=2;\n            break;\n            }\n        default:\n            bar();\n        }\n    }\n",
    );
}

#[test]
fn nested_case_label_inside_case_block_uses_the_nested_label_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 3:{\ncase 4:\ncall();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n    {\n    case 3:\n    {\n        case 4:\n            call();\n            break;\n        }\n    }\n}\n",
    );
}

#[test]
fn indent_cases_does_not_add_an_extra_level_to_a_nested_case_label() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=java".to_owned(), "--indent-cases".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 3:{\ncase 4:\ncall();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value) {\n    switch(value) {\n    case 3: {\n        case 4:\n            call();\n            break;\n        }\n    }\n}\n",
    );
}

#[test]
fn java_indent_cases_indents_the_complete_case_block() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=java".to_owned(), "--indent-cases".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:{\ncall();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value) {\n    switch(value) {\n    case 1: {\n            call();\n            break;\n        }\n    }\n}\n",
    );
}

#[test]
fn vtk_case_block_uses_the_case_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:{\ncall();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n        {\n        case 1:\n            {\n            call();\n            break;\n            }\n        }\n}\n",
    );
}

#[test]
fn gnu_indented_switch_case_block_uses_the_case_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=gnu".to_owned(), "--indent-switches".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:{\ncall();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n        {\n            case 1:\n            {\n                call();\n                break;\n            }\n        }\n}\n",
    );
}

#[test]
fn ratliff_indented_switch_case_block_closer_uses_the_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=ratliff".to_owned(), "--indent-switches".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:{\ncall();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value) {\n    switch(value) {\n        case 1: {\n            call();\n            break;\n            }\n        }\n    }\n",
    );
}

#[test]
fn whitesmith_repeated_case_blocks_keep_the_same_columns() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:{\ncall();\nbreak;\n}\ndefault:{\nother();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value)\n    {\n    switch(value)\n        {\n        case 1:\n            {\n            call();\n            break;\n            }\n        default:\n            {\n            other();\n            break;\n            }\n        }\n    }\n",
    );
}

#[test]
fn whitesmith_tab_indented_case_block_body_uses_structural_tabs() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=whitesmith".to_owned(),
            "--indent=tab=4".to_owned(),
            "--indent-cases".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:{\ncall();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value)\n\t{\n\tswitch(value)\n\t\t{\n\t\tcase 1:\n\t\t\t\t{\n\t\t\t\tcall();\n\t\t\t\tbreak;\n\t\t\t\t}\n\t\t}\n\t}\n",
    );
}

#[test]
fn horstmann_case_blocks_restore_the_switch_closer_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:{\ncall();\nbreak;\n}\ndefault:{\nother();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{   switch(value)\n    {   case 1:\n        {   call();\n            break;\n        }\n        default:\n        {   other();\n            break;\n        }\n    }\n}\n",
    );
}

#[test]
fn kr_broken_case_block_with_indent_cases_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent-cases".to_owned()],
    )
    .expect("valid options");
    let input = "void run(int value){\nswitch(value){\ncase 1:\n{\ncall();\nbreak;\n}\n}\n}\n";
    let expected = "void run(int value)\n{\n    switch(value) {\n    case 1: {\n            call();\n            break;\n        }\n    }\n}\n";
    let first = format_c(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), first);
}
