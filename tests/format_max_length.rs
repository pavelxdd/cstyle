#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format_c, format_with};
use cstyle::config::{
    BraceStyle, FormatOptions, IndentStyle, MinConditionalIndent, apply_command_line_args,
};

fn assert_stable_max_length_format(source: &str, options: &FormatOptions, expected: &str) {
    let actual = format_c(source, options);
    assert_eq!(actual, expected);
    assert_eq!(format_c(&actual, options), actual);
}

fn non_whitespace(source: &str) -> String {
    source.chars().filter(|ch| !ch.is_whitespace()).collect()
}

#[test]
fn max_code_length_splits_control_condition_when_padding_pushes_trailing_comment_near_width() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    if (line[i] == '}')                     // comment in column 45",
            "    {",
            "        call();",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (line[i] ==",
            "            '}')                     // comment in column 45",
            "    {",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn max_code_length_splits_function_call_before_trailing_comment_when_code_reaches_width() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    object->call(first_value, SECOND_VALUE_KIND); //comment",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    object->call(first_value,",
            "                 SECOND_VALUE_KIND); //comment",
            "}",
        )
    );
}

#[test]
fn max_code_length_keeps_trailing_comment_with_split_statement() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let actual = format_c(
        "void f(){ return is_generic_item(resource); } // comment text\n",
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f() {",
            "    return is_generic_item(",
            "               resource);    // comment text",
            "}",
        )
    );
}

#[test]
fn max_code_length_does_not_split_code_for_trailing_line_comment_text_overflow() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let source = fixture!(
        "void f()",
        "{",
        "    if (x == 1) // this is a long, long, long, long, long comment",
        "        call();",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn max_code_length_splits_string_macro_comparison_at_operator() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    if (value == TEXT(\"long string literal value for wrapping\"))",
            "        call();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (value ==",
            "            TEXT(\"long string literal value for wrapping\"))",
            "        call();",
            "}",
        )
    );
}

#[test]
fn max_code_length_pad_operators_splits_string_concat_before_plus() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    options.pad_operators = true;
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    if (debug)",
            "        GenericLogger.debug(\"Time to compute generic values: \"+(t2-t1));",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (debug)",
            "        GenericLogger.debug(\"Time to compute generic values: \"",
            "                            + (t2 - t1));",
            "}",
        )
    );
}

#[test]
fn max_code_length_splits_concatenated_string_macro_calls_at_plus() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    TRACE(TEXT(\"first value=\")+name+TEXT(\", second value=\")+args+TEXT(\", done\"));",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    TRACE(TEXT(\"first value=\")+name",
            "          +TEXT(\", second value=\")+args+TEXT(\", done\"));",
            "}",
        )
    );
}

#[test]
fn max_code_length_keeps_nested_string_macro_call_intact() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(80);
    let source = fixture!(
        "void f()",
        "{",
        "    CHECK(is_valid(value),",
        "          TEXT(\"long string literal value for formatter checks and wrapping across the width\") );",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn max_code_length_uses_previous_comma_when_next_comma_exceeds_width() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    size_t result = convert(input, count, encoding_mode, first_block, output);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    size_t result = convert(input, count,",
            "                            encoding_mode, first_block, output);",
            "}",
        )
    );
}

#[test]
fn max_code_length_breaks_before_alternative_logical_operator_words() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    if (alpha == beta or gamma == delta or epsilon == zeta)",
            "        call();",
            "    if (alpha == beta and gamma == delta and epsilon == zeta)",
            "        call();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (alpha == beta or gamma == delta",
            "            or epsilon == zeta)",
            "        call();",
            "    if (alpha == beta and gamma == delta",
            "            and epsilon == zeta)",
            "        call();",
            "}",
        )
    );
}

#[test]
fn max_code_length_splits_logical_condition_before_operators() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);

    assert_eq!(
        format_c(
            fixture!(
                "void foo()",
                "{",
                "    if (thisVariable1 == thatVariable1 || thisVariable2 == thatVariable2 || thisVariable3 == thatVariable3)",
            ),
            &options,
        ),
        fixture!(
            "void foo()",
            "{",
            "    if (thisVariable1 == thatVariable1",
            "            || thisVariable2 == thatVariable2",
            "            || thisVariable3 == thatVariable3)",
        )
    );
}

#[test]
fn max_code_length_break_after_logical_splits_after_operators() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    options.break_after_logical = true;

    assert_eq!(
        format_c(
            fixture!(
                "void foo()",
                "{",
                "    if (thisVariable1 == thatVariable1 || thisVariable2 == thatVariable2 || thisVariable3 == thatVariable3)",
            ),
            &options,
        ),
        fixture!(
            "void foo()",
            "{",
            "    if (thisVariable1 == thatVariable1 ||",
            "            thisVariable2 == thatVariable2 ||",
            "            thisVariable3 == thatVariable3)",
        )
    );
}

#[test]
fn max_code_length_splits_after_unpadded_comparison_operator() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);

    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    if(data->get_kind()->get_other_kind()==TreeData::File)",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    if(data->get_kind()->get_other_kind()==",
            "            TreeData::File)",
        )
    );
}

#[test]
fn max_code_length_splits_before_long_string_literal() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);

    assert_eq!(
        format_c(
            fixture!(
                "void foo()",
                "{",
                "    if (thisVariable1 == \"a very long, long, long, long, long, quote\")",
            ),
            &options,
        ),
        fixture!(
            "void foo()",
            "{",
            "    if (thisVariable1 ==",
            "            \"a very long, long, long, long, long, quote\")",
        )
    );
}

