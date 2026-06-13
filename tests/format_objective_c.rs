#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format_c, format_with};
use cstyle::config::{FormatOptions, Mode, ObjCColonPad, PointerAlign, apply_command_line_args};

#[test]
fn preserves_multiline_objc_method_selector_alignment() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "@interface Item",
            "- (void)doThing:(int)value",
            "withName:(NSString *)name",
            "count:(NSUInteger)count;",
            "@end",
            "@implementation Item",
            "- (void)doThing:(int)value",
            "withName:(NSString *)name",
            "count:(NSUInteger)count",
            "{",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "- (void)doThing:(int)value",
            "    withName:(NSString *)name",
            "    count:(NSUInteger)count;",
            "@end",
            "@implementation Item",
            "- (void)doThing:(int)value",
            "    withName:(NSString *)name",
            "    count:(NSUInteger)count",
            "{",
            "}",
            "@end",
        )
    );
}

#[test]
fn objc_mode_prefers_message_alignment_after_cast() {
    let mut default_options = FormatOptions::default();
    default_options.align_method_colon = true;
    let mut objc_options = default_options.clone();
    objc_options.mode = Mode::ObjC;
    let source = fixture!(
        "void f(){",
        "value=(id)[object",
        "doThing:value",
        "withValue:other];",
        "}",
    );

    assert_eq!(
        format_with(source, &default_options),
        fixture!(
            "void f()",
            "{",
            "    value = (id)[object",
            "            doThing:value",
            "            withValue:other];",
            "}",
        )
    );
    assert_eq!(
        format_with(source, &objc_options),
        fixture!(
            "void f()",
            "{",
            "    value = (id)[object",
            "            doThing:value",
            "          withValue:other];",
            "}",
        )
    );
}

#[test]
fn pad_method_prefix_inserts_single_space_after_objc_prefix() {
    // INTENTIONAL DIVERGENCE: padding inserts a missing gap without collapsing
    // existing source whitespace.
    let mut options = FormatOptions::default();
    options.pad_method_prefix = true;
    let actual = format_c(
        fixture!(
            "@interface Item",
            "-(void)alpha:(int)value;",
            "+  (id)beta:(int)value;",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "- (void)alpha:(int)value;",
            "+  (id)beta:(int)value;",
            "@end",
        )
    );
}

#[test]
fn unpad_method_prefix_removes_space_after_objc_prefix() {
    let mut options = FormatOptions::default();
    options.unpad_method_prefix = true;
    let actual = format_c(
        fixture!(
            "@interface Item",
            "- (void)alpha:(int)value;",
            "+  (id)beta:(int)value;",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "-(void)alpha:(int)value;",
            "+(id)beta:(int)value;",
            "@end",
        )
    );
}

#[test]
fn pad_return_type_inserts_space_after_objc_return_type() {
    let mut options = FormatOptions::default();
    options.pad_return_type = true;
    let actual = format_c(
        fixture!(
            "@interface Item",
            "-(void)alpha:(int)value;",
            "+(id)beta;",
            "- (NSString *)gamma;",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "-(void) alpha:(int)value;",
            "+(id) beta;",
            "- (NSString *) gamma;",
            "@end",
        )
    );
}

#[test]
fn unpad_return_type_removes_space_after_objc_return_type() {
    let mut options = FormatOptions::default();
    options.unpad_return_type = true;
    let actual = format_c(
        fixture!(
            "@interface Item",
            "-(void) alpha:(int)value;",
            "+(id) beta;",
            "- (NSString *) gamma;",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "-(void)alpha:(int)value;",
            "+(id)beta;",
            "- (NSString *)gamma;",
            "@end",
        )
    );
}

#[test]
fn negated_cast_call_argument_is_not_objc_method() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  adjust (tree->parent_tree,\n          node,\n          0,\n          -(int) count,\n          -tree->offset);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    adjust (tree->parent_tree,\n            node,\n            0,\n            -(int) count,\n            -tree->offset);\n}\n",
    );
}

#[test]
fn pad_param_type_pads_around_objc_param_parens() {
    let mut options = FormatOptions::default();
    options.pad_param_type = true;
    let actual = format_c(
        fixture!(
            "@interface Item",
            "-(void)alpha:(int)value beta:(int)other;",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "-(void)alpha: (int) value beta: (int) other;",
            "@end",
        )
    );
}

