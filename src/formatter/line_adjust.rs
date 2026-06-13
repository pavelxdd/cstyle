use super::indentation::LineKind;
use super::labels;
use super::switch_cases::{SwitchCaseLineTransformer, SwitchCaseObserver};
use super::tabs;
use crate::config::{FormatOptions, IndentStyle};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LineAdjuster {
    switch_observer: SwitchCaseObserver,
    switch_case_transformer: SwitchCaseLineTransformer,
    macro_block_depth: usize,
    access_labels: Vec<String>,
    macro_blocks: Vec<(String, String)>,
    tab_converter: tabs::Converter,
    tab_width: usize,
    indent_width: usize,
    indent_style: IndentStyle,
    empty_line_fill: bool,
    case_processing_enabled: bool,
    line_observe_enabled: bool,
}

impl LineAdjuster {
    pub fn new(options: &FormatOptions) -> Self {
        Self {
            switch_observer: SwitchCaseObserver::default(),
            switch_case_transformer: SwitchCaseLineTransformer::new(options),
            macro_block_depth: 0,
            access_labels: options.access_labels.clone(),
            macro_blocks: options.macro_blocks.clone(),
            tab_converter: tabs::Converter::new(options.convert_tabs),
            tab_width: options.tab_width,
            indent_width: options.indent_width,
            indent_style: options.indent_style,
            empty_line_fill: options.empty_line_fill,
            case_processing_enabled: true,
            line_observe_enabled: true,
        }
    }

    pub fn set_case_processing_enabled(&mut self, enabled: bool) {
        self.case_processing_enabled = enabled;
    }

    pub fn set_line_observe_enabled(&mut self, enabled: bool) {
        self.line_observe_enabled = enabled;
    }

    pub fn set_tab_conversion_enabled(&mut self, enabled: bool) {
        self.tab_converter.set_enabled(enabled);
    }

    pub fn observe_raw_comment_line(&mut self, line: &str) {
        if self.line_observe_enabled {
            let kind = labels::line_kind(line.trim_start(), &self.access_labels);
            self.switch_observer.observe_line(line, kind);
        }
    }

    pub fn mark_case_label_colon(&mut self, byte_index: usize) {
        self.switch_case_transformer.mark_label_colon(byte_index);
    }

    pub fn observe_line(&mut self, line: &str) -> LineKind {
        if !self.line_observe_enabled {
            return LineKind::Normal;
        }
        let kind = labels::line_kind(line.trim_start(), &self.access_labels);
        self.switch_observer.observe_line(line, kind)
    }

    pub fn adjust_line(&mut self, line: String) -> String {
        self.switch_case_transformer.begin_line();
        let line = self.adjust_macro_block_line(line);
        let line = if self.case_processing_enabled {
            self.switch_case_transformer.transform_line(line)
        } else {
            line
        };
        self.convert_line_tabs(line)
    }

    pub fn adjust_raw_literal_line(&mut self, line: String) -> String {
        self.switch_case_transformer.begin_line();
        let observed_suffix = self
            .switch_case_transformer
            .raw_literal_suffix_start(&line)
            .map(|end| &line[end..]);
        if self.case_processing_enabled {
            self.switch_case_transformer.scan_raw_literal_line(&line);
        }
        if self.line_observe_enabled
            && let Some(suffix) = observed_suffix
        {
            let kind = labels::line_kind(suffix.trim_start(), &self.access_labels);
            self.switch_observer.observe_line(suffix, kind);
        }
        self.convert_line_tabs(line)
    }

    fn convert_line_tabs(&mut self, line: String) -> String {
        let keep_indent_tabs = matches!(
            self.indent_style,
            IndentStyle::Tabs | IndentStyle::ForceTabs
        );
        self.tab_converter
            .convert(line, self.tab_width, keep_indent_tabs)
    }

    fn adjust_macro_block_line(&mut self, line: String) -> String {
        if self.macro_blocks.is_empty() {
            return line;
        }
        let trimmed = line.trim_start();
        let is_begin = macro_block_end_for(trimmed, &self.macro_blocks).is_some();
        let is_end = macro_block_end_macro(trimmed, &self.macro_blocks);
        let is_preprocessor = trimmed.starts_with('#');
        let extra_levels = if is_preprocessor {
            0
        } else if is_end {
            self.macro_block_depth.saturating_sub(1)
        } else {
            self.macro_block_depth
        };
        let mut line = line;
        if extra_levels > 0 {
            self.indent_line(&mut line, extra_levels);
        }
        if is_begin {
            self.macro_block_depth += 1;
        }
        if is_end {
            self.macro_block_depth = self.macro_block_depth.saturating_sub(1);
        }
        line
    }

    pub fn total_case_unindent_depth(&self) -> usize {
        self.switch_case_transformer.total_unindent_depth()
    }

