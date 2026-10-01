#![allow(clippy::field_reassign_with_default)]

use crate::config::{BraceStyle, FormatOptions, PointerAlign};
use crate::formatter::braces::classification::ExternCGuard;
use crate::formatter::braces::initializers::InlineArrayFrame;
use crate::formatter::engine::FormatEngine;
use crate::formatter::format;
use crate::formatter::lexer::{Token, tokenize};
use crate::formatter::state::BraceType;
use crate::formatter::state::frame::BraceSemanticKind;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::syntax::OperatorRole;

fn fixture(lines: &[&str]) -> String {
    lines.join("\n") + "\n"
}

#[test]
fn operator_path_can_read_token_indexed_roles_without_output_change() {
    let source = "x * f(1);\n";
    let tokens = tokenize(source);
    let star_index = tokens
        .iter()
        .position(|token| matches!(token, Token::Operator(operator) if operator == "*"))
        .expect("star token");
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert_eq!(
        formatter.operator_role_at(star_index),
        OperatorRole::BinaryOperator
    );
    assert_eq!(formatter.finish(), source);
}

#[test]
fn syntax_roles_do_not_change_plain_output() {
    let source = fixture(&["int value;", "int other;"]);
    let tokens = tokenize(&source);
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert_eq!(formatter.finish(), source);
}

#[test]
fn line_ready_pipeline_preserves_existing_output() {
    let source = fixture(&["int a;", "int b;"]);
    let tokens = tokenize(&source);
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert_eq!(formatter.finish(), source);
}

#[test]
fn records_previous_command_and_non_ws_chars() {
    let tokens = tokenize(&fixture(&["int x = 1;"]));
    let options = FormatOptions::default();
    let state = FormatEngine::new(&options)
        .format_into(&tokens)
        .layout
        .command_state;

    assert_eq!(state.previous_command_char, Some(';'));
    assert_eq!(state.previous_non_ws_char, Some(';'));
}

#[test]
fn records_current_headers_on_pre_brace_stack() {
    let tokens = tokenize(&fixture(&["if(x){while(y){y--;}}"]));
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert!(formatter.layout.command_state.current_header.is_none());
    assert!(
        formatter
            .layout
            .command_state
            .pre_brace_header_stack
            .is_empty()
    );
    assert_eq!(
        formatter.layout.command_state.previous_command_char,
        Some('}')
    );
}

#[test]
fn keeps_header_stack_through_nested_non_header_braces() {
    let tokens = tokenize(&fixture(&["extern \"C\"{if(x){{}while(y){y--;}}}"]));
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert!(
        formatter
            .layout
            .command_state
            .pre_brace_header_stack
            .is_empty()
    );
}

#[test]
fn extern_c_state_does_not_leak_past_statements() {
    let tokens = tokenize(&fixture(&[
        "extern int data;",
        "void f(){return;}",
        "extern \"C\"{int g();}",
    ]));
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert_eq!(
        formatter.layout.nesting.last_closed_brace_type,
        Some(BraceType::Extern)
    );
    assert!(!formatter.pending_extern);
}

#[test]
fn records_paren_brace_and_question_stacks() {
    let tokens = tokenize(&fixture(&["if((a ? b : c)){x=(y+z);}"]));
    let options = FormatOptions::default();
    let state = FormatEngine::new(&options)
        .format_into(&tokens)
        .layout
        .nesting;

    assert_eq!(state.paren_depth, 0);
    assert!(state.brace_header_stack.is_empty());
    assert_eq!(state.question_depth, 0);
    assert_eq!(state.last_closed_brace_header, Some("if".to_string()));
}

#[test]
fn format_pipeline_records_adjuster_observed_lines() {
    let tokens = tokenize(&fixture(&["int a;", "int b;"]));
    let options = FormatOptions::default();
    let state = FormatEngine::new(&options)
        .format_into(&tokens)
        .layout
        .run_in_state;

    assert_eq!(state.adjuster_observed_line_count, 2);
}