#[test]
fn max_code_length_keeps_string_macro_call_intact_after_semicolon_split() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let actual = format_c(
        fixture!(
            "void f() {",
            "    switch (kind) {",
            "    case Alpha: value = TEXT(\"long string literal value for formatter\"); break;",
            "    default: value = TEXT(\"long string literal value for formatter\"); break;",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f() {",
            "    switch (kind) {",
            "    case Alpha:",
            "        value = TEXT(\"long string literal value for formatter\");",
            "        break;",
            "    default:",
            "        value = TEXT(\"long string literal value for formatter\");",
            "        break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn max_code_length_skips_comments_arrays_asm_and_preprocessor() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(24);
    let actual = format_with(
        fixture!(
            "#define LONG a,b,c,d",
            "int a[]={1,2,3,4};",
            "void f(){// a,b,c,d,e",
            "asm(\"a,b,c,d\");}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#define LONG a,b,c,d",
            "int a[] = {1, 2, 3, 4};",
            "void f() // a,b,c,d,e",
            "{",
            "    asm(\"a,b,c,d\");",
            "}",
        )
    );
}

#[test]
fn max_code_length_aligns_logical_chain_after_return() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(109);
    options.pad_operators = true;
    options.pad_commas = true;
    options.break_after_logical = true;
    let actual = format_with(
        fixture!(
            "int f(){return compare_text(record.value, \"alpha\") == 0 || compare_text(record.value, \"beta\") == 0 || compare_text(record.value, \"gamma\") == 0;}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    return compare_text(record.value, \"alpha\") == 0 || compare_text(record.value, \"beta\") == 0 ||",
            "           compare_text(record.value, \"gamma\") == 0;",
            "}",
        )
    );
}

#[test]
fn break_after_logical_prefers_logical_split_points() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(32);
    options.break_after_logical = true;
    let actual = format_with(
        fixture!("int f(){return alpha&&beta&&gamma&&delta;}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    return alpha && beta && gamma &&",
            "           delta;",
            "}",
        )
    );
}

#[test]
fn max_code_length_generated_return_arithmetic_uses_return_anchor() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "int run(){return alphaValue+betaValue+gammaValue+deltaValue+epsilonValue+zetaValue+etaValue;}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "int run()",
            "{",
            "    return alphaValue+betaValue+gammaValue+deltaValue",
            "           +epsilonValue+zetaValue+etaValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_generated_assignment_logical_uses_value_anchor() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.break_after_logical = true;
    let source = fixture!(
        "bool run(){resultValue=alphaValue==betaValue||gammaValue!=deltaValue||epsilonValue>=zetaValue;}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "bool run()",
            "{",
            "    resultValue=alphaValue==betaValue||",
            "                gammaValue!=deltaValue||epsilonValue>=zetaValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_alternative_logical_tokens_keep_return_anchor() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "bool run(){return alphaCondition and betaCondition or gammaCondition and deltaCondition or epsilonCondition;}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "bool run()",
            "{",
            "    return alphaCondition and betaCondition",
            "           or gammaCondition and deltaCondition",
            "           or epsilonCondition;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_indent_after_parens_uses_condition_level() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.indent_after_parens = true;
    options.continuation_indent = 2;
    let source = fixture!(
        "void run(){if(alphaCondition&&betaCondition&&gammaCondition&&deltaCondition&&epsilonCondition){call();}}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    if(alphaCondition&&betaCondition&&gammaCondition",
            "            &&deltaCondition&&epsilonCondition)",
            "    {",
            "        call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_indent_after_parens_uses_declaration_level() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.indent_after_parens = true;
    options.continuation_indent = 2;
    let source = fixture!(
        "ResultType calculateResult(AlphaType alphaValue,BetaType betaValue,GammaType gammaValue,DeltaType deltaValue,EpsilonType epsilonValue);"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "ResultType calculateResult(AlphaType alphaValue,",
            "        BetaType betaValue,GammaType gammaValue,",
            "        DeltaType deltaValue,EpsilonType epsilonValue);",
        ),
    );
}

#[test]
fn max_code_length_nested_call_restores_outer_delimiter_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(79);
    let source = fixture!(
        "void run(){resultValue=outerFunction(alphaValue,innerFunction(betaValue,gammaValue,deltaValue),epsilonValue,zetaValue);}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=outerFunction(alphaValue,innerFunction(betaValue,gammaValue,",
            "                              deltaValue),epsilonValue,zetaValue);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_moves_padded_operator_when_it_exceeds_limit() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    let source = fixture!(
        "void run(){resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue+zetaValue+etaValue+thetaValue;}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue = alphaValue + betaValue + gammaValue",
            "                  + deltaValue + epsilonValue + zetaValue + etaValue",
            "                  + thetaValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_split_call_uses_assignment_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=builderObject.withAlpha(alphaValue).withBeta(betaValue).withGamma(gammaValue).withDelta(deltaValue).finish();}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=builderObject.withAlpha(",
            "                    alphaValue).withBeta(betaValue).withGamma(",
            "                    gammaValue).withDelta(deltaValue).finish();",
            "}",
        ),
    );
}

#[test]
fn max_code_length_long_call_arguments_use_delimiter_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=calculateResultWithAnIntentionallyLongNeutralName(alphaValueOne,betaValueTwo,gammaValueThree,deltaValueFour,epsilonValueFive,zetaValueSix,etaValueSeven,thetaValueEight,iotaValueNine,kappaValueTen,lambdaValueEleven,muValueTwelve);}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=",
            "        calculateResultWithAnIntentionallyLongNeutralName(",
            "            alphaValueOne,betaValueTwo,gammaValueThree,",
            "            deltaValueFour,epsilonValueFive,zetaValueSix,",
            "            etaValueSeven,thetaValueEight,iotaValueNine,",
            "            kappaValueTen,lambdaValueEleven,muValueTwelve);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_template_assignment_keeps_type_id_intact() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "using ResultType=WrapperType<AlphaType,BetaType,GammaType,DeltaType,EpsilonType,ZetaType,EtaType,ThetaType>;"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "using ResultType=",
            "    WrapperType<AlphaType,BetaType,GammaType,DeltaType,EpsilonType,ZetaType,EtaType,ThetaType>;",
        ),
    );
}

#[test]
fn max_code_length_bitwise_splits_at_outer_operators() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "unsigned run(){return(alphaValue<<firstShift)|(betaValue<<secondShift)|(gammaValue<<thirdShift)|(deltaValue<<fourthShift);}"
    );

    // Every fitting outer bitwise operator uses the same split-side rule.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "unsigned run()",
            "{",
            "    return(alphaValue<<firstShift)|",
            "          (betaValue<<secondShift)|(gammaValue<<thirdShift)|",
            "          (deltaValue<<fourthShift);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_keeps_stream_chains_unsplit() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){outputStream<<alphaValue<<betaValue<<gammaValue<<deltaValue<<epsilonValue<<zetaValue<<etaValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    outputStream<<alphaValue<<betaValue<<gammaValue<<deltaValue<<epsilonValue<<zetaValue<<etaValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_splits_function_signature_at_opening_paren() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);

    assert_eq!(
        format_c(
            fixture!(
                "ItemNode* ItemNode::replace_child (ItemNode* old_item, const ItemNode& new_item)",
                "{",
            ),
            &options,
        ),
        fixture!(
            "ItemNode* ItemNode::replace_child (",
            "    ItemNode* old_item, const ItemNode& new_item)",
            "{",
        )
    );
}

