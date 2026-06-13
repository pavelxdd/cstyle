#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LineEnding {
    Preserve,
    Lf,
    Crlf,
    Cr,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IndentStyle {
    Spaces,
    Tabs,
    ForceTabs,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MinConditionalIndent {
    Zero,
    One,
    Two,
    OneHalf,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BraceStyle {
    None,
    Allman,
    Attach,
    OneTrueBrace,
    WebKit,
    Whitesmith,
    Vtk,
    Ratliff,
    Gnu,
    Horstmann,
    Pico,
    Lisp,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PointerAlign {
    None,
    Type,
    Middle,
    Name,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReferenceAlign {
    None,
    Type,
    Middle,
    Name,
    SameAsPointer,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObjCColonPad {
    NoChange,
    None,
    All,
    After,
    Before,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LineBetweenMembers {
    None,
    Members,
    All,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Mode {
    C,
    ObjC,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StylePreset {
    None,
    Allman,
    Java,
    Kr,
    Stroustrup,
    Whitesmith,
    Vtk,
    Ratliff,
    Gnu,
    Linux,
    Horstmann,
    OneTrueBrace,
    Google,
    Mozilla,
    WebKit,
    Pico,
    Lisp,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct StyleFields {
    brace_style: BraceStyle,
    add_braces: bool,
    remove_braces: bool,
    break_one_line_blocks: bool,
    break_one_line_statements: bool,
    break_closing_braces: bool,
    attach_namespace: bool,
    attach_class: bool,
    attach_struct: bool,
    attach_enum: bool,
    min_conditional_indent: MinConditionalIndent,
    indent_braces: bool,
    indent_blocks: bool,
    indent_switches: bool,
    indent_classes: bool,
    indent_modifiers: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FormatOptions {
    pub indent_width: usize,
    pub tab_width: usize,
    pub indent_style: IndentStyle,
    pub brace_style: BraceStyle,
    pub add_braces: bool,
    pub add_one_line_braces: bool,
    pub remove_braces: bool,
    pub break_one_line_blocks: bool,
    pub break_one_line_headers: bool,
    pub break_one_line_statements: bool,
    pub pad_operators: bool,
    pub pad_commas: bool,
    pub pad_parens_outside: bool,
    pub pad_first_paren_outside: bool,
    pub pad_parens_inside: bool,
    pub pad_header: bool,
    pub unpad_parens: bool,
    pub pointer_align: PointerAlign,
    pub reference_align: ReferenceAlign,
    pub max_code_length: Option<usize>,
    pub break_after_logical: bool,
    pub break_blocks: bool,
    pub break_closing_header_blocks: bool,
    pub break_closing_braces: bool,
    pub attach_extern_c: bool,
    pub attach_namespace: bool,
    pub attach_class: bool,
    pub attach_struct: bool,
    pub attach_enum: bool,
    pub attach_inline: bool,
    pub attach_closing_while: bool,
    pub break_else_ifs: bool,
    pub no_indent_if_after_else: bool,
    pub break_return_type: bool,
    pub break_return_type_decl: bool,
    pub attach_return_type: bool,
    pub attach_return_type_decl: bool,
    pub continuation_indent: usize,
    pub max_continuation_indent: usize,
    pub min_conditional_indent: MinConditionalIndent,
    pub indent_after_parens: bool,
    pub indent_braces: bool,
    pub indent_blocks: bool,
    pub indent_switches: bool,
    pub indent_cases: bool,
    pub indent_labels: bool,
    pub indent_classes: bool,
    pub indent_modifiers: bool,
    pub indent_preproc_define: bool,
    pub indent_preproc_conditional: bool,
    pub indent_preproc_block: bool,
    pub indent_namespaces: bool,
    pub indent_col1_comments: bool,
    pub delete_empty_lines: bool,
    pub empty_line_fill: bool,
    pub strip_comment_prefix: bool,
    pub convert_tabs: bool,
    pub close_templates: bool,
    pub pad_method_prefix: bool,
    pub unpad_method_prefix: bool,
    pub pad_return_type: bool,
    pub unpad_return_type: bool,
    pub pad_param_type: bool,
    pub unpad_param_type: bool,
    pub align_method_colon: bool,
    pub pad_method_colon: ObjCColonPad,
    pub line_between_members: LineBetweenMembers,
    pub mode: Mode,
    pub line_ending: LineEnding,
    pub access_labels: Vec<String>,
    pub macro_blocks: Vec<(String, String)>,
    pub control_headers: Vec<String>,
    pub non_paren_headers: Vec<String>,
    tab_width_explicit: bool,
    keep_one_line_blocks_explicit: bool,
    active_style: Option<StylePreset>,
    style_base: Option<StyleFields>,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_width: 4,
            tab_width: 4,
            indent_style: IndentStyle::Spaces,
            brace_style: BraceStyle::None,
            add_braces: false,
            add_one_line_braces: false,
            remove_braces: false,
            break_one_line_blocks: true,
            break_one_line_headers: false,
            break_one_line_statements: true,
            pad_operators: false,
            pad_commas: false,
            pad_parens_outside: false,
            pad_first_paren_outside: false,
            pad_parens_inside: false,
            pad_header: false,
            unpad_parens: false,
            pointer_align: PointerAlign::None,
            reference_align: ReferenceAlign::SameAsPointer,
            max_code_length: None,
            break_after_logical: false,
            break_blocks: false,
            break_closing_header_blocks: false,
            break_closing_braces: false,
            attach_extern_c: false,
            attach_namespace: false,
            attach_class: false,
            attach_struct: true,
            attach_enum: true,
            attach_inline: false,
            attach_closing_while: false,
            break_else_ifs: false,
            no_indent_if_after_else: false,
            break_return_type: false,
            break_return_type_decl: false,
            attach_return_type: false,
            attach_return_type_decl: false,
            continuation_indent: 1,
            max_continuation_indent: 40,
            min_conditional_indent: MinConditionalIndent::Two,
            indent_after_parens: false,
            indent_braces: false,
            indent_blocks: false,
            indent_switches: false,
            indent_cases: false,
            indent_labels: false,
            indent_classes: false,
            indent_modifiers: false,
            indent_preproc_define: false,
            indent_preproc_conditional: false,
            indent_preproc_block: false,
            indent_namespaces: false,
            indent_col1_comments: false,
            delete_empty_lines: false,
            empty_line_fill: false,
            strip_comment_prefix: false,
            convert_tabs: false,
            close_templates: false,
            pad_method_prefix: false,
            unpad_method_prefix: false,
            pad_return_type: false,
            unpad_return_type: false,
            pad_param_type: false,
            unpad_param_type: false,
            align_method_colon: false,
            pad_method_colon: ObjCColonPad::NoChange,
            line_between_members: LineBetweenMembers::None,
            mode: Mode::C,
            line_ending: LineEnding::Preserve,
            access_labels: Vec::new(),
            macro_blocks: vec![
                (
                    "wxBEGIN_EVENT_TABLE".to_string(),
                    "wxEND_EVENT_TABLE".to_string(),
                ),
                (
                    "BEGIN_MESSAGE_MAP".to_string(),
                    "END_MESSAGE_MAP".to_string(),
                ),
            ],
            control_headers: Vec::new(),
            non_paren_headers: Vec::new(),
            tab_width_explicit: false,
            keep_one_line_blocks_explicit: false,
            active_style: None,
            style_base: None,
        }
    }
}

impl StyleFields {
    fn capture(options: &FormatOptions) -> Self {
        Self {
            brace_style: options.brace_style,
            add_braces: options.add_braces,
            remove_braces: options.remove_braces,
            break_one_line_blocks: options.break_one_line_blocks,
            break_one_line_statements: options.break_one_line_statements,
            break_closing_braces: options.break_closing_braces,
            attach_namespace: options.attach_namespace,
            attach_class: options.attach_class,
            attach_struct: options.attach_struct,
            attach_enum: options.attach_enum,
            min_conditional_indent: options.min_conditional_indent,
            indent_braces: options.indent_braces,
            indent_blocks: options.indent_blocks,
            indent_switches: options.indent_switches,
            indent_classes: options.indent_classes,
            indent_modifiers: options.indent_modifiers,
        }
    }

    fn restore(self, options: &mut FormatOptions) {
        options.brace_style = self.brace_style;
        options.add_braces = self.add_braces;
        options.remove_braces = self.remove_braces;
        options.break_one_line_blocks = self.break_one_line_blocks;
        options.break_one_line_statements = self.break_one_line_statements;
        options.break_closing_braces = self.break_closing_braces;
        options.attach_namespace = self.attach_namespace;
        options.attach_class = self.attach_class;
        options.attach_struct = self.attach_struct;
        options.attach_enum = self.attach_enum;
        options.min_conditional_indent = self.min_conditional_indent;
        options.indent_braces = self.indent_braces;
        options.indent_blocks = self.indent_blocks;
        options.indent_switches = self.indent_switches;
        options.indent_classes = self.indent_classes;
        options.indent_modifiers = self.indent_modifiers;
    }
}

impl FormatOptions {
    pub fn set_style(&mut self, style: StylePreset) {
        self.remove_active_style();
        self.apply_style_preset(style);
    }

    pub fn set_min_conditional_indent(&mut self, value: MinConditionalIndent) {
        let style = self.remove_active_style();
        self.min_conditional_indent = value;
        if let Some(style) = style {
            self.apply_style_preset(style);
        }
    }

    pub(super) fn remove_active_style(&mut self) -> Option<StylePreset> {
        let style = self.active_style.take()?;
        self.style_base
            .take()
            .expect("active style must have a baseline")
            .restore(self);
        Some(style)
    }

    pub(super) fn has_active_style(&self) -> bool {
        self.active_style.is_some()
    }

    pub(super) fn apply_style_preset(&mut self, style: StylePreset) {
        let base = StyleFields::capture(self);
        match style {
            StylePreset::None => self.brace_style = BraceStyle::None,
            StylePreset::Allman => self.brace_style = BraceStyle::Allman,
            StylePreset::Java => self.brace_style = BraceStyle::Attach,
            StylePreset::Kr => {
                self.brace_style = BraceStyle::OneTrueBrace;
                self.attach_struct = true;
                self.attach_enum = true;
            }
            StylePreset::Linux => {
                self.brace_style = BraceStyle::OneTrueBrace;
                self.attach_struct = true;
                self.attach_enum = true;
                self.min_conditional_indent = MinConditionalIndent::OneHalf;
            }
            StylePreset::Mozilla => {
                self.brace_style = BraceStyle::OneTrueBrace;
                self.attach_namespace = true;
                self.attach_struct = false;
                self.attach_enum = false;
            }
            StylePreset::WebKit => self.brace_style = BraceStyle::WebKit,
            StylePreset::Stroustrup => {
                self.brace_style = BraceStyle::OneTrueBrace;
                self.break_closing_braces = true;
                self.attach_namespace = true;
                self.attach_class = true;
                self.attach_struct = true;
                self.attach_enum = true;
            }
            StylePreset::Whitesmith => {
                self.brace_style = BraceStyle::Whitesmith;
                self.indent_braces = true;
                self.indent_classes = true;
                self.indent_switches = true;
            }
            StylePreset::Vtk => self.brace_style = BraceStyle::Vtk,
            StylePreset::Ratliff => {
                self.brace_style = BraceStyle::Ratliff;
                self.indent_braces = true;
                self.indent_classes = true;
            }
            StylePreset::Gnu => {
                self.brace_style = BraceStyle::Gnu;
                self.indent_blocks = true;
            }
            StylePreset::Horstmann => {
                self.brace_style = BraceStyle::Horstmann;
                self.indent_switches = true;
            }
            StylePreset::OneTrueBrace => {
                self.brace_style = BraceStyle::OneTrueBrace;
                self.add_braces = true;
                self.remove_braces = false;
                self.attach_struct = true;
                self.attach_enum = true;
            }
            StylePreset::Google => {
                self.brace_style = BraceStyle::Attach;
                self.indent_modifiers = true;
            }
            StylePreset::Pico => {
                self.brace_style = BraceStyle::Pico;
                self.break_one_line_blocks = false;
                self.break_one_line_statements = false;
                self.indent_switches = true;
            }
            StylePreset::Lisp => {
                self.brace_style = BraceStyle::Lisp;
                self.break_one_line_statements = false;
            }
        }
        if self.add_braces || self.add_one_line_braces {
            self.remove_braces = false;
        }
        self.active_style = Some(style);
        self.style_base = Some(base);
    }

    pub(super) fn keep_one_line_blocks(&mut self) {
        self.break_one_line_blocks = false;
        self.keep_one_line_blocks_explicit = true;
    }

    pub fn set_space_indentation(&mut self, width: usize) {
        self.set_indent_style_width(IndentStyle::Spaces, width);
    }

    pub fn set_tab_indentation(&mut self, width: usize) {
        self.set_indent_style_width(IndentStyle::Tabs, width);
    }

    pub fn set_force_tab_indentation(&mut self, width: usize) {
        self.set_indent_style_width(IndentStyle::ForceTabs, width);
    }

    pub(super) fn set_indent_style_width(&mut self, style: IndentStyle, width: usize) {
        self.indent_style = style;
        if style == IndentStyle::ForceTabs {
            self.set_indent_width(width);
        } else {
            self.indent_width = width;
            self.tab_width = width;
            self.tab_width_explicit = false;
        }
    }

    pub(super) fn set_indent_width(&mut self, width: usize) {
        self.indent_width = width;
        if !self.tab_width_explicit {
            self.tab_width = width;
        }
    }

    pub fn set_force_tab_width(&mut self, width: usize) {
        self.indent_style = IndentStyle::ForceTabs;
        self.tab_width = width;
        self.tab_width_explicit = true;
    }

    pub(crate) fn lisp_add_one_line_braces_breaks_blocks(&self) -> bool {
        self.brace_style == BraceStyle::Lisp
            && self.add_one_line_braces
            && !self.keep_one_line_blocks_explicit
    }

    pub(crate) fn keeps_multi_statement_line(&self) -> bool {
        !self.break_one_line_statements
    }

    pub fn line_break(&self) -> &'static str {
        match self.line_ending {
            LineEnding::Preserve | LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
            LineEnding::Cr => "\r",
        }
    }

    pub fn indent_prefix(&self, level: usize) -> String {
        let columns = level * self.indent_width;
        match self.indent_style {
            IndentStyle::Spaces => " ".repeat(columns),
            IndentStyle::Tabs => "\t".repeat(level),
            IndentStyle::ForceTabs => {
                let tab_width = self.tab_width.max(1);
                format!(
                    "{}{}",
                    "\t".repeat(columns / tab_width),
                    " ".repeat(columns % tab_width)
                )
            }
        }
    }

    pub fn continuation_indent_prefix(
        &self,
        structural_level: usize,
        total_columns: usize,
    ) -> String {
        match self.indent_style {
            IndentStyle::Spaces => " ".repeat(total_columns),
            IndentStyle::ForceTabs => {
                let tab_width = self.tab_width.max(1);
                format!(
                    "{}{}",
                    "\t".repeat(total_columns / tab_width),
                    " ".repeat(total_columns % tab_width)
                )
            }
            IndentStyle::Tabs => {
                let width = self.indent_width.max(1);
                let tabs = structural_level.min(total_columns / width);
                format!(
                    "{}{}",
                    "\t".repeat(tabs),
                    " ".repeat(total_columns - tabs * width)
                )
            }
        }
    }
}