#[test]
fn preserves_preprocessor_macro_comments() {
    let source = fixture(&[
        "#if A",
        "#define VALUE(x) (x) // keep",
        "#endif",
        "int y=VALUE(1);",
    ]);
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format(&source, &options),
        fixture(&[
            "#if A",
            "#define VALUE(x) (x) // keep",
            "#endif",
            "int y = VALUE(1);",
        ])
    );
}

#[test]
fn restores_continuation_checkpoints_after_nested_scopes_and_semicolons() {
    let source = fixture(&["int f(){return sum(a[", "i],", "other(b,", "c));}"]);
    let tokens = tokenize(&source);
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert_eq!(formatter.layout.indentation.continuation_stack_depth(), 0);
    let expected = fixture(&[
        "int f() {",
        "    return sum(a[",
        "                   i],",
        "               other(b,",
        "                     c));",
        "}",
    ]);
    assert_eq!(
        formatter.output.join("\n"),
        expected.strip_suffix('\n').unwrap()
    );
}

#[test]
fn capture_only_lambda_uses_lambda_brace_frame() {
    let tokens = tokenize("auto value = condition ? [] { return 1;\n");
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert_eq!(
        formatter
            .layout
            .frame_stack
            .active_brace()
            .map(|frame| frame.semantic_kind),
        Some(BraceSemanticKind::Lambda)
    );
    assert!(formatter.layout.frame_stack.active_ternary().is_some());
}

#[test]
fn inline_open_brace_runs_keep_parallel_scope_depths() {
    let tokens = tokenize("int values[] = {{{");
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert_eq!(
        formatter.layout.nesting.brace_type_stack.len(),
        formatter.layout.frame_stack.brace_depth()
    );
}

#[test]
fn brace_exit_truncates_unclosed_ternary_frame() {
    let tokens = tokenize(&fixture(&["void f(){", "value ?", "}"]));
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert_eq!(formatter.layout.nesting.question_depth, 0);
    assert!(formatter.layout.frame_stack.active_ternary().is_none());
}

#[test]
fn brace_exit_truncates_unclosed_bracket_state() {
    let tokens = tokenize(&fixture(&["void f(){", "array[", "}"]));
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);

    assert!(formatter.layout.frame_stack.active_bracket().is_none());
}