#[test]
fn max_code_length_splits_each_function_parameter() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);

    assert_eq!(
        format_c(
            fixture!(
                "void Processor::format_element_tokens(ElementTokenKind element_kind, bool opening_element)",
                "{",
            ),
            &options,
        ),
        fixture!(
            "void Processor::format_element_tokens(",
            "    ElementTokenKind element_kind,",
            "    bool opening_element)",
            "{",
        )
    );
}

#[test]
fn max_code_length_splits_pointer_parameters_at_declarators() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "ResultType calculateResult(const AlphaType *alphaValue,const BetaType *betaValue,const GammaType *gammaValue,const DeltaType *deltaValue);",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "ResultType calculateResult(const AlphaType",
            "                           *alphaValue,const BetaType *betaValue,",
            "                           const GammaType *gammaValue,",
            "                           const DeltaType *deltaValue);",
        ),
    );
}

#[test]
fn max_code_length_nested_new_uses_outer_call_fallback() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=new ContainerType(new AlphaType(alphaValue,betaValue),new BetaType(gammaValue,deltaValue),epsilonValue);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=new ContainerType(new AlphaType(",
            "                                      alphaValue,betaValue),new BetaType(gammaValue,",
            "                                              deltaValue),epsilonValue);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_skips_empty_chain_call_boundary() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(120);
    let source = fixture!(
        "void run(){resultValue=builderObject.withAlpha(alphaValue).withBeta(betaValue).withGamma(gammaValue).withDelta(deltaValue).finish();}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=builderObject.withAlpha(alphaValue).withBeta(betaValue).withGamma(gammaValue).withDelta(",
            "                    deltaValue).finish();",
            "}",
        ),
    );
}

#[test]
fn max_code_length_over_max_call_uses_assignment_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(79);
    let source = fixture!(
        "void run(){resultValue=calculateResultWithAnIntentionallyLongNeutralName(alphaValueOne,betaValueTwo,gammaValueThree,deltaValueFour,epsilonValueFive,zetaValueSix,etaValueSeven,thetaValueEight,iotaValueNine,kappaValueTen,lambdaValueEleven,muValueTwelve);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=calculateResultWithAnIntentionallyLongNeutralName(alphaValueOne,",
            "                betaValueTwo,gammaValueThree,deltaValueFour,epsilonValueFive,zetaValueSix,",
            "                etaValueSeven,thetaValueEight,iotaValueNine,kappaValueTen,lambdaValueEleven,",
            "                muValueTwelve);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_break_after_word_logical_chain_is_stable() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.break_after_logical = true;
    let source = fixture!(
        "bool run(){return alphaCondition and betaCondition or gammaCondition and deltaCondition or epsilonCondition;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "bool run()",
            "{",
            "    return alphaCondition and betaCondition or",
            "           gammaCondition and deltaCondition or",
            "           epsilonCondition;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_padded_stream_uses_stream_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    let source = fixture!(
        "void run(){outputStream<<alphaValue<<betaValue<<gammaValue<<deltaValue<<epsilonValue<<zetaValue<<etaValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    outputStream << alphaValue << betaValue <<",
            "                 gammaValue << deltaValue << epsilonValue <<",
            "                 zetaValue << etaValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_for_condition_keeps_generated_minimum_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(60);
    options.break_after_logical = true;
    let source = fixture!(
        "void run(){for(int indexValue=initialValue;indexValue<maximumValue&&readyCondition;++indexValue){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    for(int indexValue=initialValue; indexValue<maximumValue&&",
            "            readyCondition; ++indexValue)",
            "    {",
            "        call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_padded_ternary_keeps_assignment_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    let source = fixture!(
        "int run(){resultValue=alphaCondition&&betaCondition?firstLongResultValue+secondLongResultValue:thirdLongResultValue+fourthLongResultValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "int run()",
            "{",
            "    resultValue = alphaCondition",
            "                  && betaCondition ? firstLongResultValue +",
            "                  secondLongResultValue : thirdLongResultValue +",
            "                  fourthLongResultValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_member_call_honors_zero_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 0;
    let source = fixture!(
        "void run(){resultValue=builderObject.withAlpha(alphaValue).withBeta(betaValue).withGamma(gammaValue).withDelta(deltaValue).finish();}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=builderObject.withAlpha(",
            "                alphaValue).withBeta(betaValue).withGamma(",
            "                gammaValue).withDelta(deltaValue).finish();",
            "}",
        ),
    );
}

#[test]
fn max_code_length_member_call_honors_four_continuation_levels() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 4;
    let source = fixture!(
        "void run(){resultValue=builderObject.withAlpha(alphaValue).withBeta(betaValue).withGamma(gammaValue).withDelta(deltaValue).finish();}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=builderObject.withAlpha(",
            "                                alphaValue).withBeta(betaValue).withGamma(",
            "                                gammaValue).withDelta(deltaValue).finish();",
            "}",
        ),
    );
}

