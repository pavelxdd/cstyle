use crate::config::{FormatOptions, LineEnding};
use crate::formatter;
use crate::source::encoding::DecodedSource;
use crate::source::line_endings::ObservedLineEnding;
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
    formatter::format(source, options)
}

/// Formats encoded source bytes, preserving the input encoding, the observed
/// line endings when `options.line_ending` is `Preserve`, and a missing final
/// line break.
pub fn format_bytes(input: &[u8], options: &FormatOptions) -> io::Result<Vec<u8>> {
    format_owned_bytes(input.to_vec(), options)
}

/// `format_bytes` of input it takes, so the source is not copied.
pub(crate) fn format_owned_bytes(input: Vec<u8>, options: &FormatOptions) -> io::Result<Vec<u8>> {
    if u32::try_from(input.len()).is_err() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input is larger than 4 GiB",
        ));
    }
    let mut source = DecodedSource::from_vec(input)?;
    let options = resolve_preserved_line_ending(options, source.observed_line_ending());
    let mut output = formatter::format_owned(source.take_text(), &options);
    if !source.had_final_line_break() {
        let line_break = options.line_break();
        if output.ends_with(line_break) {
            output.truncate(output.len() - line_break.len());
        }
    }
    Ok(source.encode(&output))
}

fn resolve_preserved_line_ending(
    options: &FormatOptions,
    observed: ObservedLineEnding,
) -> FormatOptions {
    let mut options = options.clone();
    if options.line_ending == LineEnding::Preserve {
        options.line_ending = match observed {
            ObservedLineEnding::CrLf => LineEnding::Crlf,
            ObservedLineEnding::Cr => LineEnding::Cr,
            ObservedLineEnding::None | ObservedLineEnding::Lf => LineEnding::Lf,
        };
    }
    options
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

    fn utf16le_with_bom(text: &str) -> Vec<u8> {
        let mut output = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            output.extend_from_slice(&unit.to_le_bytes());
        }
        output
    }

    fn decode_utf16le_with_bom(bytes: &[u8]) -> String {
        let body = bytes.strip_prefix(&[0xFF, 0xFE]).expect("UTF-16LE BOM");
        let units = body
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).expect("UTF-16LE output")
    }

    #[test]
    fn preserves_utf8_bom_when_formatting_reader() {
        let mut input = vec![0xEF, 0xBB, 0xBF];
        input.extend_from_slice(b"int main(){return 0;}\n");

        let output = format_bytes(&input, &FormatOptions::default()).expect("format UTF-8 BOM");

        assert!(output.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert_eq!(
            std::str::from_utf8(&output[3..]).expect("UTF-8 output"),
            "int main() {\n    return 0;\n}\n"
        );
    }

    #[test]
    fn formats_eight_bit_input_and_preserves_high_bytes() {
        let input = b"int x=1;// caf\xe9\nvoid f(){int y=2;}\n";

        let output = format_bytes(input, &FormatOptions::default()).expect("format 8-bit input");

        assert_eq!(output, b"int x=1;// caf\xe9\nvoid f() {\n    int y=2;\n}\n");
    }

    #[test]
    fn preserves_high_bytes_inside_string_literal() {
        let input = b"const char* s=\"\xe9\xe8\";int a=3;\n";

        let output = format_bytes(input, &FormatOptions::default()).expect("format 8-bit string");

        assert_eq!(output, b"const char* s=\"\xe9\xe8\";\nint a=3;\n");
    }

    #[test]
    fn preserves_eight_bit_identifier_separator() {
        let input = b"int \xe9;\n";

        let output =
            format_bytes(input, &FormatOptions::default()).expect("format 8-bit identifier");

        assert_eq!(output, input);
    }

    #[test]
    fn preserves_utf16le_encoding_when_formatting_reader() {
        let input = utf16le_with_bom("int main(){return 0;}\n");

        let output = format_bytes(&input, &FormatOptions::default()).expect("format UTF-16LE");

        assert_eq!(
            decode_utf16le_with_bom(&output),
            "int main() {\n    return 0;\n}\n"
        );
    }

    #[test]
    fn rejects_malformed_bom_encoded_input() {
        for input in [
            &[0xEF, 0xBB, 0xBF, 0xFF][..],
            &[0xFF, 0xFE, 0x41][..],
            &[0xFE, 0xFF, 0xD8, 0x00][..],
        ] {
            let error = format_bytes(input, &FormatOptions::default())
                .expect_err("malformed encoded input must fail");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }

    #[test]
    fn rejects_utf32_input() {
        let error = format_bytes(&[0xFF, 0xFE, 0x00, 0x00], &FormatOptions::default())
            .expect_err("UTF-32 must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn default_reader_formatting_preserves_observed_line_endings() {
        let crlf = format_bytes(b"int main(){return 0;}\r\n", &FormatOptions::default())
            .expect("format CRLF input");
        assert_eq!(
            std::str::from_utf8(&crlf).expect("UTF-8 CRLF output"),
            "int main() {\r\n    return 0;\r\n}\r\n"
        );

        let cr = format_bytes(b"int main(){return 0;}\r", &FormatOptions::default())
            .expect("format CR input");
        assert_eq!(
            std::str::from_utf8(&cr).expect("UTF-8 CR output"),
            "int main() {\r    return 0;\r}\r"
        );
    }

    #[test]
    fn mixed_reader_line_endings_use_stable_dominant_ending() {
        let output = format_bytes(b"int a;\r\nint b;\n", &FormatOptions::default())
            .expect("format mixed input");

        assert_eq!(
            std::str::from_utf8(&output).expect("UTF-8 mixed output"),
            "int a;\r\nint b;\r\n"
        );
    }

    #[test]
    fn configured_lf_overrides_observed_crlf() {
        let mut options = FormatOptions::default();
        options.line_ending = LineEnding::Lf;

        let output = format_bytes(b"int main(){return 0;}\r\n", &options).expect("format LF");

        assert_eq!(
            std::str::from_utf8(&output).expect("UTF-8 output"),
            "int main() {\n    return 0;\n}\n"
        );
    }

    #[test]
    fn configured_line_endings_are_written_by_reader_formatting() {
        let mut options = FormatOptions::default();
        options.line_ending = LineEnding::Crlf;

        let output = format_bytes(b"int main(){return 0;}\n", &options).expect("format CRLF");

        assert_eq!(
            std::str::from_utf8(&output).expect("UTF-8 output"),
            "int main() {\r\n    return 0;\r\n}\r\n"
        );
    }

    #[test]
    fn reader_formatting_preserves_missing_final_line_break() {
        let output = format_bytes(b"int main(){return 0;}", &FormatOptions::default())
            .expect("format without final newline");

        assert_eq!(
            std::str::from_utf8(&output).expect("UTF-8 output"),
            "int main() {\n    return 0;\n}"
        );
    }

    #[test]
    fn utf8_bom_and_utf16_preserve_observed_line_endings() {
        let mut utf8_bom = vec![0xEF, 0xBB, 0xBF];
        utf8_bom.extend_from_slice(b"int main(){return 0;}\r\n");
        let utf8_output = format_bytes(&utf8_bom, &FormatOptions::default()).expect("UTF-8 BOM");
        assert_eq!(
            std::str::from_utf8(&utf8_output[3..]).expect("UTF-8 BOM output"),
            "int main() {\r\n    return 0;\r\n}\r\n"
        );

        let utf16_output = format_bytes(
            &utf16le_with_bom("int main(){return 0;}\r\n"),
            &FormatOptions::default(),
        )
        .expect("UTF-16LE");
        assert_eq!(
            decode_utf16le_with_bom(&utf16_output),
            "int main() {\r\n    return 0;\r\n}\r\n"
        );
    }
}
