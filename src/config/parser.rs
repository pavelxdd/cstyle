use super::{
    ConfigError, FormatOptions, IndentStyle, LineBetweenMembers, LineEnding, MinConditionalIndent,
    Mode, ObjCColonPad, PointerAlign, ReferenceAlign, StylePreset,
};
use crate::source::{lex, line_endings};
use std::path::Path;

pub fn apply_command_line_args(
    options: &mut FormatOptions,
    args: &[String],
) -> Result<(), ConfigError> {
    let path = Path::new("<command-line>");
    let mut updated = options.clone();
    for arg in args {
        apply_option_token(path, 1, arg, &mut updated, OptionSource::CommandLine)?;
    }
    *options = updated;
    Ok(())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OptionSource {
    Config,
    CommandLine,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct OptionToken {
    text: String,
    line_number: usize,
}

pub(super) fn parse_source(path: &Path, source: &str) -> Result<FormatOptions, ConfigError> {
    let mut options = FormatOptions::default();
    apply_source(path, source, &mut options)?;
    Ok(options)
}

pub(super) fn apply_source(
    path: &Path,
    source: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    for token in import_option_tokens(source) {
        apply_option_token(
            path,
            token.line_number,
            &token.text,
            options,
            OptionSource::Config,
        )?;
    }
    Ok(())
}

fn import_option_tokens(source: &str) -> Vec<OptionToken> {
    let source = line_endings::normalize(source);
    let source = source.strip_prefix('\u{feff}').unwrap_or(&source);
    let mut tokens = Vec::new();
    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let mut token = String::new();
        let mut is_in_quote = false;
        let mut quote_char = ' ';

        for ch in raw_line.chars() {
            if ch == '#' {
                break;
            }
            if is_in_quote {
                if ch == quote_char {
                    is_in_quote = false;
                } else {
                    token.push(ch);
                }
                continue;
            }
            match ch {
                '\'' | '"' => {
                    is_in_quote = true;
                    quote_char = ch;
                }
                ' ' | '\t' | ',' => push_option_token(&mut tokens, &mut token, line_number),
                _ => token.push(ch),
            }
        }
        push_option_token(&mut tokens, &mut token, line_number);
    }
    tokens
}

fn push_option_token(tokens: &mut Vec<OptionToken>, token: &mut String, line_number: usize) {
    let text = token.trim();
    if !text.is_empty() {
        tokens.push(OptionToken {
            text: text.to_string(),
            line_number,
        });
    }
    token.clear();
}

fn apply_option_token(
    path: &Path,
    line_number: usize,
    token: &str,
    options: &mut FormatOptions,
    source: OptionSource,
) -> Result<(), ConfigError> {
    let token = token.trim();
    if token.is_empty() {
        return Ok(());
    }

    let mut updated = options.clone();
    if let Some(option) = token.strip_prefix("--") {
        apply_single_option_with_style(path, line_number, option, &mut updated, source)?;
    } else if let Some(short_options) = token.strip_prefix('-') {
        if short_options.is_empty() {
            return Err(unknown_option_error(path, line_number, token, source));
        }
        for option in split_short_options(short_options) {
            apply_single_option_with_style(path, line_number, &option, &mut updated, source)?;
        }
    } else {
        apply_single_option_with_style(path, line_number, token, &mut updated, source)?;
    }
    *options = updated;
    Ok(())
}

fn apply_single_option_with_style(
    path: &Path,
    line_number: usize,
    option: &str,
    options: &mut FormatOptions,
    source: OptionSource,
) -> Result<(), ConfigError> {
    let previous_style = options.remove_active_style();
    apply_single_option(path, line_number, option, options, source)?;
    if !options.has_active_style()
        && let Some(style) = previous_style
    {
        options.apply_style_preset(style);
    }
    Ok(())
}

pub(crate) fn split_short_options(options: &str) -> Vec<String> {
    let mut split = Vec::new();
    let mut current = String::new();
    let mut previous = ' ';
    for (index, ch) in options.chars().enumerate() {
        if index > 0 && ch.is_ascii_alphabetic() && previous != 'x' {
            split.push(current);
            current = String::new();
        }
        current.push(ch);
        previous = ch;
    }
    if !current.is_empty() {
        split.push(current);
    }
    split
}

fn apply_single_option(
    path: &Path,
    line_number: usize,
    option: &str,
    options: &mut FormatOptions,
    source: OptionSource,
) -> Result<(), ConfigError> {
    if apply_short_alias(path, line_number, option, options)? {
        return Ok(());
    }

    let Some((key, value)) = option.split_once('=') else {
        return apply_flag(path, line_number, option, options, source);
    };

    match key.trim() {
        "indent" => apply_indent(path, line_number, value.trim(), options),
        "style" => apply_style(path, line_number, value.trim(), options),
        "indent-continuation" => apply_usize_range(
            path,
            line_number,
            value.trim(),
            "continuation indent",
            0,
            4,
            |value| options.continuation_indent = value,
        ),
        "max-continuation-indent" | "max-instatement-indent" => apply_usize_range(
            path,
            line_number,
            value.trim(),
            "max continuation indent",
            40,
            120,
            |value| options.max_continuation_indent = value,
        ),
        "min-conditional-indent" => {
            apply_min_conditional_indent(path, line_number, value.trim(), options)
        }
        "align-pointer" => apply_pointer_align(path, line_number, value.trim(), options),
        "align-reference" => apply_reference_align(path, line_number, value.trim(), options),
        "max-code-length" => apply_usize_range(
            path,
            line_number,
            value.trim(),
            "max code length",
            50,
            200,
            |value| options.max_code_length = Some(value),
        ),
        "break-blocks" => apply_break_blocks_value(path, line_number, value.trim(), options),
        "pad-method-colon" => apply_method_colon_pad(path, line_number, value.trim(), options),
        "line-between-members" => {
            apply_line_between_members(path, line_number, value.trim(), options)
        }
        "lineend" => apply_line_end(path, line_number, value.trim(), options),
        "mode" => apply_mode(path, line_number, value.trim(), options),
        "access-label" => apply_access_label(path, line_number, value.trim(), options),
        "macro-block" => apply_macro_block(path, line_number, value.trim(), options),
        "control-header" => apply_control_header(path, line_number, value.trim(), options),
        "non-paren-header" => apply_non_paren_header(path, line_number, value.trim(), options),
        key => Err(unknown_option_error(path, line_number, key, source)),
    }
}

fn apply_flag(
    path: &Path,
    line_number: usize,
    flag: &str,
    options: &mut FormatOptions,
    source: OptionSource,
) -> Result<(), ConfigError> {
    match flag {
        "indent-after-parens" => options.indent_after_parens = true,
        "indent-braces" => options.indent_braces = true,
        "indent-blocks" => options.indent_blocks = true,
        "indent-switches" => options.indent_switches = true,
        "indent-cases" => options.indent_cases = true,
        "indent-labels" => options.indent_labels = true,
        "indent-classes" => options.indent_classes = true,
        "indent-modifiers" => options.indent_modifiers = true,
        "indent-preprocessor" | "indent-preproc-define" => options.indent_preproc_define = true,
        "indent-preproc-cond" => options.indent_preproc_conditional = true,
        "indent-preproc-block" => options.indent_preproc_block = true,
        "indent-namespaces" => options.indent_namespaces = true,
        "indent-col1-comments" => options.indent_col1_comments = true,
        "delete-empty-lines" => options.delete_empty_lines = true,
        "fill-empty-lines" => options.empty_line_fill = true,
        "remove-comment-prefix" => options.strip_comment_prefix = true,
        "convert-tabs" => options.convert_tabs = true,
        "close-templates" => options.close_templates = true,
        "pad-method-prefix" => options.pad_method_prefix = true,
        "unpad-method-prefix" => options.unpad_method_prefix = true,
        "pad-return-type" => options.pad_return_type = true,
        "unpad-return-type" => options.unpad_return_type = true,
        "pad-param-type" => options.pad_param_type = true,
        "unpad-param-type" => options.unpad_param_type = true,
        "align-method-colon" => options.align_method_colon = true,
        "break-one-line-headers" => options.break_one_line_headers = true,
        "keep-one-line-blocks" => apply_keep_one_line_blocks(options),
        "keep-one-line-statements" => apply_keep_one_line_statements(options),
        "add-braces" | "add-brackets" => apply_add_braces(path, line_number, options)?,
        "add-one-line-braces" | "add-one-line-brackets" => {
            apply_add_one_line_braces(path, line_number, options)?
        }
        "remove-braces" | "remove-brackets" => apply_remove_braces(path, line_number, options)?,
        "pad-oper" => options.pad_operators = true,
        "pad-comma" => options.pad_commas = true,
        "pad-paren" => {
            options.pad_parens_outside = true;
            options.pad_parens_inside = true;
        }
        "pad-paren-out" => options.pad_parens_outside = true,
        "pad-first-paren-out" => options.pad_first_paren_outside = true,
        "pad-paren-in" => options.pad_parens_inside = true,
        "pad-header" => options.pad_header = true,
        "unpad-paren" => options.unpad_parens = true,
        "break-after-logical" => options.break_after_logical = true,
        "break-blocks" => options.break_blocks = true,
        "break-closing-braces" | "break-closing-brackets" => options.break_closing_braces = true,
        "attach-namespaces" | "attach-namespace" => options.attach_namespace = true,
        "attach-classes" | "attach-class" => options.attach_class = true,
        "attach-inlines" | "attach-inline" => options.attach_inline = true,
        "attach-extern-c" => options.attach_extern_c = true,
        "attach-closing-while" => options.attach_closing_while = true,
        "break-elseifs" => options.break_else_ifs = true,
        "no-indent-if-after-else" => options.no_indent_if_after_else = true,
        "line-between-members" => options.line_between_members = LineBetweenMembers::Members,
        "break-return-type" => apply_break_return_type(path, line_number, options)?,
        "break-return-type-decl" => apply_break_return_type_decl(path, line_number, options)?,
        "attach-return-type" => apply_attach_return_type(path, line_number, options)?,
        "attach-return-type-decl" => apply_attach_return_type_decl(path, line_number, options)?,
        _ => {
            return Err(unknown_option_error(path, line_number, flag, source));
        }
    }
    Ok(())
}

fn unknown_option_error(
    path: &Path,
    line_number: usize,
    option: &str,
    source: OptionSource,
) -> ConfigError {
    let label = match source {
        OptionSource::Config => "unknown config key",
        OptionSource::CommandLine => "unknown option",
    };
    ConfigError::line(path, line_number, format!("{label} '{option}'"))
}

fn apply_short_alias(
    path: &Path,
    line_number: usize,
    option: &str,
    options: &mut FormatOptions,
) -> Result<bool, ConfigError> {
    match option {
        "A1" => apply_style(path, line_number, "allman", options)?,
        "A2" => apply_style(path, line_number, "java", options)?,
        "A3" => apply_style(path, line_number, "kr", options)?,
        "A4" => apply_style(path, line_number, "stroustrup", options)?,
        "A5" => apply_style(path, line_number, "whitesmith", options)?,
        "A6" => apply_style(path, line_number, "ratliff", options)?,
        "A7" => apply_style(path, line_number, "gnu", options)?,
        "A8" => apply_style(path, line_number, "linux", options)?,
        "A9" => apply_style(path, line_number, "horstmann", options)?,
        "A10" => apply_style(path, line_number, "1tbs", options)?,
        "A11" => apply_style(path, line_number, "pico", options)?,
        "A12" => apply_style(path, line_number, "lisp", options)?,
        "A14" => apply_style(path, line_number, "google", options)?,
        "A15" => apply_style(path, line_number, "vtk", options)?,
        "A16" => apply_style(path, line_number, "mozilla", options)?,
        "A17" => apply_style(path, line_number, "webkit", options)?,
        "S" => options.indent_switches = true,
        "K" => options.indent_cases = true,
        "xU" => options.indent_after_parens = true,
        "L" => options.indent_labels = true,
        "C" => options.indent_classes = true,
        "xG" => options.indent_modifiers = true,
        "xW" => options.indent_preproc_block = true,
        "w" => options.indent_preproc_define = true,
        "xw" => options.indent_preproc_conditional = true,
        "N" => options.indent_namespaces = true,
        "Y" => options.indent_col1_comments = true,
        "xe" => options.delete_empty_lines = true,
        "E" => options.empty_line_fill = true,
        "xp" => options.strip_comment_prefix = true,
        "c" => options.convert_tabs = true,
        "xy" => options.close_templates = true,
        "xQ" => options.pad_method_prefix = true,
        "xR" => options.unpad_method_prefix = true,
        "xq" => options.pad_return_type = true,
        "xr" => options.unpad_return_type = true,
        "xS" => options.pad_param_type = true,
        "xs" => options.unpad_param_type = true,
        "xM" => options.align_method_colon = true,
        "xb" => options.break_one_line_headers = true,
        "O" => apply_keep_one_line_blocks(options),
        "o" => apply_keep_one_line_statements(options),
        "j" => apply_add_braces(path, line_number, options)?,
        "J" => apply_add_one_line_braces(path, line_number, options)?,
        "xj" => apply_remove_braces(path, line_number, options)?,
        "p" => options.pad_operators = true,
        "xg" => options.pad_commas = true,
        "P" => {
            options.pad_parens_outside = true;
            options.pad_parens_inside = true;
        }
        "d" => options.pad_parens_outside = true,
        "xd" => options.pad_first_paren_outside = true,
        "D" => options.pad_parens_inside = true,
        "H" => options.pad_header = true,
        "U" => options.unpad_parens = true,
        "xL" => options.break_after_logical = true,
        "f" => options.break_blocks = true,
        "F" => {
            options.break_blocks = true;
            options.break_closing_header_blocks = true;
        }
        "y" => options.break_closing_braces = true,
        "xn" => options.attach_namespace = true,
        "xc" => options.attach_class = true,
        "xl" => options.attach_inline = true,
        "xk" => options.attach_extern_c = true,
        "xV" => options.attach_closing_while = true,
        "e" => options.break_else_ifs = true,
        "xB" => apply_break_return_type(path, line_number, options)?,
        "xD" => apply_break_return_type_decl(path, line_number, options)?,
        "xf" => apply_attach_return_type(path, line_number, options)?,
        "xh" => apply_attach_return_type_decl(path, line_number, options)?,
        _ => {
            if apply_short_param_alias(path, line_number, option, options)? {
                return Ok(true);
            }
            return Ok(false);
        }
    }
    Ok(true)
}

fn apply_short_param_alias(
    path: &Path,
    line_number: usize,
    option: &str,
    options: &mut FormatOptions,
) -> Result<bool, ConfigError> {
    if let Some(value) = short_param(option, "s") {
        let value = default_param(value, "4");
        return apply_indent_width(path, line_number, value, IndentStyle::Spaces, options)
            .map(|()| true);
    }
    if let Some(value) = short_param(option, "t") {
        let value = default_param(value, "4");
        return apply_indent_width(path, line_number, value, IndentStyle::Tabs, options)
            .map(|()| true);
    }
    if let Some(value) = short_param(option, "T") {
        let value = default_param(value, "4");
        return apply_indent_width(path, line_number, value, IndentStyle::ForceTabs, options)
            .map(|()| true);
    }
    if let Some(value) = short_param(option, "xT") {
        let value = default_param(value, "8");
        let tab_width = parse_usize(path, line_number, value, "tab width")?;
        validate_range(path, line_number, tab_width, "tab width", 2, 20)?;
        options.indent_style = IndentStyle::ForceTabs;
        options.set_force_tab_width(tab_width);
        return Ok(true);
    }
    if let Some(value) = short_param(option, "xt") {
        let value = default_param(value, "1");
        return apply_usize_range(
            path,
            line_number,
            value,
            "continuation indent",
            0,
            4,
            |value| options.continuation_indent = value,
        )
        .map(|()| true);
    }
    if let Some(value) = short_param(option, "M") {
        let value = default_param(value, "40");
        return apply_usize_range(
            path,
            line_number,
            value,
            "max continuation indent",
            40,
            120,
            |value| options.max_continuation_indent = value,
        )
        .map(|()| true);
    }
    if let Some(value) = short_param(option, "m") {
        let value = default_param(value, "2");
        return apply_min_conditional_indent(path, line_number, value, options).map(|()| true);
    }
    if let Some(value) = short_param(option, "k") {
        match value {
            "1" => options.pointer_align = PointerAlign::Type,
            "2" => options.pointer_align = PointerAlign::Middle,
            "3" => options.pointer_align = PointerAlign::Name,
            _ => {
                return Err(ConfigError::line(
                    path,
                    line_number,
                    "align-pointer must be 1, 2, or 3",
                ));
            }
        }
        return Ok(true);
    }
    if let Some(value) = short_param(option, "W") {
        match value {
            "0" => options.reference_align = ReferenceAlign::None,
            "1" => options.reference_align = ReferenceAlign::Type,
            "2" => options.reference_align = ReferenceAlign::Middle,
            "3" => options.reference_align = ReferenceAlign::Name,
            _ => {
                return Err(ConfigError::line(
                    path,
                    line_number,
                    "align-reference must be 0, 1, 2, or 3",
                ));
            }
        }
        return Ok(true);
    }
    if let Some(value) = short_param(option, "xC") {
        let value = default_param(value, "50");
        return apply_usize_range(
            path,
            line_number,
            value,
            "max code length",
            50,
            200,
            |value| options.max_code_length = Some(value),
        )
        .map(|()| true);
    }
    if let Some(value) = short_param(option, "z") {
        match value {
            "1" => options.line_ending = LineEnding::Crlf,
            "2" => options.line_ending = LineEnding::Lf,
            "3" => options.line_ending = LineEnding::Cr,
            _ => {
                return Err(ConfigError::line(
                    path,
                    line_number,
                    "lineend must be 1, 2, or 3",
                ));
            }
        }
        return Ok(true);
    }
    if let Some(value) = short_param(option, "xP") {
        options.pad_method_colon = match value {
            "0" => ObjCColonPad::None,
            "1" => ObjCColonPad::All,
            "2" => ObjCColonPad::After,
            "3" => ObjCColonPad::Before,
            _ => {
                return Err(ConfigError::line(
                    path,
                    line_number,
                    "pad-method-colon must be 0, 1, 2, or 3",
                ));
            }
        };
        return Ok(true);
    }
    Ok(false)
}

fn short_param<'a>(option: &'a str, name: &str) -> Option<&'a str> {
    let value = option.strip_prefix(name)?;
    if value.is_empty() || value.starts_with(|ch: char| ch.is_ascii_digit()) {
        Some(value)
    } else {
        None
    }
}