#[test]
fn max_code_length_nested_new_honors_zero_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 0;
    let source = fixture!(
        "void run(){resultValue=new ContainerType(new AlphaType(alphaValue,betaValue),new BetaType(gammaValue,deltaValue),epsilonValue);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=new ContainerType(new AlphaType(",
            "                                  alphaValue,betaValue),new BetaType(gammaValue,",
            "                                          deltaValue),epsilonValue);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_nested_new_clamps_four_continuation_levels() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 4;
    let source = fixture!(
        "void run(){resultValue=new ContainerType(new AlphaType(alphaValue,betaValue),new BetaType(gammaValue,deltaValue),epsilonValue);}",
    );

    // Every nested-new row retains the same clamped continuation owner.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=new ContainerType(new AlphaType(",
            "            alphaValue,betaValue),new BetaType(gammaValue,",
            "                    deltaValue),epsilonValue);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_nested_new_honors_indent_after_parens() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 2;
    options.indent_after_parens = true;
    let source = fixture!(
        "void run(){resultValue=new ContainerType(new AlphaType(alphaValue,betaValue),new BetaType(gammaValue,deltaValue),epsilonValue);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=new ContainerType(new AlphaType(",
            "                    alphaValue,betaValue),new BetaType(gammaValue,",
            "                    deltaValue),epsilonValue);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_split_long_call_honors_zero_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 0;
    let source = fixture!(
        "void run(){resultValue=calculateResultWithAnIntentionallyLongNeutralName(alphaValueOne,betaValueTwo,gammaValueThree,deltaValueFour,epsilonValueFive,zetaValueSix,etaValueSeven,thetaValueEight,iotaValueNine,kappaValueTen,lambdaValueEleven,muValueTwelve);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=",
            "    calculateResultWithAnIntentionallyLongNeutralName(",
            "    alphaValueOne,betaValueTwo,gammaValueThree,",
            "    deltaValueFour,epsilonValueFive,zetaValueSix,",
            "    etaValueSeven,thetaValueEight,iotaValueNine,",
            "    kappaValueTen,lambdaValueEleven,muValueTwelve);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_split_long_call_honors_four_continuation_levels() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 4;
    let source = fixture!(
        "void run(){resultValue=calculateResultWithAnIntentionallyLongNeutralName(alphaValueOne,betaValueTwo,gammaValueThree,deltaValueFour,epsilonValueFive,zetaValueSix,etaValueSeven,thetaValueEight,iotaValueNine,kappaValueTen,lambdaValueEleven,muValueTwelve);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=",
            "                    calculateResultWithAnIntentionallyLongNeutralName(",
            "                                    alphaValueOne,betaValueTwo,gammaValueThree,",
            "                                    deltaValueFour,epsilonValueFive,zetaValueSix,",
            "                                    etaValueSeven,thetaValueEight,iotaValueNine,",
            "                                    kappaValueTen,lambdaValueEleven,muValueTwelve);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_split_long_call_honors_indent_after_parens() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 2;
    options.indent_after_parens = true;
    let source = fixture!(
        "void run(){resultValue=calculateResultWithAnIntentionallyLongNeutralName(alphaValueOne,betaValueTwo,gammaValueThree,deltaValueFour,epsilonValueFive,zetaValueSix,etaValueSeven,thetaValueEight,iotaValueNine,kappaValueTen,lambdaValueEleven,muValueTwelve);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=",
            "            calculateResultWithAnIntentionallyLongNeutralName(",
            "                    alphaValueOne,betaValueTwo,gammaValueThree,",
            "                    deltaValueFour,epsilonValueFive,zetaValueSix,",
            "                    etaValueSeven,thetaValueEight,iotaValueNine,",
            "                    kappaValueTen,lambdaValueEleven,muValueTwelve);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_nested_new_comma_restores_outer_call_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(60);
    let source = fixture!(
        "void run(){resultValue=new ContainerType(new AlphaType(alphaValue,betaValue),new BetaType(gammaValue,deltaValue),epsilonValue);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=new ContainerType(new AlphaType(alphaValue,",
            "                                  betaValue),new BetaType(gammaValue,deltaValue),",
            "                                  epsilonValue);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_for_condition_honors_indent_after_parens_on_replay() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.continuation_indent = 2;
    options.indent_after_parens = true;
    let source = fixture!(
        "void run(){for(int indexValue=initialValue;indexValue<maximumValue&&readyCondition;++indexValue){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    for(int indexValue=initialValue;",
            "            indexValue<maximumValue",
            "            &&readyCondition; ++indexValue)",
            "    {",
            "        call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_gnu_assignment_owner_is_stable() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue+zetaValue+etaValue+thetaValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    resultValue=alphaValue+betaValue+gammaValue",
            "                +deltaValue+epsilonValue+zetaValue+etaValue",
            "                +thetaValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_linux_condition_honors_indent_after_parens_on_replay() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.min_conditional_indent = MinConditionalIndent::OneHalf;
    options.max_code_length = Some(50);
    options.continuation_indent = 2;
    options.indent_after_parens = true;
    let source = fixture!(
        "void run(){if(alphaCondition&&betaCondition&&gammaCondition&&deltaCondition&&epsilonCondition){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    if(alphaCondition&&betaCondition&&gammaCondition",
            "            &&deltaCondition&&epsilonCondition) {",
            "        call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_whitesmith_ternary_keeps_colon_and_false_arm_together() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.max_code_length = Some(50);
    let source = fixture!(
        "int run(){resultValue=alphaCondition&&betaCondition?firstLongResultValue+secondLongResultValue:thirdLongResultValue+fourthLongResultValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "int run()",
            "    {",
            "    resultValue=alphaCondition",
            "                &&betaCondition?firstLongResultValue",
            "                +secondLongResultValue:thirdLongResultValue",
            "                +fourthLongResultValue;",
            "    }",
        ),
    );
}

#[test]
fn max_code_length_whitesmith_padded_stream_keeps_stream_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.max_code_length = Some(50);
    options.pad_operators = true;
    let source = fixture!(
        "void run(){outputStream<<alphaValue<<betaValue<<gammaValue<<deltaValue<<epsilonValue<<zetaValue<<etaValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "    {",
            "    outputStream << alphaValue << betaValue <<",
            "                 gammaValue << deltaValue << epsilonValue <<",
            "                 zetaValue << etaValue;",
            "    }",
        ),
    );
}

#[test]
fn max_code_length_break_after_logical_uses_previous_fitting_operator() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(51);
    options.break_after_logical = true;
    let source = fixture!(
        "void run(){while(alphaCondition||betaCondition||gammaCondition||deltaCondition){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    while(alphaCondition||betaCondition||",
            "            gammaCondition||deltaCondition)",
            "    {",
            "        call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_gnu_bitwise_return_keeps_return_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.max_code_length = Some(80);
    let source = fixture!(
        "unsigned run(){return(alphaValue<<firstShift)|(betaValue<<secondShift)|(gammaValue<<thirdShift)|(deltaValue<<fourthShift);}",
    );

    // Fitting bitwise operators use one placement rule without losing return ownership.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "unsigned run()",
            "{",
            "    return(alphaValue<<firstShift)|(betaValue<<secondShift)|(gammaValue<<thirdShift)",
            "          |(deltaValue<<fourthShift);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_objc_message_keeps_outer_selector_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "- (id)value { return [Builder buildWithAlpha:alphaValue beta:betaValue gamma:gammaValue delta:deltaValue epsilon:epsilonValue]; }",
    );

    // Objective-C selector names and colons remain one lexical unit.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "- (id)value",
            "{",
            "  return [Builder buildWithAlpha:alphaValue beta:",
            "                  betaValue gamma:gammaValue delta:deltaValue",
            "                  epsilon:epsilonValue];",
            "}",
        ),
    );
}

#[test]
fn max_code_length_keeps_fitting_lambda_parameter_opener_on_head() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run() {",
        "resultValue=outerFunction(alphaValue,[](int firstValue,int secondValue) {",
        "return firstValue+secondValue;",
        "},betaValue,gammaValue);",
        "}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run() {",
            "  resultValue=outerFunction(alphaValue,[](",
            "  int firstValue,int secondValue) {",
            "    return firstValue+secondValue;",
            "  },betaValue,gammaValue);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_constructor_keeps_fitting_lambda_parameter_opener_on_head() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "Item::Item():memberValue(calculate(alphaValue,[](int value) {",
        "return value+offsetValue;",
        "},betaValue,gammaValue)) {}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "Item::Item():memberValue(calculate(alphaValue,[](",
            "                                       int value)",
            "{",
            "  return value+offsetValue;",
            "},betaValue,gammaValue)) {}",
        ),
    );

    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "Item::Item():memberValue(calculate(alphaValue,[](",
            "\t                                       int value)",
            "{",
            "\treturn value+offsetValue;",
            "},betaValue,gammaValue)) {}",
        ),
    );
}

#[test]
fn max_code_length_tabbed_header_keeps_delimiter_column_on_replay() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.attach_struct = true;
    options.attach_enum = true;
    options.min_conditional_indent = MinConditionalIndent::OneHalf;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){",
        "\tif(alphaCondition&&betaCondition&&gammaCondition&&deltaCondition&&epsilonCondition){call();}",
        "}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "\tif(alphaCondition&&betaCondition&&gammaCondition",
            "\t   &&deltaCondition&&epsilonCondition) {",
            "\t\tcall();",
            "\t}",
            "}",
        ),
    );
}

#[test]
fn max_code_length_inline_access_member_keeps_class_body_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Lisp;
    options.break_one_line_statements = false;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "class Item{public:ResultType calculateResult(AlphaType alphaValue,BetaType betaValue,GammaType gammaValue,DeltaType deltaValue);};",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "class Item {",
            "public:ResultType calculateResult(",
            "    AlphaType alphaValue,BetaType betaValue,",
            "    GammaType gammaValue,DeltaType deltaValue); };",
        ),
    );
}