    pub fn next_line_case_unindent_depth(&self) -> usize {
        self.switch_case_transformer.next_line_unindent_depth()
    }

    pub fn case_unindent_depth_for_line(&self, line: &str) -> usize {
        self.switch_case_transformer.unindent_depth_for_line(line)
    }

    pub fn pending_case_unindent(&self) -> usize {
        self.switch_case_transformer.pending_unindent_depth()
    }

    fn indent_line(&self, line: &mut String, levels: usize) -> usize {
        if line.is_empty() && !self.empty_line_fill {
            return 0;
        }

        match self.indent_style {
            IndentStyle::ForceTabs if self.indent_width != self.tab_width => {
                let expanded = tabs::force_tab_indent_to_spaces(line, self.tab_width);
                *line = format!("{}{}", " ".repeat(levels * self.indent_width), expanded);
                *line = tabs::space_indent_to_force_tabs(line, self.tab_width);
                levels * self.indent_width
            }
            IndentStyle::Tabs | IndentStyle::ForceTabs => {
                line.insert_str(0, &"\t".repeat(levels));
                levels
            }
            IndentStyle::Spaces => {
                let spaces = levels * self.indent_width;
                line.insert_str(0, &" ".repeat(spaces));
                spaces
            }
        }
    }

    pub fn switch_depth(&self) -> usize {
        self.switch_observer.switch_depth()
    }

    pub fn is_in_macro_block(&self) -> bool {
        self.macro_block_depth > 0
    }
}

fn macro_block_end_for<'a>(line: &str, macro_blocks: &'a [(String, String)]) -> Option<&'a str> {
    macro_blocks
        .iter()
        .find_map(|(begin, end)| macro_block_line_starts_with(line, begin).then_some(end.as_str()))
}

fn macro_block_end_macro(line: &str, macro_blocks: &[(String, String)]) -> bool {
    macro_blocks
        .iter()
        .any(|(_, end)| macro_block_line_starts_with(line, end))
}

fn macro_block_line_starts_with(line: &str, name: &str) -> bool {
    macro_call_starts_with(line, name)
        || line
            .strip_prefix("#define")
            .is_some_and(|rest| macro_call_starts_with(rest.trim_start(), name))
}