#[test]
fn unpad_param_type_removes_space_around_objc_param_parens() {
    let mut options = FormatOptions::default();
    options.unpad_param_type = true;
    let actual = format_c(
        fixture!(
            "@interface Item",
            "-(void)alpha: (int) value beta: (int) other;",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "-(void)alpha:(int)value beta:(int)other;",
            "@end",
        )
    );
}

#[test]
fn preserves_objc_method_colon_spacing_by_default() {
    let options = FormatOptions::default();
    let source = fixture!("@interface Item", "-(void)alpha: (int)value;", "@end",);
    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_method_colon_all_pads_both_sides_of_objc_colons() {
    let mut options = FormatOptions::default();
    options.pad_method_colon = ObjCColonPad::All;
    let actual = format_c(
        fixture!(
            "@implementation Item",
            "-(void)alpha:(int)value beta:(int)other",
            "{",
            "    [self alpha:value beta:other];",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@implementation Item",
            "-(void)alpha : (int)value beta : (int)other",
            "{",
            "    [self alpha : value beta : other];",
            "}",
            "@end",
        )
    );
}

#[test]
fn pad_method_colon_after_pads_only_after_objc_colons() {
    let mut options = FormatOptions::default();
    options.pad_method_colon = ObjCColonPad::After;
    let actual = format_c(
        fixture!(
            "@implementation Item",
            "-(void)alpha:(int)value beta:(int)other",
            "{",
            "    [self alpha:value beta:other];",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@implementation Item",
            "-(void)alpha: (int)value beta: (int)other",
            "{",
            "    [self alpha: value beta: other];",
            "}",
            "@end",
        )
    );
}

#[test]
fn pad_method_colon_before_pads_only_before_objc_colons() {
    let mut options = FormatOptions::default();
    options.pad_method_colon = ObjCColonPad::Before;
    let actual = format_c(
        fixture!(
            "@implementation Item",
            "-(void)alpha:(int)value beta:(int)other",
            "{",
            "    [self alpha:value beta:other];",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@implementation Item",
            "-(void)alpha :(int)value beta :(int)other",
            "{",
            "    [self alpha :value beta :other];",
            "}",
            "@end",
        )
    );
}

#[test]
fn pad_method_colon_none_removes_objc_colon_spacing() {
    let mut options = FormatOptions::default();
    options.pad_method_colon = ObjCColonPad::None;
    let actual = format_c(
        fixture!(
            "@implementation Item",
            "-(void)alpha : (int)value beta : (int)other",
            "{",
            "    [self alpha : value beta : other];",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@implementation Item",
            "-(void)alpha:(int)value beta:(int)other",
            "{",
            "    [self alpha:value beta:other];",
            "}",
            "@end",
        )
    );
}

#[test]
fn pad_method_colon_skips_colon_before_close_paren() {
    let mut options = FormatOptions::default();
    options.pad_method_colon = ObjCColonPad::After;
    let actual = format_c(
        fixture!(
            "@implementation Item",
            "-(void)run",
            "{",
            "    SEL s = @selector(setValue : forKey : );",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@implementation Item",
            "-(void)run",
            "{",
            "    SEL s = @selector(setValue: forKey:);",
            "}",
            "@end",
        )
    );
}

#[test]
fn align_method_colon_aligns_objc_method_definition_colons() {
    let mut options = FormatOptions::default();
    options.align_method_colon = true;
    let actual = format_c(
        fixture!(
            "@implementation Item",
            "- (void)doThing:(int)value",
            "withName:(NSString *)name",
            "count:(NSUInteger)count",
            "{",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@implementation Item",
            "- (void)doThing:(int)value",
            "       withName:(NSString *)name",
            "          count:(NSUInteger)count",
            "{",
            "}",
            "@end",
        )
    );
}

#[test]
fn align_method_colon_uses_longest_continuation_selector() {
    let mut options = FormatOptions::default();
    options.align_method_colon = true;
    let actual = format_c(
        fixture!(
            "@implementation Item",
            "- (void)a:(int)value",
            "veryLongName:(int)other",
            "b:(int)third",
            "{",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@implementation Item",
            "- (void)a:(int)value",
            "    veryLongName:(int)other",
            "               b:(int)third",
            "{",
            "}",
            "@end",
        )
    );
}

#[test]
fn align_method_colon_aligns_interface_declarations() {
    let mut options = FormatOptions::default();
    options.align_method_colon = true;
    let actual = format_c(
        fixture!(
            "@interface Item",
            "- (void)doThing:(int)value",
            "withName:(NSString *)name",
            "count:(NSUInteger)count;",
            "- (void)single;",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "- (void)doThing:(int)value",
            "       withName:(NSString *)name",
            "          count:(NSUInteger)count;",
            "- (void)single;",
            "@end",
        )
    );
}

#[test]
fn align_method_colon_combines_with_colon_pad_after() {
    let mut options = FormatOptions::default();
    options.align_method_colon = true;
    options.pad_method_colon = ObjCColonPad::After;
    let actual = format_c(
        fixture!(
            "@implementation Item",
            "- (void)doThing:(int)value",
            "withName:(NSString *)name",
            "count:(NSUInteger)count",
            "{",
            "}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@implementation Item",
            "- (void)doThing: (int)value",
            "       withName: (NSString *)name",
            "          count: (NSUInteger)count",
            "{",
            "}",
            "@end",
        )
    );
}

#[test]
fn keeps_single_line_objc_collection_literals_inline() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "void f(void) {",
                "d = @{key: value};",
                "a = @[one, two];",
                "b = @{};",
                "c = @[];",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    d = @ {key: value};",
            "    a = @[one, two];",
            "    b = @ {};",
            "    c = @[];",
            "}"
        )
    );
}