fn default_param<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.is_empty() { default } else { value }
}

fn apply_style(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    let style = match value {
        "none" => StylePreset::None,
        "allman" | "bsd" | "break" | "ansi" => StylePreset::Allman,
        "java" | "attach" => StylePreset::Java,
        "kr" | "k&r" | "k/r" => StylePreset::Kr,
        "linux" | "knf" => StylePreset::Linux,
        "mozilla" => StylePreset::Mozilla,
        "webkit" => StylePreset::WebKit,
        "stroustrup" => StylePreset::Stroustrup,
        "1tbs" | "otbs" => StylePreset::OneTrueBrace,
        "whitesmith" => StylePreset::Whitesmith,
        "vtk" => StylePreset::Vtk,
        "ratliff" | "banner" => StylePreset::Ratliff,
        "gnu" => StylePreset::Gnu,
        "horstmann" | "run-in" => StylePreset::Horstmann,
        "google" => StylePreset::Google,
        "pico" => StylePreset::Pico,
        "lisp" | "python" => StylePreset::Lisp,
        _ => return Err(unsupported_style_error(path, line_number, value)),
    };
    options.set_style(style);
    Ok(())
}

fn unsupported_style_error(path: &Path, line_number: usize, value: &str) -> ConfigError {
    ConfigError::line(
        path,
        line_number,
        format!("unsupported style value '{value}'"),
    )
}