pub(super) fn macro_call_starts_with(line: &str, name: &str) -> bool {
    let rest = match line.strip_prefix(name) {
        Some(rest) => rest,
        None => return false,
    };
    rest.chars()
        .next()
        .is_none_or(|ch| ch == '(' || ch.is_whitespace())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]

    use super::*;

    #[test]
    fn unindents_case_brace_blocks_after_formatting() {
        let options = FormatOptions::default();
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(
            line_adjuster.adjust_line("    switch (x)".to_string()),
            "    switch (x)"
        );
        assert_eq!(line_adjuster.adjust_line("    {".to_string()), "    {");
        assert_eq!(
            line_adjuster.adjust_line("    case 1:".to_string()),
            "    case 1:"
        );
        assert_eq!(line_adjuster.adjust_line("        {".to_string()), "    {");
        assert_eq!(
            line_adjuster.adjust_line("            return 1;".to_string()),
            "        return 1;"
        );
        assert_eq!(line_adjuster.adjust_line("        }".to_string()), "    }");
        assert_eq!(line_adjuster.adjust_line("    }".to_string()), "    }");
    }

    #[test]
    fn case_unindent_force_tabs_removes_tabs_when_tab_width_matches_indent_width() {
        let mut options = FormatOptions::default();
        options.indent_style = IndentStyle::ForceTabs;
        options.indent_width = 4;
        options.tab_width = 4;
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(
            line_adjuster.adjust_line("\tswitch (x)".to_string()),
            "\tswitch (x)"
        );
        assert_eq!(line_adjuster.adjust_line("\t{".to_string()), "\t{");
        assert_eq!(
            line_adjuster.adjust_line("\tcase 1:".to_string()),
            "\tcase 1:"
        );
        assert_eq!(line_adjuster.adjust_line("\t\t{".to_string()), "\t{");
        assert_eq!(
            line_adjuster.adjust_line("\t\t\treturn 1;".to_string()),
            "\t\treturn 1;"
        );
        assert_eq!(line_adjuster.adjust_line("\t\t}".to_string()), "\t}");
    }

    #[test]
    fn case_unindent_preserves_pending_brace_across_preprocessor_lines() {
        let options = FormatOptions::default();
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(
            line_adjuster.adjust_line("    switch (x)".to_string()),
            "    switch (x)"
        );
        assert_eq!(line_adjuster.adjust_line("    {".to_string()), "    {");
        assert_eq!(
            line_adjuster.adjust_line("    case 1:".to_string()),
            "    case 1:"
        );
        assert_eq!(line_adjuster.adjust_line("#if A".to_string()), "#if A");
        assert_eq!(line_adjuster.adjust_line("        {".to_string()), "    {");
    }

    #[test]
    fn case_comment_parser_carries_block_comment_state() {
        let options = FormatOptions::default();
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(
            line_adjuster.adjust_line("    switch (x)".to_string()),
            "    switch (x)"
        );
        assert_eq!(line_adjuster.adjust_line("    {".to_string()), "    {");
        assert_eq!(
            line_adjuster.adjust_line("    case 1:".to_string()),
            "    case 1:"
        );
        assert_eq!(line_adjuster.adjust_line("        {".to_string()), "    {");
        assert_eq!(
            line_adjuster.adjust_line("            /* multi".to_string()),
            "        /* multi"
        );
        assert_eq!(
            line_adjuster.adjust_line("             * body".to_string()),
            "         * body"
        );
        assert_eq!(
            line_adjuster.adjust_line("             */".to_string()),
            "         */"
        );
    }

    #[test]
    fn indents_configured_macro_block_bodies() {
        let mut options = FormatOptions::default();
        options.macro_blocks = vec![("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())];
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(
            line_adjuster.adjust_line("BEGIN_BLOCK(Frame, Base)".to_string()),
            "BEGIN_BLOCK(Frame, Base)"
        );
        assert_eq!(
            line_adjuster.adjust_line("BLOCK_ITEM(ID_MENU, Frame::HandleMenu)".to_string()),
            "    BLOCK_ITEM(ID_MENU, Frame::HandleMenu)"
        );
        assert_eq!(
            line_adjuster.adjust_line("END_BLOCK()".to_string()),
            "END_BLOCK()"
        );
    }

    #[test]
    fn macro_block_blank_lines_follow_empty_line_fill() {
        let mut options = FormatOptions::default();
        options.macro_blocks = vec![("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())];
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(
            line_adjuster.adjust_line("BEGIN_BLOCK(Frame, Base)".to_string()),
            "BEGIN_BLOCK(Frame, Base)"
        );
        assert_eq!(line_adjuster.adjust_line(String::new()), "");

        options.empty_line_fill = true;
        let mut line_adjuster = LineAdjuster::new(&options);
        assert_eq!(
            line_adjuster.adjust_line("BEGIN_BLOCK(Frame, Base)".to_string()),
            "BEGIN_BLOCK(Frame, Base)"
        );
        assert_eq!(line_adjuster.adjust_line(String::new()), "    ");
    }

    #[test]
    fn macro_block_indent_uses_tab_style() {
        let mut options = FormatOptions::default();
        options.indent_style = IndentStyle::ForceTabs;
        options.empty_line_fill = true;
        options.macro_blocks = vec![("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())];
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(
            line_adjuster.adjust_line("BEGIN_BLOCK(Frame, Base)".to_string()),
            "BEGIN_BLOCK(Frame, Base)"
        );
        assert_eq!(
            line_adjuster.adjust_line("BLOCK_ITEM(ID_MENU, Frame::HandleMenu)".to_string()),
            "\tBLOCK_ITEM(ID_MENU, Frame::HandleMenu)"
        );
        assert_eq!(line_adjuster.adjust_line(String::new()), "\t");
    }

    #[test]
    fn macro_block_does_not_indent_preprocessor_lines() {
        let mut options = FormatOptions::default();
        options.macro_blocks = vec![("BEGIN_BLOCK".to_string(), "END_BLOCK".to_string())];
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(
            line_adjuster.adjust_line("BEGIN_BLOCK(Frame, Base)".to_string()),
            "BEGIN_BLOCK(Frame, Base)"
        );
        assert_eq!(line_adjuster.adjust_line("#if A".to_string()), "#if A");
        assert_eq!(
            line_adjuster.adjust_line("BLOCK_ITEM(ID_MENU, Frame::HandleMenu)".to_string()),
            "    BLOCK_ITEM(ID_MENU, Frame::HandleMenu)"
        );
        assert_eq!(line_adjuster.adjust_line("#endif".to_string()), "#endif");
    }

    #[test]
    fn stateful_line_adjuster_observes_lines_and_converts_tabs() {
        let mut options = FormatOptions::default();
        options.convert_tabs = true;
        options.tab_width = 2;
        let mut line_adjuster = LineAdjuster::new(&options);

        assert_eq!(line_adjuster.observe_line("switch (x)"), LineKind::Normal);
        assert_eq!(line_adjuster.observe_line("{"), LineKind::Normal);
        assert_eq!(line_adjuster.switch_depth(), 1);
        assert_eq!(line_adjuster.observe_line("case 1:"), LineKind::SwitchLabel);
        assert_eq!(
            line_adjuster.adjust_line("\treturn;".to_string()),
            "  return;"
        );
    }
}