#[test]
fn objc_block_literal_body_keeps_block_indent() {
    let source = fixture!(
        "void f(void) {",
        "    provider.block = ^NSArray *(Item *item) {",
        "        Value *value = [Value new];",
        "        [value run];",
        "        return @[ value ];",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn objc_array_literal_elements_align_to_first_element_column() {
    assert_eq!(
        format_c(
            fixture!("void f() {", "    id a = @[", "value,", "other", "];", "}"),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f() {",
            "    id a = @[",
            "               value,",
            "               other",
            "           ];",
            "}",
        )
    );
}

#[test]
fn aligns_multiline_objc_message_send_under_first_selector() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "void f(void) {",
                "[receiver alpha:a",
                "beta:b",
                "gamma:c];",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    [receiver alpha:a",
            "              beta:b",
            "              gamma:c];",
            "}"
        )
    );
}

#[test]
fn aligns_multiline_objc_message_send_after_assignment() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "void f(void) {",
                "result = [helper alpha:a",
                "beta:b];",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    result = [helper alpha:a",
            "                     beta:b];",
            "}"
        )
    );
}

#[test]
fn aligns_method_colon_across_multiline_objc_message_send() {
    let mut options = FormatOptions::default();
    options.align_method_colon = true;
    options.pad_method_colon = ObjCColonPad::NoChange;

    assert_eq!(
        format_c(
            fixture!(
                "void f(void) {",
                "[receiver alpha:a",
                "beta:b",
                "gamma:c];",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    [receiver alpha:a",
            "               beta:b",
            "              gamma:c];",
            "}"
        )
    );
}

#[test]
fn flushes_multiline_bare_key_dictionary_keys_as_labels() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "void f(void){",
                "d = @{",
                "akey : value,",
                "bkey : other,",
                "};",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    d = @ {",
            "akey :",
            "        value,",
            "bkey :",
            "        other,",
            "    };",
            "}"
        )
    );
}

#[test]
fn aligns_method_colon_when_receiver_is_alone_on_first_line() {
    let mut options = FormatOptions::default();
    options.align_method_colon = true;
    options.pad_method_colon = ObjCColonPad::NoChange;

    assert_eq!(
        format_c(
            fixture!("void f(void) {", "[receiver", "alpha:a", "beta:b];", "}"),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    [receiver",
            "     alpha:a",
            "      beta:b];",
            "}"
        )
    );
}

#[test]
fn keeps_nested_objc_collection_literals_inline() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "void f(void) {",
                "q = @[@{a: b}, @{c: d}];",
                "r = @{k: @[x, y]};",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    q = @[@ {a: b}, @ {c: d}];",
            "    r = @ {k: @[x, y]};",
            "}"
        )
    );
}