fn apply_keep_one_line_blocks(options: &mut FormatOptions) {
    options.keep_one_line_blocks();
}

fn apply_keep_one_line_statements(options: &mut FormatOptions) {
    options.break_one_line_statements = false;
}

fn apply_add_braces(
    _path: &Path,
    _line_number: usize,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.add_braces = true;
    options.remove_braces = false;
    Ok(())
}

fn apply_add_one_line_braces(
    _path: &Path,
    _line_number: usize,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.add_one_line_braces = true;
    options.remove_braces = false;
    options.break_one_line_blocks = false;
    Ok(())
}

fn apply_remove_braces(
    _path: &Path,
    _line_number: usize,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    if !options.add_braces && !options.add_one_line_braces {
        options.remove_braces = true;
    }
    Ok(())
}

fn apply_break_return_type(
    _path: &Path,
    _line_number: usize,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.break_return_type = true;
    options.attach_return_type = false;
    Ok(())
}

fn apply_break_return_type_decl(
    _path: &Path,
    _line_number: usize,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.break_return_type_decl = true;
    options.attach_return_type_decl = false;
    Ok(())
}

fn apply_attach_return_type(
    _path: &Path,
    _line_number: usize,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    if !options.break_return_type {
        options.attach_return_type = true;
    }
    Ok(())
}

fn apply_attach_return_type_decl(
    _path: &Path,
    _line_number: usize,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    if !options.break_return_type_decl {
        options.attach_return_type_decl = true;
    }
    Ok(())
}

fn apply_break_blocks_value(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    match value {
        "all" => {
            options.break_blocks = true;
            options.break_closing_header_blocks = true;
            Ok(())
        }
        _ => Err(ConfigError::line(
            path,
            line_number,
            "break-blocks value must be all",
        )),
    }
}

fn apply_method_colon_pad(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.pad_method_colon = match value {
        "none" => ObjCColonPad::None,
        "all" => ObjCColonPad::All,
        "after" => ObjCColonPad::After,
        "before" => ObjCColonPad::Before,
        _ => {
            return Err(ConfigError::line(
                path,
                line_number,
                "pad-method-colon must be none, all, after, or before",
            ));
        }
    };
    Ok(())
}

fn apply_line_between_members(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.line_between_members = match value {
        "all" => LineBetweenMembers::All,
        _ => {
            return Err(ConfigError::line(
                path,
                line_number,
                "line-between-members value must be all",
            ));
        }
    };
    Ok(())
}

fn apply_indent(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    if value == "spaces" {
        options.set_indent_style_width(IndentStyle::Spaces, 4);
        return Ok(());
    }
    if value == "tab" {
        options.set_indent_style_width(IndentStyle::Tabs, 4);
        return Ok(());
    }
    if value == "force-tab" {
        options.set_indent_style_width(IndentStyle::ForceTabs, 4);
        return Ok(());
    }
    if value == "force-tab-x" {
        options.indent_style = IndentStyle::ForceTabs;
        options.set_force_tab_width(8);
        return Ok(());
    }

    if let Some(width) = value.strip_prefix("spaces=") {
        return apply_indent_width(path, line_number, width, IndentStyle::Spaces, options);
    }
    if let Some(width) = value.strip_prefix("tab=") {
        return apply_indent_width(path, line_number, width, IndentStyle::Tabs, options);
    }
    if let Some(width) = value.strip_prefix("force-tab=") {
        return apply_indent_width(path, line_number, width, IndentStyle::ForceTabs, options);
    }
    if let Some(width) = value.strip_prefix("force-tab-x=") {
        let width = parse_usize(path, line_number, width, "tab width")?;
        validate_range(path, line_number, width, "tab width", 2, 20)?;
        options.indent_style = IndentStyle::ForceTabs;
        options.set_force_tab_width(width);
        return Ok(());
    }

    Err(ConfigError::line(
        path,
        line_number,
        "expected indent=spaces=N, indent=tab=N, indent=force-tab=N, or indent=force-tab-x=N",
    ))
}

fn apply_indent_width(
    path: &Path,
    line_number: usize,
    raw: &str,
    style: IndentStyle,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    let width = parse_usize(path, line_number, raw, "indent width")?;
    validate_range(path, line_number, width, "indent width", 2, 20)?;
    options.set_indent_style_width(style, width);
    Ok(())
}

fn apply_usize_range(
    path: &Path,
    line_number: usize,
    raw: &str,
    label: &str,
    min: usize,
    max: usize,
    apply: impl FnOnce(usize),
) -> Result<(), ConfigError> {
    let value = parse_usize(path, line_number, raw, label)?;
    validate_range(path, line_number, value, label, min, max)?;
    apply(value);
    Ok(())
}

fn parse_usize(
    path: &Path,
    line_number: usize,
    raw: &str,
    label: &str,
) -> Result<usize, ConfigError> {
    raw.parse::<usize>()
        .map_err(|_| ConfigError::line(path, line_number, format!("invalid {label} '{raw}'")))
}

fn validate_range(
    path: &Path,
    line_number: usize,
    value: usize,
    label: &str,
    min: usize,
    max: usize,
) -> Result<(), ConfigError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(ConfigError::line(
            path,
            line_number,
            format!("{label} must be between {min} and {max}"),
        ))
    }
}

fn apply_min_conditional_indent(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.min_conditional_indent = match value {
        "0" => MinConditionalIndent::Zero,
        "1" => MinConditionalIndent::One,
        "2" => MinConditionalIndent::Two,
        "3" => MinConditionalIndent::OneHalf,
        _ => {
            return Err(ConfigError::line(
                path,
                line_number,
                "min conditional indent must be 0, 1, 2, or 3",
            ));
        }
    };
    Ok(())
}

fn apply_pointer_align(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.pointer_align = match value {
        "type" => PointerAlign::Type,
        "middle" => PointerAlign::Middle,
        "name" => PointerAlign::Name,
        _ => {
            return Err(ConfigError::line(
                path,
                line_number,
                "align-pointer must be type, middle, or name",
            ));
        }
    };
    Ok(())
}

fn apply_reference_align(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    options.reference_align = match value {
        "none" => ReferenceAlign::None,
        "type" => ReferenceAlign::Type,
        "middle" => ReferenceAlign::Middle,
        "name" => ReferenceAlign::Name,
        _ => {
            return Err(ConfigError::line(
                path,
                line_number,
                "align-reference must be none, type, middle, or name",
            ));
        }
    };
    Ok(())
}

fn apply_line_end(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    match value {
        "linux" => {
            options.line_ending = LineEnding::Lf;
            Ok(())
        }
        "windows" => {
            options.line_ending = LineEnding::Crlf;
            Ok(())
        }
        "macold" => {
            options.line_ending = LineEnding::Cr;
            Ok(())
        }
        _ => Err(ConfigError::line(
            path,
            line_number,
            format!("unsupported lineend value '{value}'"),
        )),
    }
}

fn apply_mode(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    match value {
        "c" => {
            options.mode = Mode::C;
            Ok(())
        }
        "objc" => {
            options.mode = Mode::ObjC;
            Ok(())
        }
        _ => Err(ConfigError::line(
            path,
            line_number,
            format!("unsupported mode value '{value}'"),
        )),
    }
}

fn apply_access_label(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    let label = value.trim().trim_end_matches(':').trim();
    if label.is_empty() {
        return Err(ConfigError::line(
            path,
            line_number,
            "access-label must not be empty",
        ));
    }
    add_access_label(options, label);
    Ok(())
}

fn add_access_label(options: &mut FormatOptions, label: &str) {
    if !options
        .access_labels
        .iter()
        .any(|existing| existing == label)
    {
        options.access_labels.push(label.to_string());
    }
}