#[test]
fn max_code_length_inline_case_statement_keeps_case_body_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Lisp;
    options.break_one_line_statements = false;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(int value){switch(value){case 1:resultValue=calculateResult(alphaValue,betaValue,gammaValue,deltaValue,epsilonValue);break;default:break;}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run(int value) {",
            "  switch(value) {",
            "  case 1:resultValue=calculateResult(alphaValue,",
            "                                       betaValue,gammaValue,deltaValue,epsilonValue);",
            "    break;",
            "  default:break; } }",
        ),
    );
}

#[test]
fn max_code_length_ratliff_lambda_closer_uses_body_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Ratliff;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=outerFunction(alphaValue,[](int firstValue,int secondValue){return firstValue+secondValue;},betaValue,gammaValue);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run() {",
            "  resultValue=outerFunction(alphaValue,[](",
            "  int firstValue,int secondValue) {",
            "    return firstValue+secondValue;",
            "    },betaValue,gammaValue);",
            "  }",
        ),
    );
}

#[test]
fn max_code_length_attached_constructor_lambda_body_uses_semantic_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    options.max_code_length = Some(50);
    let source = fixture!(
        "Item::Item():memberValue(calculate(alphaValue,[](int value){return value+offsetValue;},betaValue,gammaValue)){}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "Item::Item():memberValue(calculate(alphaValue,[](",
            "\t                                       int value) {",
            "\treturn value+offsetValue;",
            "},betaValue,gammaValue)) {}",
        ),
    );

    options.indent_style = IndentStyle::Spaces;
    options.indent_width = 4;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pad_parens_outside = true;
    // A split boundary is valid when the emitted head fits after separator trimming.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "Item::Item() : memberValue (calculate (alphaValue,",
            "                                       [] (int value) {",
            "    return value + offsetValue;",
            "}, betaValue, gammaValue) ) {}",
        ),
    );
}

#[test]
fn max_code_length_vtk_constructor_lambda_body_uses_brace_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Vtk;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "Item::Item():memberValue(calculate(alphaValue,[](int value){return value+offsetValue;},betaValue,gammaValue)){}",
    );

    // VTK lambda bodies use the brace column at every structural depth.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "Item::Item():memberValue(calculate(alphaValue,[](",
            "                                       int value)",
            "{",
            "return value+offsetValue;",
            "},betaValue,gammaValue)) {}",
        ),
    );
}

#[test]
fn max_code_length_constructor_lambda_tail_keeps_call_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "Item::Item():memberValue(calculate(alphaValue,[](int value){return value+offsetValue;},betaValue,gammaValue)){}",
    );

    // Arguments after an inline lambda remain owned by the enclosing call.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "Item::Item():memberValue(calculate(alphaValue,[](",
            "                                       int value) {return value+offsetValue;},betaValue,",
            "                                   gammaValue)) {}",
        ),
    );
}

#[test]
fn max_code_length_horstmann_objc_message_does_not_split_at_bracket() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Horstmann;
    options.max_code_length = Some(50);
    let source = fixture!(
        "- (id)value { return [Builder buildWithAlpha:alphaValue beta:betaValue gamma:gammaValue delta:deltaValue epsilon:epsilonValue]; }",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "- (id)value",
            "{   return [Builder buildWithAlpha:alphaValue",
            "                    beta:betaValue gamma:gammaValue delta:deltaValue",
            "                    epsilon:epsilonValue];",
            "}",
        ),
    );
}

#[test]
fn max_code_length_horstmann_tab_run_in_counts_emitted_prefix() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Horstmann;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    options.max_code_length = Some(50);
    let source = fixture!(
        "- (id)value { return [Builder buildWithAlpha:alphaValue beta:betaValue gamma:gammaValue delta:deltaValue epsilon:epsilonValue]; }",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "- (id)value",
            "{\treturn [Builder buildWithAlpha:alphaValue beta:",
            "\t                betaValue gamma:gammaValue delta:deltaValue",
            "\t                epsilon:epsilonValue];",
            "}",
        ),
    );
}

#[test]
fn max_code_length_pico_tab_run_in_after_header_comment_is_stable() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 8;
    options.tab_width = 8;
    options.max_code_length = Some(80);
    options.break_after_logical = true;
    let source = fixture!(
        "void run(){// an intentionally long column-one comment is not a code split candidate",
        "resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue;",
        "}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run() // an intentionally long column-one comment is not a code split candidate",
            "{\tresultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue; }",
        ),
    );
}