#[test]
fn padding_and_pointer_align_name_preserve_objc_methods_and_selectors() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "@interface Item",
            "- (void)doThing:(int)value withName:(NSString *)name;",
            "+ (Item *)itemWithValue:(int)value;",
            "@end",
            "@implementation Item",
            "- (void)doThing:(int)value withName:(NSString *)name{[self setValue:value forKey:name];}",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "- (void)doThing:(int)value withName:(NSString *)name;",
            "+ (Item *)itemWithValue:(int)value;",
            "@end",
            "@implementation Item",
            "- (void)doThing:(int)value withName:(NSString *)name",
            "{",
            "    [self setValue:value forKey:name];",
            "}",
            "@end",
        )
    );
}

#[test]
fn pad_operators_preserves_objc_method_nested_pointer_type_spacing() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c(
            fixture!(
                "- (void) method: (int)type",
                "    withName: (Type*   *)namePtr",
                "{ }",
            ),
            &options,
        ),
        fixture!(
            "- (void) method: (int)type",
            "    withName: (Type*   *)namePtr",
            "{ }",
        )
    );
}

#[test]
fn preserves_objc_method_prefix_spacing_from_source() {
    let options = FormatOptions::default();
    let actual = format_c(
        fixture!(
            "@interface Item",
            "-(void)doThing:(int)value;",
            "+(Item *)make;",
            "- (void)keepSpaced;",
            "@end",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface Item",
            "-(void)doThing:(int)value;",
            "+(Item *)make;",
            "- (void)keepSpaced;",
            "@end",
        )
    );
}