fn apply_control_header(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    let header = value.trim();
    if !is_macro_name(header) {
        return Err(ConfigError::line(
            path,
            line_number,
            "control-header must be an identifier",
        ));
    }
    add_word(&mut options.control_headers, header);
    Ok(())
}

fn apply_non_paren_header(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    let header = value.trim();
    if !is_macro_name(header) {
        return Err(ConfigError::line(
            path,
            line_number,
            "non-paren-header must be an identifier",
        ));
    }
    add_word(&mut options.control_headers, header);
    add_word(&mut options.non_paren_headers, header);
    Ok(())
}

fn add_word(words: &mut Vec<String>, word: &str) {
    if !words.iter().any(|existing| existing == word) {
        words.push(word.to_string());
    }
}

fn apply_macro_block(
    path: &Path,
    line_number: usize,
    value: &str,
    options: &mut FormatOptions,
) -> Result<(), ConfigError> {
    let Some((begin, end)) = value.split_once(':') else {
        return Err(ConfigError::line(
            path,
            line_number,
            "macro-block must be BEGIN:END",
        ));
    };
    let begin = begin.trim();
    let end = end.trim();
    if !is_macro_name(begin) || !is_macro_name(end) {
        return Err(ConfigError::line(
            path,
            line_number,
            "macro-block names must be identifiers",
        ));
    }
    if !options
        .macro_blocks
        .iter()
        .any(|(existing_begin, existing_end)| existing_begin == begin && existing_end == end)
    {
        options
            .macro_blocks
            .push((begin.to_string(), end.to_string()));
    }
    Ok(())
}