#[test]
fn preprocessor_branch_snapshots_restore_formatter_contract_state() {
    let options = FormatOptions::default();
    let mut formatter = FormatEngine::new(&options);
    formatter.layout.line_state.operator_padding_disabled = true;
    formatter.layout.run_in_state.current_run_in_indent = Some(3);
    formatter.update_case_body_indent(LineKind::SwitchLabel, "case 1:");
    formatter.update_case_brace_unindent(LineKind::SwitchLabel, "case 1:");
    let expected_switch_case_layout = formatter.layout.switch_case_layout.clone();
    formatter.layout.in_class_base_clause = true;
    formatter.layout.split_class_export_pending_base = true;
    formatter.preprocessor.split_else.pending_body = true;
    formatter.preprocessor.split_else.after_line = true;
    formatter
        .layout
        .compound_literal
        .forced_break_depths
        .push(3);
    formatter.layout.compound_literal.arg_indent_spaces = Some(12);
    formatter.layout.compound_literal.arg_paren_depth = Some(2);
    formatter.header_paren.depth = Some(2);
    formatter.inline_array.initializer_designator_bracket_depth = 1;
    formatter.inline_array.frames.push(InlineArrayFrame {
        depth: 1,
        body_column: 2,
        brace_column: 3,
        output_line: 4,
        aggregate_assignment: true,
    });
    formatter.pending_extern = true;
    formatter.extern_c_guard = ExternCGuard::ExternC;

    let snapshot = formatter.branch_snapshot();
    formatter.layout.line_state.operator_padding_disabled = false;
    formatter.layout.run_in_state.current_run_in_indent = None;
    formatter.layout.switch_case_layout = Default::default();
    formatter.layout.in_class_base_clause = false;
    formatter.layout.split_class_export_pending_base = false;
    formatter.preprocessor.split_else.pending_body = true;
    formatter.preprocessor.split_else.after_line = false;
    formatter
        .layout
        .compound_literal
        .forced_break_depths
        .clear();
    formatter.layout.compound_literal.arg_indent_spaces = None;
    formatter.layout.compound_literal.arg_paren_depth = None;
    formatter.header_paren.depth = None;
    formatter.inline_array.initializer_designator_bracket_depth = 0;
    formatter.inline_array.frames.clear();
    formatter.pending_extern = false;
    formatter.extern_c_guard = ExternCGuard::Idle;

    formatter.restore_branch_snapshot(snapshot);

    assert!(formatter.layout.line_state.operator_padding_disabled);
    assert_eq!(formatter.layout.run_in_state.current_run_in_indent, Some(3));
    assert_eq!(
        formatter.layout.switch_case_layout,
        expected_switch_case_layout
    );
    assert!(formatter.layout.in_class_base_clause);
    assert!(formatter.layout.split_class_export_pending_base);
    assert!(formatter.preprocessor.split_else.pending_body);
    assert!(formatter.preprocessor.split_else.after_line);
    assert_eq!(
        formatter.layout.compound_literal.forced_break_depths,
        vec![3]
    );
    assert_eq!(
        formatter.layout.compound_literal.arg_indent_spaces,
        Some(12)
    );
    assert_eq!(formatter.layout.compound_literal.arg_paren_depth, Some(2));
    assert_eq!(formatter.header_paren.depth, Some(2));
    assert_eq!(
        formatter.inline_array.initializer_designator_bracket_depth,
        1
    );
    assert_eq!(
        formatter.inline_array.frames,
        vec![InlineArrayFrame {
            depth: 1,
            body_column: 2,
            brace_column: 3,
            output_line: 4,
            aggregate_assignment: true,
        }]
    );
    assert!(formatter.pending_extern);
    assert_eq!(formatter.extern_c_guard, ExternCGuard::ExternC);

    let snapshot = formatter.branch_snapshot();
    formatter.preprocessor.split_else.pending_body = false;
    formatter.preprocessor.split_else.after_line = false;
    formatter.restore_branch_snapshot(snapshot);
    assert!(!formatter.preprocessor.split_else.pending_body);
    assert!(!formatter.preprocessor.split_else.after_line);
}

#[test]
fn output_lines_record_their_source_tokens() {
    let source = fixture(&[
        "static int",
        "f(int a, char *b) {",
        "    if (a) { return g(a, b); }",
        "    /* note */ return 0;",
        "}",
    ]);
    let tokens = tokenize(&source);
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);
    let code = |text: &str| -> String {
        let tokens = tokenize(text);
        tokens
            .iter()
            .filter(|token| crate::formatter::structure::blocks::is_code_token(token))
            .map(crate::formatter::lexer::token_text)
            .collect()
    };

    for index in 0..formatter.output.len() {
        let line = &formatter.output[index];
        let span = formatter.output.line_tokens(index);
        if code(line).is_empty() {
            continue;
        }
        let span = span.unwrap_or_else(|| panic!("no tokens for {line:?}"));
        let spanned: String = tokens[span.first..=span.last]
            .iter()
            .filter(|token| crate::formatter::structure::blocks::is_code_token(token))
            .map(crate::formatter::lexer::token_text)
            .collect();
        assert_eq!(spanned, code(line), "line {index}");
    }
}

#[test]
fn comment_lines_map_to_the_line_opening_their_comment() {
    let source = fixture(&[
        "char *s = \"/*\";",
        "int x; /* trailing",
        "   continued */",
        "/*",
        "  plain text line",
        "",
        " * starred line",
        " */",
        "*p = 1;",
    ]);
    let tokens = tokenize(&source);
    let options = FormatOptions::default();
    let formatter = FormatEngine::new(&options).format_into(&tokens);
    let starts: Vec<usize> = (0..formatter.output.len())
        .map(|index| formatter.output.comment_start_index(index))
        .collect();

    assert_eq!(starts, [0, 1, 1, 3, 3, 5, 3, 3, 8]);
}