#[test]
fn objc_private_marker_stays_at_column_zero() {
    let source = fixture!(
        "@interface WidgetCell : BaseCell",
        "{",
        "@private",
        "    int value;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn objc_interface_inheritance_colon_gets_leading_space() {
    let actual = format_c(
        fixture!(
            "@interface View: Base",
            "@end",
            "@interface Handler: NSObject<Protocol>",
            "@end",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "@interface View : Base",
            "@end",
            "@interface Handler : NSObject<Protocol>",
            "@end",
        )
    );
}

#[test]
fn objc_dictionary_in_one_method_does_not_break_following_method_message() {
    // Dictionary layout ends at its statement and cannot affect the next method.
    let mut options = FormatOptions::default();
    options.mode = Mode::ObjC;
    let source = "+ (id)alpha {\n    return [Helper makeWithDomain:Domain\n                             code:code\n                         userInfo:@{ Key: [value stringByAppendingString:@\".\"] }];\n}\n+ (id)beta {\n    id result = source ?: [Builder buildWithFormat:@\"%@ %@\",\n                           source.domain, @(source.code)];\n}\n";
    let expected = "+ (id)alpha {\n    return [Helper makeWithDomain:Domain\n                   code:code\n                   userInfo:@ { Key: [value stringByAppendingString:@\".\"] }];\n}\n+ (id)beta {\n    id result = source ?: [Builder buildWithFormat:@\"%@ %@\",\n                                   source.domain, @(source.code)];\n}\n";

    assert_eq!(format_c(source, &options), expected);
}

#[test]
fn objc_message_selector_colons_align_across_all_rows_including_last() {
    let mut options = FormatOptions::default();
    options.mode = Mode::ObjC;
    options.align_method_colon = true;

    assert_eq!(
        format_c(
            "void f(void) {\n    [obj doSomethingWith:a\n        and:b\n        more:c];\n}\n",
            &options,
        ),
        "void f(void) {\n    [obj doSomethingWith:a\n                     and:b\n                    more:c];\n}\n",
    );
}

#[test]
fn objc_message_continuation_colon_keeps_single_space() {
    let source = "void f() {\n    m_textBuffer = [[MutableFormattedTextValue alloc]\n                    initWithBuffer: input.AsTextData()];\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn objc_message_dictionary_value_selector_colon_aligns_under_first_selector() {
    let mut options = FormatOptions::default();
    options.mode = Mode::ObjC;
    options.align_method_colon = true;

    assert_eq!(
        format_c(
            "void f(void) {\n    return [Err errorWithDomain:Domain\n            code:code\n            userInfo:@{ Key: [detail stringByAppendingString:@\".\"] }];\n}\n",
            &options,
        ),
        "void f(void) {\n    return [Err errorWithDomain:Domain\n                           code:code\n                       userInfo:@ { Key: [detail stringByAppendingString:@\".\"] }];\n}\n",
    );
}

#[test]
fn objc_message_continuation_aligns_under_selector_even_when_column_overflows() {
    // Wrapped message arguments align under their selector at every receiver column.
    let mut options = FormatOptions::default();
    options.mode = Mode::ObjC;

    assert_eq!(
        format_c(
            "void f(void) {\n    NSString *inner = err.localizedDescription ?: [NSString stringWithFormat:@\"%@ code %@\",\n                                                    err.domain, @(err.code)];\n}\n",
            &options,
        ),
        "void f(void) {\n    NSString *inner = err.localizedDescription ?: [NSString stringWithFormat:@\"%@ code %@\",\n                                                            err.domain, @(err.code)];\n}\n",
    );
}

#[test]
fn objc_dictionary_literal_breaks_value_rows_after_colons() {
    assert_eq!(
        format_c(
            "- (void)draw {\n    Dictionary *attributes =\n        @{ Name: [Color whiteColor],\n           Font: [Font systemFontOfSize:[Font systemFontSize]] };\n}\n",
            &FormatOptions::default(),
        ),
        "- (void)draw {\n    Dictionary *attributes =\n        @ { Name:\n            [Color whiteColor],\n            Font:\n            [Font systemFontOfSize:[Font systemFontSize]]\n          };\n}\n",
    );
}

#[test]
fn objc_dictionary_literal_after_assignment_keeps_at_brace_spacing() {
    assert_eq!(
        format_c(
            "void f(void) {\n    Dictionary *attributes =\n    @{ Name:\n       [Color whiteColor],\n       Font:\n       [Font systemFont]\n     };\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void) {\n    Dictionary *attributes =\n        @ { Name:\n            [Color whiteColor],\n            Font:\n            [Font systemFont]\n          };\n}\n",
    );
}

#[test]
fn objc_catch_keeps_space_after_try_closing_brace() {
    let source = fixture!(
        "void f(void)",
        "{",
        "    @try {",
        "        call();",
        "    } @catch (Exception *exception) {",
        "        recover();",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn objc_interface_multiline_doc_comment_keeps_member_indent() {
    assert_eq!(
        format_c(
            "@interface Item : Base\n\n/**\n * Picks a value.\n * @param block Completion block.\n */\n+ (void)pickWithBlock:(void (^)(Value *value))block;\n\n@end\n",
            &FormatOptions::default(),
        ),
        "@interface Item : Base\n\n    /**\n     * Picks a value.\n     * @param block Completion block.\n     */\n+ (void)pickWithBlock:(void (^)(Value *value))block;\n\n@end\n",
    );
}

#[test]
fn objc_interface_method_after_unavailable_decls_stays_at_member_indent() {
    let source = "@interface Item : Base\n\n- (instancetype)initWithFrame:(Rect)frame\n                        color:(Color *)color\n                         type:(Type)type\n                        block:(void (^)(Value))block;\n\n- (instancetype)initWithFrame:(Rect)frame NO_UNAVAILABLE;\n- (instancetype)initWithCoder:(Coder *)coder NO_UNAVAILABLE;\n\n/** Changes the value. */\n- (void)setValue:(Value *)value;\n\n@end\n";
    let expected = "@interface Item : Base\n\n- (instancetype)initWithFrame:(Rect)frame\n    color:(Color *)color\n    type:(Type)type\n    block:(void (^)(Value))block;\n\n- (instancetype)initWithFrame:(Rect)frame NO_UNAVAILABLE;\n- (instancetype)initWithCoder:(Coder *)coder NO_UNAVAILABLE;\n\n/** Changes the value. */\n- (void)setValue:(Value *)value;\n\n@end\n";

    assert_eq!(format_c(source, &FormatOptions::default()), expected,);
}

#[test]
fn objc_interface_property_doc_comment_keeps_member_indent() {
    assert_eq!(
        format_c(
            "@interface Item : Base\n\n/** Selected value. */\n@property(nonatomic) Value *value;\n\n@end\n",
            &FormatOptions::default(),
        ),
        "@interface Item : Base\n\n    /** Selected value. */\n@property(nonatomic) Value *value;\n\n@end\n",
    );
}

#[test]
fn objc_nested_message_argument_continuation_aligns_to_outer_selector() {
    let source = "- (id)initWithFrame:(Rect)frame\n                  color:(Color *)color\n                  type:(Type)type\n                  block:(void (^)(Value))block {\n    self = [super initWithFrame:frame\n                  value:[Builder valueForColor:color\n                  type:type]\n                  colorSpace:space\n                  block:block];\n}\n";
    let expected = "- (id)initWithFrame:(Rect)frame\n    color:(Color *)color\n    type:(Type)type\n    block:(void (^)(Value))block {\n    self = [super initWithFrame:frame\n                  value:[Builder valueForColor:color\n                   type:type]\n                  colorSpace:space\n                  block:block];\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), expected,);
}

#[test]
fn objc_nested_message_continuation_aligns_under_object() {
    assert_eq!(
        format_c(
            "- (id)init {\n    self = [self initWithColor:[Color colorWithSpace:space\n                                        hue:hue\n                                        alpha:alpha]];\n}\n",
            &FormatOptions::default(),
        ),
        "- (id)init {\n    self = [self initWithColor:[Color colorWithSpace:space\n                                hue:hue\n                                alpha:alpha]];\n}\n",
    );
}

#[test]
fn objc_message_selector_colon_at_line_end_keeps_body_indent() {
    assert_eq!(
        format_c(
            "- (void)text {\nreturn [obj foo:\n1];\n}\n",
            &FormatOptions::default(),
        ),
        "- (void)text {\n    return [obj foo:\n                1];\n}\n",
    );
}

#[test]
fn objc_string_format_continuation_aligns_under_selector_argument() {
    assert_eq!(
        format_c(
            "- (NSString *)text {\n    return [NSString stringWithFormat:\n            @\"%@ %@\",\n            [self first],\n            value ? [self second] : 0];\n}\n",
            &FormatOptions::default(),
        ),
        "- (NSString *)text {\n    return [NSString stringWithFormat:\n                     @\"%@ %@\",\n                     [self first],\n                     value ? [self second] : 0];\n}\n",
    );
}

#[test]
fn objc_autoreleasepool_preprocessor_split_keeps_nested_block_indent() {
    let source = "\nvoid f()\n{\n#if HAS_POOL\n    @autoreleasepool\n#else\n    Pool* pool = get_pool();\n#endif\n    {\n        for (Item* item in items)\n        {\n            use(item);\n        }\n    }\n#if !HAS_POOL\n    drain(pool);\n#endif\n    return done;\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn objc_method_empty_body_brace_stays_at_method_indent() {
    let source = "\n- (void)foo:(int)value\n{}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn objc_method_after_unclosed_bracket_does_not_indent_body_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--unpad-param-type".to_owned()])
        .expect("valid options");
    let source = "\nt[N:U\n  -((void))foo:(int)icon\n{}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn malformed_objc_prefix_before_brace_is_padded_and_broken() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--pad-method-prefix".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("\n-{void)Foo \n{}\n", &options),
        "\n- {\n    void)Foo\n    {}\n",
    );
}

#[test]
fn preprocessor_branch_restores_objective_c_message_state() {
    let mut options = FormatOptions::default();
    let args = ["--style=allman", "--pad-oper"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void f(){\n#if A\n[receiver method\n#else\narray[index\n#endif\n];\n}\n",
            &options,
        ),
        "void f()\n{\n#if A\n    [receiver method\n#else\n    array[index\n#endif\n         ];\n}\n",
    );
}

#[test]
fn objc_method_prefix_padding_preserves_existing_tab() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--mode=c".to_owned(), "--pad-method-prefix".to_owned()],
    )
    .expect("valid options");
    let source = "-(id)value;\n+\t(id)item;\n";
    let expected = "- (id)value;\n+\t(id)item;\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_colon_padding_survives_nested_message_opener() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--mode=c".to_owned(),
            "--pad-method-colon=all".to_owned(),
        ],
    )
    .expect("valid options");
    let source = "void run(){id value=[Builder buildWithAlpha:[Factory make:alpha]];}\n";
    let expected =
        "void run()\n{\n    id value=[Builder buildWithAlpha : [Factory make : alpha]];\n}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_pico_method_run_in_body_uses_indent_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=pico".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source =
        "- (id)buildWithAlpha:(id)alpha\nbeta:(id)beta\ngamma:(id)gamma\n{return alpha;}\n";
    let expected = "- (id)buildWithAlpha:(id)alpha\n    beta:(id)beta\n    gamma:(id)gamma\n{   return alpha;}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_multiline_method_whitesmith_brace_uses_first_selector_owner() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=whitesmith".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source =
        "- (id)buildWithAlpha:(id)alpha\nbeta:(id)beta\ngamma:(id)gamma\n{return alpha;}\n";
    let expected = "- (id)buildWithAlpha:(id)alpha\n    beta:(id)beta\n    gamma:(id)gamma\n    {\n    return alpha;\n    }\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

