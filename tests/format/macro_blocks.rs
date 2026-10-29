use crate::common::{format_exact, format_with};
use cstyle::config::{FormatOptions, apply_command_line_args};

#[test]
fn default_message_map_body_rows_are_indented() {
    assert_eq!(
        format_exact(
            fixture!(
                "BEGIN_MESSAGE_MAP( C, B )",
                "// open",
                "ON_EVENT()",
                "ON_ACTION( ID, OnAction )",
                "// close",
                "END_MESSAGE_MAP()",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "BEGIN_MESSAGE_MAP( C, B )",
            "// open",
            "    ON_EVENT()",
            "    ON_ACTION( ID, OnAction )",
            "// close",
            "END_MESSAGE_MAP()",
        )
    );
}

#[test]
fn default_wx_event_table_body_rows_are_indented() {
    let source = fixture!(
        "wxBEGIN_EVENT_TABLE(Foo, Bar)",
        "    EVT_SIZE(Foo::OnSize)",
        "#if X",
        "    EVT_HELP(ID, Foo::OnHelp)",
        "#endif",
        "wxEND_EVENT_TABLE()",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn wx_event_table_split_macro_argument_keeps_call_argument_column() {
    let source = fixture!(
        "wxBEGIN_EVENT_TABLE(Page, Base)",
        "    EVT_RANGE(ID_FIRST, ID_LAST,",
        "              Page::OnRange)",
        "wxEND_EVENT_TABLE()",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn macro_blocks_indent_configured_regions_and_preserve_markers() {
    let mut options = FormatOptions::default();
    options.macro_blocks = vec![
        ("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string()),
        ("BEGIN_GROUP".to_string(), "END_GROUP".to_string()),
    ];
    let actual = format_with(
        fixture!(
            "BEGIN_BLOCK(Frame, Base)",
            "BLOCK_ITEM(ID_MENU, Frame::HandleMenu)",
            "BLOCK_ITEM(ID_ABOUT, Frame::HandleAbout)",
            "END_BLOCK()",
            "void f(){",
            "BEGIN_GROUP(View, BaseView)",
            "GROUP_ITEM(Paint)",
            "END_GROUP()",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "BEGIN_BLOCK(Frame, Base)",
            "    BLOCK_ITEM(ID_MENU, Frame::HandleMenu)",
            "    BLOCK_ITEM(ID_ABOUT, Frame::HandleAbout)",
            "END_BLOCK()",
            "void f()",
            "{",
            "    BEGIN_GROUP(View, BaseView)",
            "        GROUP_ITEM(Paint)",
            "    END_GROUP()",
            "}",
        )
    );
}

#[test]
fn macro_blocks_recognize_bare_markers() {
    let mut options = FormatOptions::default();
    options.macro_blocks = vec![("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())];

    let actual = format_with(fixture!("BEGIN_BLOCK", "item();", "END_BLOCK"), &options);

    assert_eq!(actual, fixture!("BEGIN_BLOCK", "    item();", "END_BLOCK"));
}

#[test]
fn macro_blocks_keep_conditional_directives_at_marker_column() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    options.indent_preproc_block = true;
    options.macro_blocks = vec![("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())];
    let actual = format_with(
        fixture!(
            "BEGIN_BLOCK(Frame, Base)",
            "#if A",
            "BLOCK_ITEM(ID_MENU, Frame::HandleMenu)",
            "#endif",
            "BLOCK_ITEM(ID_ABOUT, Frame::HandleAbout)",
            "END_BLOCK()",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "BEGIN_BLOCK(Frame, Base)",
            "#if A",
            "    BLOCK_ITEM(ID_MENU, Frame::HandleMenu)",
            "#endif",
            "    BLOCK_ITEM(ID_ABOUT, Frame::HandleAbout)",
            "END_BLOCK()",
        )
    );
}

#[test]
fn macro_blocks_preserve_column_one_line_comments() {
    let mut options = FormatOptions::default();
    options.macro_blocks = vec![("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())];
    let actual = format_with(
        fixture!(
            "BEGIN_BLOCK(Frame, Base)",
            "//    BLOCK_ITEM(Frame::OnClose)",
            "    BLOCK_ITEM(Frame::OnPaint)",
            "    BLOCK_ITEM(ID_Open, Frame::OnOpen)",
            "//    BLOCK_ITEM(ID_Exit, Frame::OnExit)",
            "END_BLOCK()",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "BEGIN_BLOCK(Frame, Base)",
            "//    BLOCK_ITEM(Frame::OnClose)",
            "    BLOCK_ITEM(Frame::OnPaint)",
            "    BLOCK_ITEM(ID_Open, Frame::OnOpen)",
            "//    BLOCK_ITEM(ID_Exit, Frame::OnExit)",
            "END_BLOCK()",
        )
    );
}

#[test]
fn default_macro_block_define_indents_following_doc_comment() {
    assert_eq!(
        format_exact(
            "#define wxBEGIN_EVENT_TABLE(theClass, baseClass)\n\n/**\n    docs\n*/\n#define wxEND_EVENT_TABLE()\n",
            &FormatOptions::default(),
        ),
        "#define wxBEGIN_EVENT_TABLE(theClass, baseClass)\n\n    /**\n        docs\n    */\n#define wxEND_EVENT_TABLE()\n",
    );
}

#[test]
fn default_macro_block_define_indents_continued_body_lines() {
    assert_eq!(
        format_exact(
            "#define wxBEGIN_EVENT_TABLE(theClass, baseClass) \\\n    const int value = 1; \\\n    const int other = 2;\n#define wxEND_EVENT_TABLE()\n",
            &FormatOptions::default(),
        ),
        "#define wxBEGIN_EVENT_TABLE(theClass, baseClass) \\\n        const int value = 1; \\\n        const int other = 2;\n#define wxEND_EVENT_TABLE()\n",
    );
}

#[test]
fn tab_indentation_indents_macro_block_rows_with_tabs_and_aligns_their_arguments_in_spaces() {
    // A row of a macro block stands at the block's level, which tab
    // indentation writes as a tab; an argument split from its row aligns
    // past that tab in spaces.
    for (args, source, expected) in [
        (
            &["--indent=tab=4"][..],
            "BEGIN_MESSAGE_MAP(Item, Base)\n    ON_COMMAND(ID_FIRST, OnFirst)\n    ON_RANGE(ID_FIRST, ID_LAST,\n             OnRange)\nEND_MESSAGE_MAP()\n",
            "BEGIN_MESSAGE_MAP(Item, Base)\n\tON_COMMAND(ID_FIRST, OnFirst)\n\tON_RANGE(ID_FIRST, ID_LAST,\n\t         OnRange)\nEND_MESSAGE_MAP()\n",
        ),
        (
            &["--indent=tab=4", "--indent-namespaces"][..],
            "namespace Config\n{\nBEGIN_MESSAGE_MAP(Item, Base)\n    ON_COMMAND(ID_FIRST, OnFirst)\nEND_MESSAGE_MAP()\n}\n",
            "namespace Config\n{\n\tBEGIN_MESSAGE_MAP(Item, Base)\n\t\tON_COMMAND(ID_FIRST, OnFirst)\n\tEND_MESSAGE_MAP()\n}\n",
        ),
        (
            &["--indent=tab=4"][..],
            "wxBEGIN_EVENT_TABLE(Item, Base)\n    EVT_SIZE(Item::OnSize)\n#if X\n    EVT_HELP(ID_HELP, Item::OnHelp)\n#endif\nwxEND_EVENT_TABLE()\n",
            "wxBEGIN_EVENT_TABLE(Item, Base)\n\tEVT_SIZE(Item::OnSize)\n#if X\n\tEVT_HELP(ID_HELP, Item::OnHelp)\n#endif\nwxEND_EVENT_TABLE()\n",
        ),
    ] {
        let mut options = FormatOptions::default();
        let args = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        apply_command_line_args(&mut options, &args).expect("valid options");
        assert_eq!(format_exact(source, &options), expected);
        assert_eq!(format_exact(expected, &options), expected);
    }
}