fn is_macro_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(lex::is_identifier_start) && chars.all(lex::is_identifier_continue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BraceStyle, CONFIG_FILE_NAME};
    use std::path::PathBuf;

    fn parse_config(source: &str) -> FormatOptions {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        parse_source(&path, source).expect("parse config")
    }

    fn parse_command_line_arg(arg: &str) -> FormatOptions {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[arg.to_string()]).expect("parse command line");
        options
    }

    #[test]
    fn parses_supported_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let source = [
            "style=1tbs",
            "indent=spaces=2",
            "indent-continuation=2",
            "max-continuation-indent=80",
            "min-conditional-indent=3",
            "align-pointer=name",
            "align-reference=none",
            "max-code-length=120",
            "break-after-logical",
            "break-blocks=all",
            "break-closing-braces",
            "attach-namespaces",
            "attach-classes",
            "attach-inlines",
            "attach-extern-c",
            "attach-closing-while",
            "break-elseifs",
            "no-indent-if-after-else",
            "line-between-members=all",
            "break-return-type",
            "attach-return-type-decl",
            "indent-after-parens",
            "indent-switches",
            "indent-cases",
            "indent-labels",
            "indent-preproc-define",
            "indent-preproc-cond",
            "indent-preproc-block",
            "indent-col1-comments",
            "break-one-line-headers",
            "keep-one-line-statements",
            "add-one-line-braces",
            "delete-empty-lines",
            "fill-empty-lines",
            "remove-comment-prefix",
            "convert-tabs",
            "\"access-label=custom section\"",
            "macro-block=BEGIN_BLOCK:END_BLOCK",
            "control-header=FOR_EACH",
            "non-paren-header=FOREVER",
            "lineend=linux",
        ]
        .join("\n")
            + "\n";
        let options = parse_source(&path, &source).expect("parse config");

        assert_eq!(options.brace_style, BraceStyle::OneTrueBrace);
        assert!(options.add_braces);
        assert!(options.add_one_line_braces);
        assert!(!options.remove_braces);
        assert!(!options.break_one_line_blocks);
        assert!(options.break_one_line_headers);
        assert!(!options.break_one_line_statements);
        assert_eq!(options.indent_style, IndentStyle::Spaces);
        assert_eq!(options.indent_width, 2);
        assert_eq!(options.continuation_indent, 2);
        assert_eq!(options.max_continuation_indent, 80);
        assert_eq!(
            options.min_conditional_indent,
            MinConditionalIndent::OneHalf
        );
        assert_eq!(options.pointer_align, PointerAlign::Name);
        assert_eq!(options.reference_align, ReferenceAlign::None);
        assert_eq!(options.max_code_length, Some(120));
        assert!(options.break_after_logical);
        assert!(options.break_blocks);
        assert!(options.break_closing_header_blocks);
        assert!(options.break_closing_braces);
        assert!(options.attach_namespace);
        assert!(options.attach_class);
        assert!(options.attach_inline);
        assert!(options.attach_extern_c);
        assert!(options.attach_closing_while);
        assert!(options.break_else_ifs);
        assert!(options.no_indent_if_after_else);
        assert_eq!(options.line_between_members, LineBetweenMembers::All);
        assert!(options.break_return_type);
        assert!(!options.break_return_type_decl);
        assert!(!options.attach_return_type);
        assert!(options.attach_return_type_decl);
        assert!(options.indent_after_parens);
        assert!(options.indent_switches);
        assert!(options.indent_cases);
        assert!(options.indent_labels);
        assert!(options.indent_preproc_define);
        assert!(options.indent_preproc_conditional);
        assert!(options.indent_preproc_block);
        assert!(options.indent_col1_comments);
        assert!(options.delete_empty_lines);
        assert!(options.empty_line_fill);
        assert!(options.strip_comment_prefix);
        assert!(options.convert_tabs);
        assert_eq!(options.access_labels, ["custom section"]);
        assert_eq!(
            options.macro_blocks,
            [
                (
                    "wxBEGIN_EVENT_TABLE".to_string(),
                    "wxEND_EVENT_TABLE".to_string()
                ),
                (
                    "BEGIN_MESSAGE_MAP".to_string(),
                    "END_MESSAGE_MAP".to_string()
                ),
                ("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())
            ]
        );
        assert_eq!(options.control_headers, ["FOR_EACH", "FOREVER"]);
        assert_eq!(options.non_paren_headers, ["FOREVER"]);
        assert_eq!(options.line_ending, LineEnding::Lf);
    }

    #[test]
    fn parses_custom_access_label_option() {
        let options = parse_command_line_arg("--access-label=custom section");
        assert_eq!(options.access_labels, ["custom section"]);
    }

    #[test]
    fn parses_custom_macro_block_option() {
        let options = parse_command_line_arg("--macro-block=BEGIN_BLOCK:END_BLOCK");
        assert_eq!(
            options.macro_blocks,
            [
                (
                    "wxBEGIN_EVENT_TABLE".to_string(),
                    "wxEND_EVENT_TABLE".to_string()
                ),
                (
                    "BEGIN_MESSAGE_MAP".to_string(),
                    "END_MESSAGE_MAP".to_string()
                ),
                ("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())
            ]
        );
    }

    #[test]
    fn custom_header_and_macro_names_use_formatter_identifier_rules() {
        let options = parse_config(
            "control-header=Συνθήκη\nnon-paren-header=répéter\nmacro-block=НАЧАЛО:КОНЕЦ\n",
        );

        assert_eq!(options.control_headers, ["Συνθήκη", "répéter"]);
        assert_eq!(options.non_paren_headers, ["répéter"]);
        assert_eq!(
            options.macro_blocks.last(),
            Some(&("НАЧАЛО".to_string(), "КОНЕЦ".to_string()))
        );
    }

    #[test]
    fn parses_custom_header_options() {
        let mut options = FormatOptions::default();
        apply_command_line_args(
            &mut options,
            &[
                "--control-header=FOR_EACH".to_string(),
                "--non-paren-header=FOREVER".to_string(),
            ],
        )
        .expect("parse command line");
        assert_eq!(options.control_headers, ["FOR_EACH", "FOREVER"]);
        assert_eq!(options.non_paren_headers, ["FOREVER"]);
    }

    #[test]
    fn parses_utf8_bom_at_start_of_option_file() {
        let options = parse_config("\u{feff}style=allman\n");

        assert_eq!(options.brace_style, BraceStyle::Allman);
    }

    #[test]
    fn parses_bare_cr_option_file_lines() {
        let options = parse_config("indent=spaces=2\rstyle=allman\r");

        assert_eq!(options.indent_width, 2);
        assert_eq!(options.brace_style, BraceStyle::Allman);
    }

    #[test]
    fn parses_option_file_tokens_with_commas_spaces_quotes_and_comments() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let options = parse_source(
            &path,
            "# comment\n'indent=spaces=2', pad-oper pad-header # trailing comment\n\"align-pointer=name\"\n",
        )
        .expect("parse tokenized config");

        assert_eq!(options.indent_width, 2);
        assert!(options.pad_operators);
        assert!(options.pad_header);
        assert_eq!(options.pointer_align, PointerAlign::Name);
    }

    #[test]
    fn parses_command_line_long_short_and_bundled_options() {
        let mut options = FormatOptions::default();
        apply_command_line_args(
            &mut options,
            &[
                "--style=1tbs".to_string(),
                "-s2pH".to_string(),
                "-xC80xL".to_string(),
                "-k3W0".to_string(),
                "-z1".to_string(),
            ],
        )
        .expect("parse command line options");

        assert_eq!(options.brace_style, BraceStyle::OneTrueBrace);
        assert_eq!(options.indent_width, 2);
        assert!(options.pad_operators);
        assert!(options.pad_header);
        assert_eq!(options.max_code_length, Some(80));
        assert!(options.break_after_logical);
        assert_eq!(options.pointer_align, PointerAlign::Name);
        assert_eq!(options.reference_align, ReferenceAlign::None);
        assert_eq!(options.line_ending, LineEnding::Crlf);
    }

    #[test]
    fn command_line_options_override_loaded_config() {
        let mut options = parse_config("indent=spaces=2\n");

        apply_command_line_args(&mut options, &["--indent=spaces=3".to_string()])
            .expect("parse override");

        assert_eq!(options.indent_width, 3);
    }

    #[test]
    fn failed_command_line_batch_leaves_options_unchanged() {
        let mut options = FormatOptions::default();
        let original = options.clone();

        let error = apply_command_line_args(
            &mut options,
            &["--pad-oper".to_string(), "--unknown-option".to_string()],
        )
        .expect_err("invalid option batch must fail");

        assert!(error.to_string().contains("unknown option"));
        assert_eq!(options, original);
    }

    #[test]
    fn rejects_indent_lambda_option() {
        let mut options = FormatOptions::default();
        let command_line_error =
            apply_command_line_args(&mut options, &["--indent-lambda".to_string()])
                .expect_err("indent-lambda must not be accepted on the command line");
        assert_eq!(
            command_line_error.to_string(),
            "<command-line>:1: unknown option 'indent-lambda'"
        );

        let config_error = parse_source(&PathBuf::from(CONFIG_FILE_NAME), "indent-lambda\n")
            .expect_err("indent-lambda must not be accepted from config");
        assert_eq!(
            config_error.to_string(),
            ".cstylerc:1: unknown config key 'indent-lambda'"
        );
    }

    #[test]
    fn defaults_to_none_brace_style_and_accepts_explicit_none() {
        assert_eq!(FormatOptions::default().brace_style, BraceStyle::None);
        assert_eq!(parse_config("style=none\n").brace_style, BraceStyle::None);
    }

    #[test]
    fn later_style_replaces_all_earlier_style_defaults() {
        let allman = parse_config("style=allman\n");

        for earlier in [
            "none",
            "java",
            "kr",
            "stroustrup",
            "whitesmith",
            "vtk",
            "ratliff",
            "gnu",
            "linux",
            "horstmann",
            "1tbs",
            "google",
            "mozilla",
            "webkit",
            "pico",
            "lisp",
        ] {
            let options = parse_config(&format!("style={earlier}\nstyle=allman\n"));
            assert_eq!(options, allman, "style={earlier}");
        }
    }

    #[test]
    fn linux_style_overrides_explicit_conditional_indent_regardless_of_order() {
        for source in [
            "min-conditional-indent=2\nstyle=linux\n",
            "style=linux\nmin-conditional-indent=2\n",
        ] {
            let options = parse_config(source);
            assert_eq!(
                options.min_conditional_indent,
                MinConditionalIndent::OneHalf
            );
        }
    }

    #[test]
    fn linux_style_names_match_the_a8_short_option() {
        let short = parse_config("-A8\n");
        assert_eq!(short.min_conditional_indent, MinConditionalIndent::OneHalf);

        for name in ["linux", "knf"] {
            let named = parse_config(&format!("style={name}\n"));
            assert_eq!(named, short, "style={name}");
        }
    }

    #[test]
    fn parses_retained_long_style_and_value_aliases() {
        assert_eq!(
            parse_config("style=allman\n").brace_style,
            BraceStyle::Allman
        );
        assert_eq!(parse_config("style=bsd\n").brace_style, BraceStyle::Allman);
        assert_eq!(
            parse_config("style=break\n").brace_style,
            BraceStyle::Allman
        );
        assert_eq!(parse_config("style=java\n").brace_style, BraceStyle::Attach);
        assert_eq!(
            parse_config("style=attach\n").brace_style,
            BraceStyle::Attach
        );
        assert_eq!(
            parse_config("style=kr\n").brace_style,
            BraceStyle::OneTrueBrace
        );
        assert_eq!(
            parse_config("style=linux\n").brace_style,
            BraceStyle::OneTrueBrace
        );
        assert_eq!(
            parse_config("style=knf\n").brace_style,
            BraceStyle::OneTrueBrace
        );
        let stroustrup = parse_config("style=stroustrup\n");
        assert_eq!(stroustrup.brace_style, BraceStyle::OneTrueBrace);
        assert!(stroustrup.break_closing_braces);
        assert_eq!(
            parse_config("style=mozilla\n").brace_style,
            BraceStyle::OneTrueBrace
        );
        assert_eq!(
            parse_config("style=1tbs\n").brace_style,
            BraceStyle::OneTrueBrace
        );
        assert_eq!(
            parse_config("style=otbs\n").brace_style,
            BraceStyle::OneTrueBrace
        );
        let whitesmith = parse_config("style=whitesmith\n");
        assert_eq!(whitesmith.brace_style, BraceStyle::Whitesmith);
        assert!(whitesmith.indent_braces);
        assert!(whitesmith.indent_classes);
        assert!(whitesmith.indent_switches);
        assert_eq!(parse_config("style=vtk\n").brace_style, BraceStyle::Vtk);
        assert_eq!(
            parse_config("style=ratliff\n").brace_style,
            BraceStyle::Ratliff
        );
        assert_eq!(
            parse_config("style=banner\n").brace_style,
            BraceStyle::Ratliff
        );
        let gnu = parse_config("style=gnu\n");
        assert_eq!(gnu.brace_style, BraceStyle::Gnu);
        assert!(gnu.indent_blocks);
        let horstmann = parse_config("style=horstmann\n");
        assert_eq!(horstmann.brace_style, BraceStyle::Horstmann);
        assert!(horstmann.indent_switches);
        assert_eq!(
            parse_config("style=run-in\n").brace_style,
            BraceStyle::Horstmann
        );
        let google = parse_config("style=google\n");
        assert_eq!(google.brace_style, BraceStyle::Attach);
        assert_eq!(
            parse_config("style=webkit\n").brace_style,
            BraceStyle::WebKit
        );
        assert!(google.indent_modifiers);
        let pico = parse_config("style=pico\n");
        assert_eq!(pico.brace_style, BraceStyle::Pico);
        assert!(!pico.break_one_line_blocks);
        assert!(!pico.break_one_line_statements);
        let lisp = parse_config("style=lisp\n");
        assert_eq!(lisp.brace_style, BraceStyle::Lisp);
        assert!(!lisp.break_one_line_statements);

        let options = parse_config("indent=spaces\n");
        assert_eq!(options.indent_style, IndentStyle::Spaces);
        assert_eq!(options.indent_width, 4);

        let options = parse_config("indent=tab\n");
        assert_eq!(options.indent_style, IndentStyle::Tabs);
        assert_eq!(options.indent_width, 4);

        let options = parse_config("indent=force-tab\n");
        assert_eq!(options.indent_style, IndentStyle::ForceTabs);
        assert_eq!(options.indent_width, 4);

        let options = parse_config("indent=force-tab-x\n");
        assert_eq!(options.indent_style, IndentStyle::ForceTabs);
        assert_eq!(options.tab_width, 8);

        assert_eq!(parse_config("indent=spaces=2\n").indent_width, 2);
        assert_eq!(parse_config("indent=tab=3\n").indent_width, 3);
        assert_eq!(parse_config("indent=force-tab=4\n").indent_width, 4);
        assert_eq!(parse_config("indent=force-tab-x=6\n").tab_width, 6);

        let options = parse_config("indent=force-tab=3\n");
        assert_eq!(options.indent_width, 3);
        assert_eq!(options.tab_width, 3);

        let options = parse_config("indent=force-tab=8\n");
        assert_eq!(options.indent_width, 8);
        assert_eq!(options.tab_width, 8);

        for source in [
            "indent=force-tab-x=6\nindent=force-tab=3\n",
            "indent=force-tab=3\nindent=force-tab-x=6\n",
        ] {
            let options = parse_config(source);
            assert_eq!(options.indent_width, 3);
            assert_eq!(options.tab_width, 6);
        }

        let tabs = parse_config("indent=force-tab-x=2\nindent=tab=6\n");
        assert_eq!(tabs.indent_style, IndentStyle::Tabs);
        assert_eq!(tabs.indent_width, 6);
        assert_eq!(tabs.tab_width, 6);

        let spaces = parse_config("indent=force-tab-x=6\nindent=spaces=2\n");
        assert_eq!(spaces.indent_style, IndentStyle::Spaces);
        assert_eq!(spaces.indent_width, 2);
        assert_eq!(spaces.tab_width, 2);

        let tabs = parse_config("indent=force-tab-x=2\nindent=tab\n");
        assert_eq!(tabs.indent_style, IndentStyle::Tabs);
        assert_eq!(tabs.indent_width, 4);
        assert_eq!(tabs.tab_width, 4);

        let spaces = parse_config("indent=force-tab-x=6\nindent=spaces\n");
        assert_eq!(spaces.indent_style, IndentStyle::Spaces);
        assert_eq!(spaces.indent_width, 4);
        assert_eq!(spaces.tab_width, 4);

        assert_eq!(
            parse_config("indent-continuation=2\n").continuation_indent,
            2
        );
        assert_eq!(
            parse_config("max-continuation-indent=80\n").max_continuation_indent,
            80
        );
        assert_eq!(
            parse_config("min-conditional-indent=0\n").min_conditional_indent,
            MinConditionalIndent::Zero
        );
        assert_eq!(
            parse_config("min-conditional-indent=1\n").min_conditional_indent,
            MinConditionalIndent::One
        );
        assert_eq!(
            parse_config("min-conditional-indent=2\n").min_conditional_indent,
            MinConditionalIndent::Two
        );
        assert_eq!(
            parse_config("min-conditional-indent=3\n").min_conditional_indent,
            MinConditionalIndent::OneHalf
        );
        assert_eq!(
            parse_config("align-pointer=type\n").pointer_align,
            PointerAlign::Type
        );
        assert_eq!(
            parse_config("align-pointer=middle\n").pointer_align,
            PointerAlign::Middle
        );
        assert_eq!(
            parse_config("align-pointer=name\n").pointer_align,
            PointerAlign::Name
        );
        assert_eq!(
            parse_config("align-reference=none\n").reference_align,
            ReferenceAlign::None
        );
        assert_eq!(
            parse_config("align-reference=type\n").reference_align,
            ReferenceAlign::Type
        );
        assert_eq!(
            parse_config("align-reference=middle\n").reference_align,
            ReferenceAlign::Middle
        );
        assert_eq!(
            parse_config("align-reference=name\n").reference_align,
            ReferenceAlign::Name
        );
        assert_eq!(
            parse_config("max-code-length=120\n").max_code_length,
            Some(120)
        );
        assert_eq!(
            parse_config("lineend=windows\n").line_ending,
            LineEnding::Crlf
        );
        assert_eq!(parse_config("lineend=linux\n").line_ending, LineEnding::Lf);
        assert_eq!(parse_config("lineend=macold\n").line_ending, LineEnding::Cr);
    }

    #[test]
    fn parses_retained_long_flag_aliases() {
        assert!(parse_config("indent-after-parens\n").indent_after_parens);
        assert!(parse_config("indent-braces\n").indent_braces);
        assert!(parse_config("indent-blocks\n").indent_blocks);
        assert!(parse_config("indent-switches\n").indent_switches);
        assert!(parse_config("indent-cases\n").indent_cases);
        assert!(parse_config("indent-labels\n").indent_labels);
        assert!(parse_config("indent-classes\n").indent_classes);
        assert!(parse_config("indent-modifiers\n").indent_modifiers);
        assert!(parse_config("indent-preprocessor\n").indent_preproc_define);
        assert!(parse_config("indent-preproc-define\n").indent_preproc_define);
        assert!(parse_config("indent-preproc-cond\n").indent_preproc_conditional);
        assert!(parse_config("indent-preproc-block\n").indent_preproc_block);
        assert!(parse_config("indent-namespaces\n").indent_namespaces);
        assert!(parse_config("indent-col1-comments\n").indent_col1_comments);
        assert!(parse_config("delete-empty-lines\n").delete_empty_lines);
        assert!(parse_config("fill-empty-lines\n").empty_line_fill);
        assert!(parse_config("remove-comment-prefix\n").strip_comment_prefix);
        assert!(parse_config("convert-tabs\n").convert_tabs);
        assert!(parse_config("close-templates\n").close_templates);
        assert!(parse_config("break-one-line-headers\n").break_one_line_headers);
        assert!(!parse_config("keep-one-line-blocks\n").break_one_line_blocks);
        assert!(!parse_config("keep-one-line-statements\n").break_one_line_statements);
        assert!(parse_config("add-braces\n").add_braces);

        let options = parse_config("add-one-line-braces\n");
        assert!(options.add_one_line_braces);
        assert!(!options.break_one_line_blocks);

        assert!(parse_config("remove-braces\n").remove_braces);
        assert!(parse_config("pad-oper\n").pad_operators);
        assert!(parse_config("pad-comma\n").pad_commas);

        let options = parse_config("pad-paren\n");
        assert!(options.pad_parens_outside);
        assert!(options.pad_parens_inside);

        assert!(parse_config("pad-paren-out\n").pad_parens_outside);
        assert!(parse_config("pad-first-paren-out\n").pad_first_paren_outside);
        assert!(parse_config("pad-paren-in\n").pad_parens_inside);
        assert!(parse_config("pad-header\n").pad_header);
        assert!(parse_config("unpad-paren\n").unpad_parens);
        assert!(parse_config("break-after-logical\n").break_after_logical);
        assert!(parse_config("break-blocks\n").break_blocks);

        let options = parse_config("break-blocks=all\n");
        assert!(options.break_blocks);
        assert!(options.break_closing_header_blocks);

        assert!(parse_config("break-closing-braces\n").break_closing_braces);
        assert!(parse_config("attach-namespaces\n").attach_namespace);
        assert!(parse_config("attach-namespace\n").attach_namespace);
        assert!(parse_config("attach-classes\n").attach_class);
        assert!(parse_config("attach-class\n").attach_class);
        assert!(parse_config("attach-inlines\n").attach_inline);
        assert!(parse_config("attach-inline\n").attach_inline);
        assert!(parse_config("attach-extern-c\n").attach_extern_c);
        assert!(parse_config("attach-closing-while\n").attach_closing_while);
        assert!(parse_config("break-elseifs\n").break_else_ifs);
        assert!(parse_config("no-indent-if-after-else\n").no_indent_if_after_else);
        assert!(parse_command_line_arg("--no-indent-if-after-else").no_indent_if_after_else);
        assert!(
            parse_source(
                &PathBuf::from(CONFIG_FILE_NAME),
                "no-indent-if-after-else=true\n"
            )
            .is_err()
        );
        assert_eq!(
            parse_config("line-between-members\n").line_between_members,
            LineBetweenMembers::Members
        );
        assert_eq!(
            parse_config("line-between-members=all\n").line_between_members,
            LineBetweenMembers::All
        );
        assert_eq!(
            parse_command_line_arg("--line-between-members").line_between_members,
            LineBetweenMembers::Members
        );
        assert_eq!(
            parse_command_line_arg("--line-between-members=all").line_between_members,
            LineBetweenMembers::All
        );
        assert!(
            parse_source(
                &PathBuf::from(CONFIG_FILE_NAME),
                "line-between-members=fields\n"
            )
            .is_err()
        );
        assert!(parse_config("break-return-type\n").break_return_type);
        assert!(parse_config("break-return-type-decl\n").break_return_type_decl);
        assert!(parse_config("attach-return-type\n").attach_return_type);
        assert!(parse_config("attach-return-type-decl\n").attach_return_type_decl);
    }

    #[test]
    fn parses_deprecated_option_aliases() {
        assert_eq!(parse_config("style=ansi\n").brace_style, BraceStyle::Allman);
        assert_eq!(parse_config("style=python\n").brace_style, BraceStyle::Lisp);
        assert!(parse_config("break-closing-brackets\n").break_closing_braces);
        assert!(parse_config("add-brackets\n").add_braces);

        let options = parse_config("add-one-line-brackets\n");
        assert!(options.add_one_line_braces);
        assert!(!options.break_one_line_blocks);

        assert!(parse_config("remove-brackets\n").remove_braces);
        assert_eq!(
            parse_config("max-instatement-indent=80\n").max_continuation_indent,
            80
        );
    }

    #[test]
    fn parses_retained_short_style_and_value_aliases() {
        assert_eq!(
            parse_command_line_arg("-A1").brace_style,
            BraceStyle::Allman
        );
        assert_eq!(
            parse_command_line_arg("-A2").brace_style,
            BraceStyle::Attach
        );
        assert_eq!(
            parse_command_line_arg("-A3").brace_style,
            BraceStyle::OneTrueBrace
        );
        assert!(parse_command_line_arg("-A4").break_closing_braces);
        assert_eq!(
            parse_command_line_arg("-A8").brace_style,
            BraceStyle::OneTrueBrace
        );
        assert_eq!(
            parse_command_line_arg("-A10").brace_style,
            BraceStyle::OneTrueBrace
        );
        assert_eq!(
            parse_command_line_arg("-A16").brace_style,
            BraceStyle::OneTrueBrace
        );
        assert_eq!(
            parse_command_line_arg("-A5").brace_style,
            BraceStyle::Whitesmith
        );
        assert_eq!(
            parse_command_line_arg("-A6").brace_style,
            BraceStyle::Ratliff
        );
        assert_eq!(parse_command_line_arg("-A7").brace_style, BraceStyle::Gnu);
        assert_eq!(
            parse_command_line_arg("-A9").brace_style,
            BraceStyle::Horstmann
        );
        assert_eq!(parse_command_line_arg("-A11").brace_style, BraceStyle::Pico);
        assert_eq!(parse_command_line_arg("-A12").brace_style, BraceStyle::Lisp);
        assert_eq!(
            parse_command_line_arg("-A14").brace_style,
            BraceStyle::Attach
        );
        assert_eq!(parse_command_line_arg("-A15").brace_style, BraceStyle::Vtk);
        assert_eq!(
            parse_command_line_arg("-A17").brace_style,
            BraceStyle::WebKit
        );

        let options = parse_command_line_arg("-s");
        assert_eq!(options.indent_style, IndentStyle::Spaces);
        assert_eq!(options.indent_width, 4);
        assert_eq!(parse_command_line_arg("-s2").indent_width, 2);

        let options = parse_command_line_arg("-t");
        assert_eq!(options.indent_style, IndentStyle::Tabs);
        assert_eq!(options.indent_width, 4);
        assert_eq!(parse_command_line_arg("-t3").indent_width, 3);

        let options = parse_command_line_arg("-T");
        assert_eq!(options.indent_style, IndentStyle::ForceTabs);
        assert_eq!(options.indent_width, 4);
        assert_eq!(parse_command_line_arg("-T4").indent_width, 4);

        let options = parse_command_line_arg("-xT");
        assert_eq!(options.indent_style, IndentStyle::ForceTabs);
        assert_eq!(options.tab_width, 8);
        assert_eq!(parse_command_line_arg("-xT6").tab_width, 6);

        assert_eq!(parse_command_line_arg("-xt").continuation_indent, 1);
        assert_eq!(parse_command_line_arg("-xt2").continuation_indent, 2);
        assert_eq!(parse_command_line_arg("-M").max_continuation_indent, 40);
        assert_eq!(parse_command_line_arg("-M80").max_continuation_indent, 80);
        assert_eq!(
            parse_command_line_arg("-m").min_conditional_indent,
            MinConditionalIndent::Two
        );
        assert_eq!(
            parse_command_line_arg("-m0").min_conditional_indent,
            MinConditionalIndent::Zero
        );
        assert_eq!(
            parse_command_line_arg("-m1").min_conditional_indent,
            MinConditionalIndent::One
        );
        assert_eq!(
            parse_command_line_arg("-m2").min_conditional_indent,
            MinConditionalIndent::Two
        );
        assert_eq!(
            parse_command_line_arg("-m3").min_conditional_indent,
            MinConditionalIndent::OneHalf
        );
        assert_eq!(
            parse_command_line_arg("-k1").pointer_align,
            PointerAlign::Type
        );
        assert_eq!(
            parse_command_line_arg("-k2").pointer_align,
            PointerAlign::Middle
        );
        assert_eq!(
            parse_command_line_arg("-k3").pointer_align,
            PointerAlign::Name
        );
        assert_eq!(
            parse_command_line_arg("-W0").reference_align,
            ReferenceAlign::None
        );
        assert_eq!(
            parse_command_line_arg("-W1").reference_align,
            ReferenceAlign::Type
        );
        assert_eq!(
            parse_command_line_arg("-W2").reference_align,
            ReferenceAlign::Middle
        );
        assert_eq!(
            parse_command_line_arg("-W3").reference_align,
            ReferenceAlign::Name
        );
        assert_eq!(parse_command_line_arg("-xC").max_code_length, Some(50));
        assert_eq!(parse_command_line_arg("-xC80").max_code_length, Some(80));
        assert_eq!(parse_command_line_arg("-z1").line_ending, LineEnding::Crlf);
        assert_eq!(parse_command_line_arg("-z2").line_ending, LineEnding::Lf);
        assert_eq!(parse_command_line_arg("-z3").line_ending, LineEnding::Cr);
    }

    #[test]
    fn parses_retained_short_flag_aliases() {
        assert!(parse_command_line_arg("-S").indent_switches);
        assert!(parse_command_line_arg("-K").indent_cases);
        assert!(parse_command_line_arg("-L").indent_labels);
        assert!(parse_command_line_arg("-C").indent_classes);
        assert!(parse_command_line_arg("-xG").indent_modifiers);
        assert!(parse_command_line_arg("-xW").indent_preproc_block);
        assert!(parse_command_line_arg("-w").indent_preproc_define);
        assert!(parse_command_line_arg("-xw").indent_preproc_conditional);
        assert!(parse_command_line_arg("-N").indent_namespaces);
        assert!(parse_command_line_arg("-Y").indent_col1_comments);
        assert!(parse_command_line_arg("-xe").delete_empty_lines);
        assert!(parse_command_line_arg("-E").empty_line_fill);
        assert!(parse_command_line_arg("-xp").strip_comment_prefix);
        assert!(parse_command_line_arg("-c").convert_tabs);
        assert!(parse_command_line_arg("-xy").close_templates);
        assert!(parse_command_line_arg("-xb").break_one_line_headers);
        assert!(!parse_command_line_arg("-O").break_one_line_blocks);
        assert!(!parse_command_line_arg("-o").break_one_line_statements);
        assert!(parse_command_line_arg("-j").add_braces);

        let options = parse_command_line_arg("-J");
        assert!(options.add_one_line_braces);
        assert!(!options.break_one_line_blocks);

        assert!(parse_command_line_arg("-xj").remove_braces);
        assert!(parse_command_line_arg("-p").pad_operators);
        assert!(parse_command_line_arg("-xg").pad_commas);

        let options = parse_command_line_arg("-P");
        assert!(options.pad_parens_outside);
        assert!(options.pad_parens_inside);

        assert!(parse_command_line_arg("-d").pad_parens_outside);
        assert!(parse_command_line_arg("-xd").pad_first_paren_outside);
        assert!(parse_command_line_arg("-D").pad_parens_inside);
        assert!(parse_command_line_arg("-H").pad_header);
        assert!(parse_command_line_arg("-U").unpad_parens);
        assert!(parse_command_line_arg("-xL").break_after_logical);
        assert!(parse_command_line_arg("-f").break_blocks);

        let options = parse_command_line_arg("-F");
        assert!(options.break_blocks);
        assert!(options.break_closing_header_blocks);

        assert!(parse_command_line_arg("-y").break_closing_braces);
        assert!(parse_command_line_arg("-xn").attach_namespace);
        assert!(parse_command_line_arg("-xc").attach_class);
        assert!(parse_command_line_arg("-xl").attach_inline);
        assert!(parse_command_line_arg("-xk").attach_extern_c);
        assert!(parse_command_line_arg("-xV").attach_closing_while);
        assert!(parse_command_line_arg("-e").break_else_ifs);
        assert!(parse_command_line_arg("-xU").indent_after_parens);
        assert!(parse_command_line_arg("-xB").break_return_type);
        assert!(parse_command_line_arg("-xD").break_return_type_decl);
        assert!(parse_command_line_arg("-xf").attach_return_type);
        assert!(parse_command_line_arg("-xh").attach_return_type_decl);
    }

    #[test]
    fn parses_tab_indentation_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let tab_options = parse_source(&path, "indent=tab=8\n").expect("parse tab config");
        assert_eq!(tab_options.indent_style, IndentStyle::Tabs);
        assert_eq!(tab_options.indent_width, 8);
        assert_eq!(tab_options.tab_width, 8);

        let force_options =
            parse_source(&path, "indent=force-tab=3\n").expect("parse force tab config");
        assert_eq!(force_options.indent_style, IndentStyle::ForceTabs);
        assert_eq!(force_options.indent_width, 3);

        let force_x_options =
            parse_source(&path, "indent=force-tab-x=6\n").expect("parse force tab x config");
        assert_eq!(force_x_options.indent_style, IndentStyle::ForceTabs);
        assert_eq!(force_x_options.tab_width, 6);
    }

    #[test]
    fn parses_padding_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let options = parse_source(
            &path,
            "pad-oper\npad-comma\npad-paren\npad-first-paren-out\npad-header\nunpad-paren\n",
        )
        .expect("parse padding config");

        assert!(options.pad_operators);
        assert!(options.pad_commas);
        assert!(options.pad_parens_outside);
        assert!(options.pad_first_paren_outside);
        assert!(options.pad_parens_inside);
        assert!(options.pad_header);
        assert!(options.unpad_parens);
    }

    #[test]
    fn parses_individual_paren_padding_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let outside = parse_source(&path, "pad-paren-out\n").expect("parse outside paren");
        assert!(outside.pad_parens_outside);
        assert!(!outside.pad_parens_inside);

        let inside = parse_source(&path, "pad-paren-in\n").expect("parse inside paren");
        assert!(!inside.pad_parens_outside);
        assert!(inside.pad_parens_inside);
    }

    #[test]
    fn parses_pointer_and_reference_alignment_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let options = parse_source(&path, "align-pointer=type\nalign-reference=middle\n")
            .expect("parse alignment config");

        assert_eq!(options.pointer_align, PointerAlign::Type);
        assert_eq!(options.reference_align, ReferenceAlign::Middle);
    }

    #[test]
    fn rejects_bad_alignment_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let error =
            parse_source(&path, "align-pointer=none\n").expect_err("pointer none must fail");
        assert_eq!(
            error.to_string(),
            ".cstylerc:1: align-pointer must be type, middle, or name"
        );

        let error = parse_source(&path, "align-reference=left\n")
            .expect_err("bad reference alignment must fail");
        assert_eq!(
            error.to_string(),
            ".cstylerc:1: align-reference must be none, type, middle, or name"
        );
    }

    #[test]
    fn parses_line_splitting_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let options = parse_source(
            &path,
            "max-code-length=50\nbreak-after-logical\nattach-return-type\nbreak-return-type-decl\n",
        )
        .expect("parse line splitting config");

        assert_eq!(options.max_code_length, Some(50));
        assert!(options.break_after_logical);
        assert!(options.attach_return_type);
        assert!(options.break_return_type_decl);
    }

    #[test]
    fn break_return_type_options_override_attach_regardless_of_order() {
        for source in [
            "break-return-type\nattach-return-type\n",
            "attach-return-type\nbreak-return-type\n",
        ] {
            let options = parse_config(source);
            assert!(options.break_return_type);
            assert!(!options.attach_return_type);
        }

        for source in [
            "break-return-type-decl\nattach-return-type-decl\n",
            "attach-return-type-decl\nbreak-return-type-decl\n",
        ] {
            let options = parse_config(source);
            assert!(options.break_return_type_decl);
            assert!(!options.attach_return_type_decl);
        }
    }

    #[test]
    fn rejects_out_of_range_indent_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let error = parse_source(&path, "indent=spaces=1\n").expect_err("indent must fail");
        assert_eq!(
            error.to_string(),
            ".cstylerc:1: indent width must be between 2 and 20"
        );

        let error = parse_source(&path, "indent-continuation=5\n")
            .expect_err("continuation indent must fail");
        assert_eq!(
            error.to_string(),
            ".cstylerc:1: continuation indent must be between 0 and 4"
        );

        let error =
            parse_source(&path, "max-code-length=49\n").expect_err("max code length must fail");
        assert_eq!(
            error.to_string(),
            ".cstylerc:1: max code length must be between 50 and 200"
        );

        let mut options = FormatOptions::default();
        let error = apply_command_line_args(&mut options, &["-xC49".to_string()])
            .expect_err("short max code length must use the same range");
        assert!(error.to_string().contains("between 50 and 200"));
    }

    #[test]
    fn add_braces_overrides_remove_braces_regardless_of_order() {
        let path = PathBuf::from(CONFIG_FILE_NAME);

        let options =
            parse_source(&path, "add-braces\nremove-braces\n").expect("add then remove must parse");
        assert!(options.add_braces);
        assert!(!options.remove_braces);

        let options =
            parse_source(&path, "remove-braces\nadd-braces\n").expect("remove then add must parse");
        assert!(options.add_braces);
        assert!(!options.remove_braces);

        let options = parse_source(&path, "remove-braces\nadd-one-line-braces\n")
            .expect("remove then add-one-line must parse");
        assert!(options.add_one_line_braces);
        assert!(!options.remove_braces);

        let options = parse_source(&path, "remove-braces\n").expect("remove alone must parse");
        assert!(options.remove_braces);
    }

    #[test]
    fn parses_block_break_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let options = parse_source(&path, "break-blocks\nbreak-closing-braces\nbreak-elseifs\n")
            .expect("parse block break config");

        assert!(options.break_blocks);
        assert!(!options.break_closing_header_blocks);
        assert!(options.break_closing_braces);
        assert!(options.break_else_ifs);

        let options = parse_source(&path, "break-blocks=all\n").expect("parse all block break");
        assert!(options.break_blocks);
        assert!(options.break_closing_header_blocks);
    }

    #[test]
    fn rejects_bad_block_break_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let error = parse_source(&path, "break-blocks=none\n")
            .expect_err("bad break blocks value must fail");
        assert_eq!(
            error.to_string(),
            ".cstylerc:1: break-blocks value must be all"
        );
    }

    #[test]
    fn parses_cleanup_preprocessor_and_line_end_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let options = parse_source(
            &path,
            "indent-preproc-block\nindent-col1-comments\ndelete-empty-lines\nremove-comment-prefix\nlineend=windows\n",
        )
        .expect("parse cleanup config");

        assert!(options.indent_preproc_block);
        assert!(options.indent_col1_comments);
        assert!(options.delete_empty_lines);
        assert!(options.strip_comment_prefix);
        assert_eq!(options.line_ending, LineEnding::Crlf);
        assert_eq!(options.line_break(), "\r\n");

        let options = parse_source(&path, "lineend=macold\n").expect("parse macold lineend");
        assert_eq!(options.line_ending, LineEnding::Cr);
        assert_eq!(options.line_break(), "\r");
    }

    #[test]
    fn accepts_c_mode_for_legacy_compatibility() {
        assert_eq!(parse_config("mode=c\n").mode, Mode::C);
        assert_eq!(parse_command_line_arg("--mode=c").mode, Mode::C);
        assert_eq!(parse_command_line_arg("mode=c").mode, Mode::C);
    }

    #[test]
    fn accepts_objc_mode_as_distinct_priority_mode() {
        assert_eq!(parse_config("mode=objc\n").mode, Mode::ObjC);
        assert_eq!(parse_command_line_arg("--mode=objc").mode, Mode::ObjC);
        assert_eq!(parse_command_line_arg("mode=objc").mode, Mode::ObjC);
    }

    #[test]
    fn rejects_non_c_mode_values() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let error = parse_source(&path, "mode=java\n").expect_err("non-C mode must fail");

        assert_eq!(
            error.to_string(),
            ".cstylerc:1: unsupported mode value 'java'"
        );
    }

    #[test]
    fn rejects_bare_short_option_marker() {
        let mut options = FormatOptions::default();
        let error = apply_command_line_args(&mut options, &["-".to_string()])
            .expect_err("bare short option marker must fail");

        assert_eq!(error.to_string(), "<command-line>:1: unknown option '-'");
    }

    #[test]
    fn rejects_removed_cli_options() {
        for option in [
            "--exclude=skip.c",
            "--ignore-exclude-errors",
            "-i",
            "--ignore-exclude-errors-x",
            "-xi",
            "--dry-run",
            "--verbose",
            "-v",
            "--suffix=.orig",
            "--preserve-date",
            "-Z",
            "--stdin=input.c",
            "--stdout=output.c",
            "--errors-to-stdout",
            "-X",
            "--project",
            "--project=none",
            "--project=custom.rc",
            "--completions",
            "--completions=bash",
        ] {
            let mut options = FormatOptions::default();
            let error = apply_command_line_args(&mut options, &[option.to_string()])
                .expect_err("removed option must fail");

            assert!(
                error.to_string().contains("unknown option"),
                "error for {option}: {error}"
            );
        }
    }

    #[test]
    fn config_source_rejects_console_options() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        for option in [
            "dry-run",
            "error-on-changes",
            "accept-empty-list",
            "recursive",
            "r",
            "R",
            "exclude=skip.c",
            "ignore-exclude-errors",
            "i",
            "ignore-exclude-errors-x",
            "xi",
            "suffix=.bak",
            "suffix=none",
            "n",
            "preserve-date",
            "Z",
            "formatted",
            "Q",
            "quiet",
            "q",
            "verbose",
            "v",
            "errors-to-stdout",
            "X",
        ] {
            let source = format!("indent=spaces=2\n{option}\n");
            let error =
                parse_source(&path, &source).expect_err("console option in config must fail");

            assert!(
                error.to_string().contains("unknown config key"),
                "{option}: {error}"
            );
        }
    }

    #[test]
    fn parses_objective_c_padding_options() {
        assert!(parse_config("pad-method-prefix\n").pad_method_prefix);
        assert!(parse_config("unpad-method-prefix\n").unpad_method_prefix);
        assert!(parse_config("pad-return-type\n").pad_return_type);
        assert!(parse_config("unpad-return-type\n").unpad_return_type);
        assert!(parse_config("pad-param-type\n").pad_param_type);
        assert!(parse_config("unpad-param-type\n").unpad_param_type);
        assert!(parse_config("align-method-colon\n").align_method_colon);
        assert_eq!(
            parse_config("pad-method-colon=none\n").pad_method_colon,
            ObjCColonPad::None
        );
        assert_eq!(
            parse_config("pad-method-colon=all\n").pad_method_colon,
            ObjCColonPad::All
        );
        assert_eq!(
            parse_config("pad-method-colon=after\n").pad_method_colon,
            ObjCColonPad::After
        );
        assert_eq!(
            parse_config("pad-method-colon=before\n").pad_method_colon,
            ObjCColonPad::Before
        );
        assert_eq!(
            FormatOptions::default().pad_method_colon,
            ObjCColonPad::NoChange
        );

        assert!(parse_command_line_arg("-xQ").pad_method_prefix);
        assert!(parse_command_line_arg("-xR").unpad_method_prefix);
        assert!(parse_command_line_arg("-xq").pad_return_type);
        assert!(parse_command_line_arg("-xr").unpad_return_type);
        assert!(parse_command_line_arg("-xS").pad_param_type);
        assert!(parse_command_line_arg("-xs").unpad_param_type);
        assert!(parse_command_line_arg("-xM").align_method_colon);
        assert_eq!(
            parse_command_line_arg("-xP0").pad_method_colon,
            ObjCColonPad::None
        );
        assert_eq!(
            parse_command_line_arg("-xP1").pad_method_colon,
            ObjCColonPad::All
        );
        assert_eq!(
            parse_command_line_arg("-xP2").pad_method_colon,
            ObjCColonPad::After
        );
        assert_eq!(
            parse_command_line_arg("-xP3").pad_method_colon,
            ObjCColonPad::Before
        );
    }

    #[test]
    fn rejects_bad_method_colon_pad() {
        let path = PathBuf::from(CONFIG_FILE_NAME);
        let error = parse_source(&path, "pad-method-colon=middle\n")
            .expect_err("bad method colon pad must fail");
        assert_eq!(
            error.to_string(),
            ".cstylerc:1: pad-method-colon must be none, all, after, or before"
        );
    }
}