// INTENTIONAL DIVERGENCE: an unclosed message bracket cannot take ownership
// of the enclosing function's closing brace.
#[test]
fn malformed_objc_message_does_not_own_function_closer() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "void run(){id value=[Builder buildWithAlpha:alpha beta:beta;call();}\n";
    let expected =
        "void run()\n{\n    id value=[Builder buildWithAlpha:alpha beta:beta; call();\n}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

// INTENTIONAL DIVERGENCE: an incomplete selector type cannot turn an attached
// brace into a method body.
#[test]
fn malformed_objc_selector_does_not_own_attached_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "-(id)buildWithAlpha:(id)alpha beta:(id beta{return alpha;}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn objc_exception_headers_use_enclosing_whitesmith_body_owner() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=whitesmith".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "void run(){@try{call();}@catch(id error){handle(error);}@finally{finish();}}\n";
    let expected = "void run()\n    {\n    @try\n        {\n        call();\n        }\n    @catch(id error)\n        {\n        handle(error);\n        }\n    @finally\n        {\n        finish();\n        }\n    }\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

// INTENTIONAL DIVERGENCE: Objective-C exception headers follow the selected
// closing-header and block policy regardless of source whitespace around `@`.
#[test]
fn objc_exception_headers_follow_attached_style() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=java".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "void run(){@try{call();}@catch(id error){handle(error);}@finally{finish();}}\n";
    let expected = "void run() {\n    @try {\n        call();\n    } @catch(id error) {\n        handle(error);\n    } @finally {\n        finish();\n    }\n}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_multiline_method_uses_attached_style_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=java".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source =
        "- (id)buildWithAlpha:(id)alpha\nbeta:(id)beta\ngamma:(id)gamma\n{return alpha;}\n";
    let expected = "- (id)buildWithAlpha:(id)alpha\n    beta:(id)beta\n    gamma:(id)gamma {\n    return alpha;\n}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_commented_method_attaches_brace_before_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=java".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source =
        "-(id)buildWithAlpha:(id)alpha /* first */ beta:(id)beta // second\n{return alpha;}\n";
    let expected = "-(id)buildWithAlpha:(id)alpha /* first */ beta:(id)beta { // second\n    return alpha;\n}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

