use crate::config::FormatOptions;
use crate::formatter;
use crate::io as cstyle_io;
use std::io;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Formatter {
    options: FormatOptions,
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            options: FormatOptions::default(),
        }
    }

    pub fn with_options(options: FormatOptions) -> Self {
        Self { options }
    }

    pub fn options(&self) -> &FormatOptions {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut FormatOptions {
        &mut self.options
    }

    pub fn format(&self, source: &str) -> String {
        format(source, &self.options)
    }

    pub fn format_bytes(&self, input: &[u8]) -> io::Result<Vec<u8>> {
        format_bytes(input, &self.options)
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn format(source: &str, options: &FormatOptions) -> String {
    formatter::format_c(source, options)
}

pub fn format_bytes(input: &[u8], options: &FormatOptions) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    cstyle_io::format_reader_to_writer(input, &mut output, options)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IndentStyle, MinConditionalIndent, StylePreset};

    #[test]
    fn formatter_facade_applies_style_and_formats_text() {
        let mut formatter = Formatter::new();
        formatter.options_mut().set_style(StylePreset::Google);

        assert_eq!(
            formatter.format("class C{public:int x;};\n"),
            "class C {\n  public:\n    int x;\n};\n"
        );
    }

    #[test]
    fn formatter_facade_styles_match_parsed_options() {
        for (style, option) in [
            (StylePreset::None, "--style=none"),
            (StylePreset::Allman, "--style=allman"),
            (StylePreset::Java, "--style=java"),
            (StylePreset::Kr, "--style=kr"),
            (StylePreset::Stroustrup, "--style=stroustrup"),
            (StylePreset::Whitesmith, "--style=whitesmith"),
            (StylePreset::Vtk, "--style=vtk"),
            (StylePreset::Ratliff, "--style=ratliff"),
            (StylePreset::Gnu, "--style=gnu"),
            (StylePreset::Linux, "--style=linux"),
            (StylePreset::Horstmann, "--style=horstmann"),
            (StylePreset::OneTrueBrace, "--style=1tbs"),
            (StylePreset::Google, "--style=google"),
            (StylePreset::Mozilla, "--style=mozilla"),
            (StylePreset::WebKit, "--style=webkit"),
            (StylePreset::Pico, "--style=pico"),
            (StylePreset::Lisp, "--style=lisp"),
        ] {
            let mut expected = FormatOptions::default();
            crate::config::apply_command_line_args(&mut expected, &[option.to_string()])
                .expect("parse style option");

            let mut formatter = Formatter::new();
            formatter.options_mut().set_style(style);

            assert_eq!(formatter.options(), &expected, "{option}");
        }
    }

    #[test]
    fn explicit_api_option_survives_a_later_unrelated_style() {
        let mut formatter = Formatter::new();
        formatter.options_mut().set_style(StylePreset::Linux);
        formatter
            .options_mut()
            .set_min_conditional_indent(MinConditionalIndent::Zero);
        formatter.options_mut().set_style(StylePreset::Allman);

        assert_eq!(
            formatter.options().min_conditional_indent,
            MinConditionalIndent::Zero
        );
    }

    #[test]
    fn setting_a_new_style_replaces_earlier_style_defaults() {
        let mut formatter = Formatter::new();
        formatter.options_mut().set_style(StylePreset::Google);
        formatter.options_mut().set_style(StylePreset::Allman);
        let mut expected = FormatOptions::default();
        crate::config::apply_command_line_args(&mut expected, &["--style=allman".to_string()])
            .expect("parse style option");

        assert_eq!(formatter.options(), &expected);
    }

    #[test]
    fn none_style_clears_an_existing_brace_style() {
        let mut formatter = Formatter::new();
        formatter.options_mut().set_style(StylePreset::Allman);
        formatter.options_mut().set_style(StylePreset::None);

        assert_eq!(
            formatter.options().brace_style,
            crate::config::BraceStyle::None
        );
    }

    #[test]
    fn indentation_setters_select_one_explicit_style() {
        let mut formatter = Formatter::new();

        formatter.options_mut().set_tab_indentation(3);
        assert_eq!(formatter.options().indent_style, IndentStyle::Tabs);
        assert_eq!(formatter.options().indent_width, 3);
        assert_eq!(formatter.options().tab_width, 3);

        formatter.options_mut().set_force_tab_indentation(5);
        assert_eq!(formatter.options().indent_style, IndentStyle::ForceTabs);
        assert_eq!(formatter.options().indent_width, 5);
        assert_eq!(formatter.options().tab_width, 5);

        formatter.options_mut().set_force_tab_width(8);
        assert_eq!(formatter.options().indent_style, IndentStyle::ForceTabs);
        assert_eq!(formatter.options().indent_width, 5);
        assert_eq!(formatter.options().tab_width, 8);
    }

    #[test]
    fn format_bytes_facade_preserves_encoding_contract() {
        let output =
            format_bytes(b"int f(){return 0;}\n", &FormatOptions::default()).expect("format bytes");

        assert_eq!(
            std::str::from_utf8(&output).expect("utf8 output"),
            "int f() {\n    return 0;\n}\n"
        );
    }
}