#[test]
fn max_code_length_vtk_objc_method_keeps_definition_brace_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Vtk;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "- (id)value { return [Builder buildWithAlpha:alphaValue beta:betaValue gamma:gammaValue delta:deltaValue epsilon:epsilonValue]; }",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "- (id)value",
            "{",
            "  return [Builder buildWithAlpha:alphaValue beta:",
            "                  betaValue gamma:gammaValue delta:deltaValue",
            "                  epsilon:epsilonValue];",
            "}",
        ),
    );
}

#[test]
fn max_code_length_preprocessor_condition_keeps_live_logical_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){if(alphaCondition&&",
        "#if ENABLED",
        "betaCondition&&gammaCondition&&deltaCondition&&epsilonCondition",
        "#endif",
        "zetaCondition){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "  if(alphaCondition&&",
            "#if ENABLED",
            "      betaCondition&&gammaCondition&&deltaCondition",
            "      &&epsilonCondition",
            "#endif",
            "      zetaCondition)",
            "  {",
            "    call();",
            "  }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_one_true_brace_condition_keeps_current_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){if(alphaCondition&&",
        "#if ENABLED",
        "betaCondition&&gammaCondition&&deltaCondition&&epsilonCondition",
        "#endif",
        "zetaCondition){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "  if(alphaCondition&&",
            "#if ENABLED",
            "      betaCondition&&gammaCondition&&deltaCondition",
            "      &&epsilonCondition",
            "#endif",
            "      zetaCondition) {",
            "    call();",
            "  }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_add_braces_keeps_interrupted_condition_ownership() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.add_braces = true;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){if(alphaCondition&&",
        "#if ENABLED",
        "betaCondition&&gammaCondition&&deltaCondition&&epsilonCondition",
        "#endif",
        "zetaCondition){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "  if(alphaCondition&&",
            "#if ENABLED",
            "      betaCondition&&gammaCondition&&deltaCondition",
            "      &&epsilonCondition",
            "#endif",
            "      zetaCondition) {",
            "    call();",
            "  }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_interrupted_condition_uses_configured_preprocessor_tabs() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    options.max_code_length = Some(80);
    options.break_after_logical = true;
    options.indent_preproc_define = true;
    options.indent_preproc_conditional = true;
    options.indent_preproc_block = true;
    let source = fixture!(
        "void run(){if(alphaCondition&&",
        "#if ENABLED",
        "betaCondition&&gammaCondition&&deltaCondition&&epsilonCondition",
        "#endif",
        "zetaCondition){call();}}",
    );

    // Additive preprocessor levels retain the configured indentation prefix.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "\tif(alphaCondition&&",
            "\t\t#if ENABLED",
            "\t        betaCondition&&gammaCondition&&deltaCondition&&epsilonCondition",
            "\t\t#endif",
            "\t        zetaCondition)",
            "\t{",
            "\t\tcall();",
            "\t}",
            "}",
        ),
    );
}

#[test]
fn max_code_length_case_body_call_uses_adjusted_base_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 8;
    options.max_code_length = Some(80);
    options.break_after_logical = true;
    let source = fixture!(
        "void run(int value){switch(value){case 1:resultValue=calculateResult(alphaValue,betaValue,gammaValue,deltaValue,epsilonValue);break;default:break;}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run(int value)",
            "{",
            "        switch(value)",
            "        {",
            "        case 1:",
            "                resultValue=calculateResult(alphaValue,betaValue,gammaValue,deltaValue,",
            "                                            epsilonValue);",
            "                break;",
            "        default:",
            "                break;",
            "        }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_tab_indent_uses_visual_structural_width() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue+zetaValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run() {",
            "\tresultValue=alphaValue+betaValue+gammaValue",
            "\t            +deltaValue+epsilonValue+zetaValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_tab_continuation_keeps_only_structural_tab() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){textValue=\"this literal remains one indivisible sequence even when it extends beyond the configured width\";}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run() {",
            "\ttextValue=",
            "\t    \"this literal remains one indivisible sequence even when it extends beyond the configured width\";",
            "}",
        ),
    );
}

#[test]
fn max_code_length_lisp_reserves_attached_closing_brace_suffix() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Lisp;
    options.max_code_length = Some(50);
    let source = fixture!(
        "bool run(){resultValue=alphaValue==betaValue||gammaValue!=deltaValue||epsilonValue>=zetaValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "bool run() {",
            "    resultValue=alphaValue==betaValue",
            "                ||gammaValue!=deltaValue",
            "                ||epsilonValue>=zetaValue; }",
        ),
    );
}

#[test]
fn max_code_length_lisp_splits_attached_lambda_suffix() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Lisp;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=outerFunction(alphaValue,[](int firstValue,int secondValue){return firstValue+secondValue;},betaValue,gammaValue);}",
    );

    // Split the suffix at the last fitting comma before Lisp postprocessing.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run() {",
            "  resultValue=outerFunction(alphaValue,[](",
            "  int firstValue,int secondValue) {",
            "    return firstValue+secondValue; },betaValue,",
            "  gammaValue); }",
        ),
    );
}

#[test]
fn max_code_length_pico_run_in_prefix_counts_toward_first_split() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.max_code_length = Some(50);
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pad_parens_outside = true;
    let source = fixture!(
        "void run(){resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue; // trailing explanation remains intact",
        "}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{   resultValue = alphaValue + betaValue +",
            "                  gammaValue + deltaValue +",
            "                  epsilonValue; // trailing explanation remains intact",
            "}",
        ),
    );
}

#[test]
fn max_code_length_pico_keeps_long_inline_body_unsplit() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue+zetaValue+etaValue+thetaValue;}",
    );

    // Balanced inline bodies are indivisible once their header fits.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run() {resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue+zetaValue+etaValue+thetaValue;}",
        ),
    );
}

#[test]
fn max_code_length_pico_splits_long_header_before_inline_body() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.max_code_length = Some(50);
    let source = fixture!(
        "ResultType calculateResult(AlphaType alphaValue,BetaType betaValue,GammaType gammaValue,DeltaType deltaValue){return alphaValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "ResultType calculateResult(AlphaType alphaValue,",
            "                           BetaType betaValue,GammaType gammaValue,",
            "                           DeltaType deltaValue) {return alphaValue;}",
        ),
    );
}

#[test]
fn max_code_length_lisp_keeps_parameter_type_with_name() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Lisp;
    options.break_one_line_statements = false;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "class Item{public:ResultType calculateResult(AlphaType alphaValue,BetaType betaValue,GammaType gammaValue,DeltaType deltaValue);};",
    );

    // The earlier delimiter boundary keeps each parameter declaration intact.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "class Item {",
            "public:ResultType calculateResult(",
            "    AlphaType alphaValue,BetaType betaValue,",
            "    GammaType gammaValue,DeltaType deltaValue); };",
        ),
    );
}