// Exact-column Objective-C alignment preserves the configured structural tab prefix.
#[test]
fn objc_aligned_selector_continuation_uses_configured_tab_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--mode=c".to_owned(),
            "--indent=tab=4".to_owned(),
            "--align-method-colon".to_owned(),
        ],
    )
    .expect("valid options");
    let source = "-(id)buildWithAlpha:(id)alpha\nbeta:(id)beta\n{return alpha;}\n";
    let expected =
        "-(id)buildWithAlpha:(id)alpha\n\t           beta:(id)beta\n{\n\treturn alpha;\n}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_interface_property_after_comment_stays_at_member_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source =
        "@interface Item\n/** Note.\n * Detail.\n */\n@property(nonatomic,strong)id value;\n@end\n";
    let expected = "@interface Item\n    /** Note.\n     * Detail.\n     */\n@property(nonatomic,strong)id value;\n@end\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_interface_comment_uses_configured_tab_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--mode=c".to_owned(),
            "--indent=tab=4".to_owned(),
        ],
    )
    .expect("valid options");
    let source =
        "@interface Item\n/** Note.\n * Detail.\n */\n@property(nonatomic)id value;\n@end\n";
    let expected =
        "@interface Item\n\t/** Note.\n\t * Detail.\n\t */\n@property(nonatomic)id value;\n@end\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_interface_stripped_comment_uses_configured_tab_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--mode=c".to_owned(),
            "--indent=tab=4".to_owned(),
            "--remove-comment-prefix".to_owned(),
        ],
    )
    .expect("valid options");
    let source =
        "@interface Item\n/** Note.\n * Detail.\n */\n@property(nonatomic)id value;\n@end\n";
    let expected =
        "@interface Item\n\t/** Note.\n\t    Detail.\n\t*/\n@property(nonatomic)id value;\n@end\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_interface_single_line_comment_uses_configured_tab_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--mode=c".to_owned(),
            "--indent=tab=4".to_owned(),
        ],
    )
    .expect("valid options");
    let source = "@interface Item\n/** Note. */\n@property(nonatomic)id value;\n@end\n";
    let expected = "@interface Item\n\t/** Note. */\n@property(nonatomic)id value;\n@end\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_interface_required_after_comment_stays_at_member_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "@interface Item\n/** Note.\n * Detail.\n */\n@required\n-(void)run;\n@end\n";
    let expected =
        "@interface Item\n    /** Note.\n     * Detail.\n     */\n@required\n-(void)run;\n@end\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_gnu_interface_method_stays_at_member_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=gnu".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "@interface Item:Base<ItemProtocol>\n@property(nonatomic,strong)id value;\n-(void)run;\n@end\n";
    let expected = "@interface Item : Base<ItemProtocol>\n@property(nonatomic,strong)id value;\n-(void)run;\n@end\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_whitesmith_interface_base_stays_on_header_row() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=whitesmith".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "@interface Item:Base<ItemProtocol>\n@end\n";
    let expected = "@interface Item : Base<ItemProtocol>\n@end\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn pico_objc_dictionary_literal_preserves_all_entries() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=pico".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "void run(){id value=@{@\"a\":first,@\"b\":second};}\n";
    let expected = "void run() {id value=@ {@\"a\":first,@\"b\":second };}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn whitesmith_colon_inside_ambiguous_brackets_stays_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=whitesmith".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "void run(){result=(Type)[object valueWithAlpha:alpha beta:beta];}\n";
    let expected =
        "void run()\n    {\n    result=(Type)[object valueWithAlpha:alpha beta:beta];\n    }\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_whitesmith_nested_message_keeps_inline_argument() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=whitesmith".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = "void run(){id value=[Builder buildWithAlpha:[Factory makeWithValue:alpha other:beta]\ngamma:gamma];}\n";
    let expected = "void run()\n    {\n    id value=[Builder buildWithAlpha:[Factory makeWithValue:alpha other:beta]\n                      gamma:gamma];\n    }\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn objc_aligned_padded_method_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--mode=c".to_owned(),
            "--pad-method-prefix".to_owned(),
            "--pad-return-type".to_owned(),
            "--pad-param-type".to_owned(),
            "--pad-method-colon=all".to_owned(),
            "--align-method-colon".to_owned(),
        ],
    )
    .expect("valid options");
    let source =
        "- (id)buildWithAlpha:(id)alpha\nbeta:(id)beta\ngamma:(id)gamma\n{return alpha;}\n";
    let expected = "- (id) buildWithAlpha : (id) alpha\n                 beta : (id) beta\n                gamma : (id) gamma\n{\n    return alpha;\n}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

// INTENTIONAL DIVERGENCE: Objective-C autorelease pools use the same block policy
// whether their opening brace had source whitespace or was attached to the keyword.
#[test]
fn objc_attached_autoreleasepool_is_a_command_block() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){@autoreleasepool{id value=alpha;call();}}\n",
            &options,
        ),
        "void run()\n{\n    @autoreleasepool\n    {\n        id value=alpha;\n        call();\n    }\n}\n",
    );
}