#[test]
fn max_code_length_splits_constructor_parameters_and_members() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "Item::Item(AlphaType alphaValue,BetaType betaValue):firstMember(alphaValue),secondMember(betaValue),thirdMember(calculate(alphaValue,betaValue)){}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "Item::Item(AlphaType alphaValue,",
            "           BetaType betaValue):firstMember(alphaValue),",
            "    secondMember(betaValue),",
            "    thirdMember(calculate(alphaValue,betaValue)) {}",
        ),
    );
}

#[test]
fn max_code_length_constructor_member_call_uses_emitted_member_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(60);
    let source = fixture!(
        "Item::Item(AlphaType alphaValue,BetaType betaValue):firstMember(alphaValue),secondMember(betaValue),thirdMember(calculate(alphaValue,betaValue)){}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "Item::Item(AlphaType alphaValue,",
            "           BetaType betaValue):firstMember(alphaValue),",
            "    secondMember(betaValue),thirdMember(calculate(alphaValue,",
            "                                        betaValue)) {}",
        ),
    );
}

#[test]
fn max_code_length_prefers_fitting_parameter_commas_over_pointer_declarators() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(60);
    let source = fixture!(
        "ResultType calculateResult(const AlphaType *alphaValue,const BetaType *betaValue,const GammaType *gammaValue,const DeltaType *deltaValue);",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "ResultType calculateResult(const AlphaType *alphaValue,",
            "                           const BetaType *betaValue,const GammaType *gammaValue,",
            "                           const DeltaType *deltaValue);",
        ),
    );
}

#[test]
fn max_code_length_horstmann_run_in_prefix_counts_toward_first_split() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Horstmann;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=calculateResult(alphaValue,betaValue,gammaValue,deltaValue,epsilonValue,zetaValue,etaValue);}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{   resultValue=calculateResult(alphaValue,",
            "                                betaValue,gammaValue,deltaValue,epsilonValue,",
            "                                zetaValue,etaValue);",
            "}",
        ),
    );
}

#[test]
fn max_code_length_horstmann_exact_run_in_width_uses_previous_boundary() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Horstmann;
    options.max_code_length = Some(60);
    options.break_after_logical = true;
    let source = fixture!(
        "void run(){for(int indexValue=initialValue;indexValue<maximumValue&&readyCondition;++indexValue){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{   for(int indexValue=initialValue;",
            "            indexValue<maximumValue&&readyCondition; ++indexValue)",
            "    {   call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_horstmann_exact_run_in_width_keeps_fitting_head() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Horstmann;
    options.max_code_length = Some(60);
    let source = fixture!(
        "void run(){for(int indexValue=initialValue;indexValue<maximumValue&&readyCondition;++indexValue){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{   for(int indexValue=initialValue; indexValue<maximumValue",
            "            &&readyCondition; ++indexValue)",
            "    {   call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_reserves_continuation_prefix_before_trailing_comment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue; // trailing explanation remains intact",
        "}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "  resultValue=alphaValue+betaValue+gammaValue",
            "              +deltaValue",
            "              +epsilonValue; // trailing explanation remains intact",
            "}",
        ),
    );
}

#[test]
fn max_code_length_does_not_split_header_for_trailing_comment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){// an intentionally long column-one comment is not a code split candidate",
        "resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue;",
        "}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run() // an intentionally long column-one comment is not a code split candidate",
            "{",
            "  resultValue=alphaValue+betaValue+gammaValue",
            "              +deltaValue+epsilonValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_splits_code_when_trailing_comment_overflows() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 8;
    options.max_code_length = Some(80);
    let source = fixture!(
        "void run(){resultValue=alphaValue+betaValue+gammaValue+deltaValue+epsilonValue; // trailing explanation remains intact",
        "}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "        resultValue=alphaValue+betaValue+gammaValue+deltaValue",
            "                    +epsilonValue; // trailing explanation remains intact",
            "}",
        ),
    );
}

#[test]
fn max_code_length_splits_after_indivisible_escaped_string() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){textValue=\"alpha beta gamma delta epsilon zeta eta theta\\\" quoted\"+suffixValue+otherValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "  textValue=",
            "    \"alpha beta gamma delta epsilon zeta eta theta\\\" quoted\"",
            "    +suffixValue+otherValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_never_splits_inside_interstitial_block_comment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_width = 2;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){resultValue=alphaValue+betaValue+/* detail */gammaValue+deltaValue+epsilonValue+zetaValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "  resultValue=alphaValue",
            "              +betaValue+/* detail */gammaValue+deltaValue",
            "              +epsilonValue+zetaValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_keeps_indivisible_string_call_before_comparison() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pad_parens_outside = true;
    let source = fixture!(
        "void run(){if(TEXT_VALUE(\"alpha beta gamma delta epsilon\")==expectedLongValue&&readyCondition){call();}}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    if (TEXT_VALUE (\"alpha beta gamma delta epsilon\")",
            "            == expectedLongValue && readyCondition)",
            "    {",
            "        call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_horstmann_moves_operator_after_indivisible_string_call() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Horstmann;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){if(TEXT_VALUE(\"alpha beta gamma delta epsilon\")==expectedLongValue&&readyCondition){call();}}",
    );

    // An indivisible string call does not invalidate the following operator boundary.
    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{   if(TEXT_VALUE(\"alpha beta gamma delta epsilon\")",
            "            ==expectedLongValue&&readyCondition)",
            "    {   call();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn max_code_length_padded_indivisible_string_splits_before_value() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pad_parens_outside = true;
    let source = fixture!(
        "void run(){textValue=\"alpha beta gamma delta epsilon zeta eta theta\\\" quoted\"+suffixValue+otherValue;}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    textValue =",
            "        \"alpha beta gamma delta epsilon zeta eta theta\\\" quoted\"",
            "        + suffixValue + otherValue;",
            "}",
        ),
    );
}

#[test]
fn max_code_length_padded_string_keeps_fitting_plus_on_previous_row() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(80);
    options.pad_operators = true;
    let source = fixture!(
        "void run(){textValue=\"alpha alpha alpha\"+\"beta beta beta\"+\"gamma gamma gamma\"+\"delta delta delta\";}",
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    textValue = \"alpha alpha alpha\" + \"beta beta beta\" + \"gamma gamma gamma\" +",
            "                \"delta delta delta\";",
            "}",
        ),
    );
}

#[test]
fn max_code_length_using_alias_honors_configured_continuation() {
    let source = fixture!(
        "using ResultType=WrapperType<AlphaType,BetaType,GammaType,DeltaType,EpsilonType,ZetaType,EtaType,ThetaType>;",
    );
    for (continuation_indent, indent_after_parens, expected_spaces) in
        [(0, false, 0), (4, false, 16), (2, true, 8)]
    {
        let mut options = FormatOptions::default();
        options.brace_style = BraceStyle::Allman;
        options.max_code_length = Some(50);
        options.continuation_indent = continuation_indent;
        options.indent_after_parens = indent_after_parens;
        let continuation = format!(
            "{}WrapperType<AlphaType,BetaType,GammaType,DeltaType,EpsilonType,ZetaType,EtaType,ThetaType>;",
            " ".repeat(expected_spaces)
        );

        let expected = format!("using ResultType=\n{continuation}\n");
        assert_stable_max_length_format(source, &options, &expected);
    }
}

#[test]
fn max_code_length_string_concatenation_splits_stably() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.max_code_length = Some(50);
    let source = fixture!(
        "void run(){textValue=\"alpha alpha alpha\"+\"beta beta beta\"+\"gamma gamma gamma\"+\"delta delta delta\";}"
    );

    assert_stable_max_length_format(
        source,
        &options,
        fixture!(
            "void run()",
            "{",
            "    textValue=\"alpha alpha alpha\"+\"beta beta beta\"",
            "              +\"gamma gamma gamma\"+\"delta delta delta\";",
            "}",
        ),
    );
}

#[test]
fn max_code_length_aligns_assignment_continuations_to_operand() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    options.pad_operators = true;
    options.break_after_logical = true;
    let actual = format_with(
        fixture!("void f(){result = alpha + beta + gamma + delta + epsilon + zeta;}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    result = alpha + beta + gamma + delta + epsilon +",
            "             zeta;",
            "}",
        )
    );
}

#[test]
fn max_code_length_floors_conditional_continuation_at_min_conditional_indent() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    let source = fixture!(
        "void f(void)",
        "{",
        "    if (alphaValue == betaValue || gammaValue == deltaValue || gg == ff) {",
        "        helper();",
        "    }",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void f(void)",
            "{",
            "    if (alphaValue == betaValue",
            "            || gammaValue == deltaValue || gg == ff) {",
            "        helper();",
            "    }",
            "}",
        )
    );

    options.min_conditional_indent = MinConditionalIndent::Zero;
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void f(void)",
            "{",
            "    if (alphaValue == betaValue",
            "        || gammaValue == deltaValue || gg == ff) {",
            "        helper();",
            "    }",
            "}",
        )
    );
}

#[test]
fn max_code_length_never_splits_at_scope_or_arrow_operator() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "    result = first->alpha + second->beta + third->gammaLongName;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    result = first->alpha + second->beta +",
            "             third->gammaLongName;",
            "}",
        )
    );

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "    Config::instance().applySettings(alphaParam, betaParam, gammaParam);",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    Config::instance().applySettings(alphaParam,",
            "                                     betaParam, gammaParam);",
            "}",
        )
    );
}

#[test]
fn max_code_length_never_splits_inside_a_trailing_comment() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "    resultValue = alphaName + betaName + gammaName; // trailing comment text here",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    resultValue = alphaName + betaName +",
            "                  gammaName; // trailing comment text here",
            "}",
        )
    );

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "    resultValue = alphaName + betaName + gammaName; /* trailing block comment */",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    resultValue = alphaName + betaName +",
            "                  gammaName; /* trailing block comment */",
            "}",
        )
    );
}

#[test]
fn max_code_length_keeps_spaceship_operator_intact() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(28);
    let actual = format_with(fixture!("int f(){return alpha<=>beta+gamma;}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    return alpha <=>",
            "        beta + gamma;",
            "}",
        )
    );
}

#[test]
fn splits_long_lines_at_commas_and_operators() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(32);
    let actual = format_with(
        fixture!(
            "int f(){return sum(alpha,beta,gamma,delta);}",
            "int g(){return alpha+beta+gamma+delta;}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    return sum(alpha, beta, gamma,",
            "               delta);",
            "}",
            "int g()",
            "{",
            "    return alpha + beta + gamma +",
            "           delta;",
            "}",
        )
    );
}

#[test]
fn max_code_length_aligns_nested_call_after_open_paren() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(90);
    options.pad_commas = true;
    let actual = format_with(
        fixture!(
            "void f(){process_result(EVENT_ERROR, context, 0, build_nested_value(alpha, beta, gamma), fetch_related_value(delta, epsilon));}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    process_result(EVENT_ERROR, context, 0, build_nested_value(alpha, beta, gamma),",
            "                   fetch_related_value(delta, epsilon));",
            "}",
        )
    );
}

#[test]
fn line_splitting_preserves_non_whitespace_tokens() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(32);
    let source = fixture!("int f(){return alpha+beta+gamma+delta;}");
    let actual = format_with(source, &options);

    assert_eq!(non_whitespace(&actual), non_whitespace(source));
}

#[test]
fn max_code_length_does_not_split_pointer_declarators() {
    let mut options = FormatOptions::default();
    options.max_code_length = Some(50);
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(){const char *very_long_pointer_name = some_call(alpha,beta,gamma,delta);}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    const char *very_long_pointer_name = some_call(",
            "            alpha, beta, gamma, delta);",
            "}",
        )
    );
}

#[test]
fn max_code_length_splits_before_name_aligned_pointer_parameter() {
    let mut options = FormatOptions::default();
    let args = [
        "--align-pointer=name",
        "--indent=tab=8",
        "--convert-tabs",
        "--max-code-length=100",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "\nvoid foo(char\t\t*\t\t\t\t\t\t\t\t\t\t\t\t\tbar)\n{}\n",
            &options,
        ),
        "\nvoid foo(char\n         *bar)\n{}\n",
    );
}

#[test]
fn max_code_length_splits_after_malformed_pointer_cast_group() {
    let mut options = FormatOptions::default();
    let args = ["--style=java", "--max-code-length=50"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    ((Object *)Factory::object_factory()->set_debug_line(-1);\n}\n",
            &options,
        ),
        "\nvoid foo() {\n    ((Object *)\n     Factory::object_factory()->set_debug_line(-1);\n}\n",
    );
}
