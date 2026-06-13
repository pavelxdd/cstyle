use self::stack::PersistentStack;
use crate::config::FormatOptions;

mod stack;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PreprocessorIndent {
    pub level: usize,
    pub spaces: Option<usize>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct IndentationState {
    indent: usize,
    paren_depth: usize,
    bracket_depth: usize,
    block_indent_extra_stack: Vec<usize>,
    block_indent_increment_stack: Vec<usize>,
    paren_depth_stack: Vec<usize>,
    bracket_depth_stack: Vec<usize>,
    block_statement_stack: Vec<bool>,
    paren_statement_stack: Vec<bool>,
    continuation_indent_stack: Vec<usize>,
    continuation_indent_stack_size_stack: Vec<usize>,
    paren_indent_stack: Vec<usize>,
    preproc_indent_stack: PersistentStack<PreprocessorIndent>,
    braceless_block_stack: Vec<(usize, usize)>,
}

impl IndentationState {
    pub fn indent(&self) -> usize {
        self.indent
    }

    pub fn statement_depth(&self) -> usize {
        self.paren_depth + self.bracket_depth
    }

    pub fn bracket_depth(&self) -> usize {
        self.bracket_depth
    }

    pub fn enter_braceless_block(&mut self, delta: usize) {
        self.braceless_block_stack.push((self.indent, delta));
        self.indent += delta;
    }

    pub fn last_braceless_block(&self) -> Option<(usize, usize)> {
        self.braceless_block_stack.last().copied()
    }

    pub fn exit_braceless_block(&mut self) {
        if let Some((_, delta)) = self.braceless_block_stack.pop() {
            self.indent = self.indent.saturating_sub(delta);
        }
    }

    pub fn enter_block(&mut self) {
        self.enter_block_with_statement(false);
    }

    pub fn enter_block_with_statement(&mut self, is_statement: bool) {
        self.enter_block_with_extra(is_statement, 0);
    }

    pub fn enter_block_with_extra(&mut self, is_statement: bool, extra_indent: usize) {
        self.enter_block_with_indent_increment(is_statement, 1 + extra_indent, extra_indent);
    }

    pub fn enter_block_without_indent(&mut self, is_statement: bool) {
        self.enter_block_with_indent_increment(is_statement, 0, 0);
    }

    fn enter_block_with_indent_increment(
        &mut self,
        is_statement: bool,
        indent_increment: usize,
        extra_indent: usize,
    ) {
        self.indent += indent_increment;
        self.block_indent_extra_stack.push(extra_indent);
        self.block_indent_increment_stack.push(indent_increment);
        self.block_statement_stack.push(is_statement);
        self.paren_depth_stack.push(self.paren_depth);
        self.bracket_depth_stack.push(self.bracket_depth);
        self.paren_depth = 0;
        self.bracket_depth = 0;
        self.push_continuation_checkpoint();
    }

    pub fn exit_block(&mut self) {
        let indent_increment = self.block_indent_increment_stack.pop().unwrap_or(1);
        self.block_indent_extra_stack.pop();
        self.indent = self.indent.saturating_sub(indent_increment);
        self.block_statement_stack.pop();
        self.paren_depth = self.paren_depth_stack.pop().unwrap_or(0);
        self.bracket_depth = self.bracket_depth_stack.pop().unwrap_or(0);
        self.restore_continuation_checkpoint();
    }

    pub fn enter_paren(&mut self) {
        if self.paren_depth == 0 {
            self.paren_indent_stack.push(self.indent);
            self.paren_statement_stack.push(true);
        }
        self.paren_depth += 1;
        self.push_continuation_checkpoint();
    }

    pub fn exit_paren(&mut self) {
        self.paren_depth = self.paren_depth.saturating_sub(1);
        if self.paren_depth == 0 {
            self.paren_indent_stack.pop();
            self.paren_statement_stack.pop();
        }
        self.restore_continuation_checkpoint();
    }

    pub fn enter_bracket(&mut self) {
        self.bracket_depth += 1;
        self.push_continuation_checkpoint();
    }

    pub fn exit_bracket(&mut self) {
        self.bracket_depth = self.bracket_depth.saturating_sub(1);
        self.restore_continuation_checkpoint();
    }

    pub fn current_block_indent_increment(&self) -> Option<usize> {
        self.block_indent_increment_stack.last().copied()
    }

    pub fn line_indent(&self, line_kind: LineKind, options: &FormatOptions) -> usize {
        match line_kind {
            LineKind::Normal => self.indent,
            LineKind::Label if options.indent_labels => self.indent.saturating_sub(1),
            LineKind::Label => 0,
            LineKind::SwitchLabel if options.indent_switches => self.indent,
            LineKind::SwitchLabel => self.indent.saturating_sub(1),
        }
    }

    pub fn brace_block_depth(&self) -> usize {
        self.block_indent_increment_stack.len()
    }

    pub fn push_continuation_checkpoint(&mut self) {
        self.continuation_indent_stack_size_stack
            .push(self.continuation_indent_stack.len());
    }

    pub fn register_continuation_indent(&mut self, indent: usize) {
        self.continuation_indent_stack.push(indent);
    }

    pub fn restore_continuation_checkpoint(&mut self) -> Option<usize> {
        let target_size = self.continuation_indent_stack_size_stack.pop().unwrap_or(0);
        self.continuation_indent_stack.truncate(target_size);
        self.current_continuation_indent()
    }

    pub fn clear_continuation_indents(&mut self) {
        let target_size = self
            .continuation_indent_stack_size_stack
            .last()
            .copied()
            .unwrap_or(0);
        self.continuation_indent_stack.truncate(target_size);
    }

    pub fn current_continuation_indent(&self) -> Option<usize> {
        self.continuation_indent_stack.last().copied()
    }

    #[cfg(test)]
    pub fn continuation_stack_depth(&self) -> usize {
        self.continuation_indent_stack.len()
    }

    pub fn push_preproc_indent(&mut self, level: usize, spaces: Option<usize>) {
        self.preproc_indent_stack
            .push(PreprocessorIndent { level, spaces });
    }

    pub fn pop_preproc_indent(&mut self) -> Option<PreprocessorIndent> {
        self.preproc_indent_stack.pop()
    }

    pub fn current_preproc_indent(&self) -> Option<PreprocessorIndent> {
        self.preproc_indent_stack.last().copied()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LineKind {
    Normal,
    Label,
    SwitchLabel,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]

    use super::*;

    #[test]
    fn tracks_blocks_without_underflow() {
        let mut state = IndentationState::default();

        state.exit_block();
        assert_eq!(state.indent(), 0);
        state.enter_block();
        state.enter_block_with_statement(true);
        assert_eq!(state.indent(), 2);
        assert_eq!(state.brace_block_depth(), 2);
        assert_eq!(
            state
                .block_statement_stack
                .iter()
                .filter(|is_statement| **is_statement)
                .count(),
            1
        );
        state.exit_block();
        assert_eq!(state.indent(), 1);
        assert_eq!(state.brace_block_depth(), 1);
    }

    #[test]
    fn unmatched_scope_exits_restore_continuation_checkpoints() {
        let exits: [fn(&mut IndentationState); 3] = [
            IndentationState::exit_block,
            IndentationState::exit_paren,
            IndentationState::exit_bracket,
        ];
        for exit in exits {
            let mut state = IndentationState::default();
            state.register_continuation_indent(4);
            state.push_continuation_checkpoint();
            state.register_continuation_indent(8);

            exit(&mut state);

            assert_eq!(state.current_continuation_indent(), Some(4));
        }
    }

    #[test]
    fn restoring_without_a_checkpoint_clears_continuation_state() {
        let mut state = IndentationState::default();
        state.register_continuation_indent(4);

        assert_eq!(state.restore_continuation_checkpoint(), None);
        assert_eq!(state.current_continuation_indent(), None);
    }

    #[test]
    fn tracks_extra_block_indent() {
        let mut state = IndentationState::default();

        state.enter_block_with_extra(false, 1);
        assert_eq!(state.indent(), 2);
        state.enter_block();
        assert_eq!(state.indent(), 3);
        state.exit_block();
        assert_eq!(state.indent(), 2);
        state.exit_block();
        assert_eq!(state.indent(), 0);
    }

    #[test]
    fn tracks_statement_depth() {
        let mut state = IndentationState::default();

        state.enter_paren();
        state.enter_bracket();
        assert_eq!(state.statement_depth(), 2);
        assert_eq!(state.paren_statement_stack.len(), 1);
        assert_eq!(state.paren_indent_stack.last().copied(), Some(0));
        state.exit_paren();
        state.exit_bracket();
        assert_eq!(state.statement_depth(), 0);
        assert_eq!(state.paren_statement_stack.len(), 0);
    }

    #[test]
    fn tracks_continuation_indent_stack() {
        let mut state = IndentationState::default();

        state.register_continuation_indent(4);
        state.register_continuation_indent(8);
        assert_eq!(state.current_continuation_indent(), Some(8));
        assert_eq!(state.continuation_stack_depth(), 2);
        assert_eq!(state.continuation_indent_stack.pop(), Some(8));
        assert_eq!(state.current_continuation_indent(), Some(4));
    }

    #[test]
    fn restores_continuation_indent_checkpoints() {
        let mut state = IndentationState::default();

        state.register_continuation_indent(4);
        state.push_continuation_checkpoint();
        state.register_continuation_indent(8);
        state.push_continuation_checkpoint();
        state.register_continuation_indent(12);
        state.register_continuation_indent(16);
        assert_eq!(state.continuation_stack_depth(), 4);

        assert_eq!(state.restore_continuation_checkpoint(), Some(8));
        assert_eq!(state.continuation_stack_depth(), 2);
        assert_eq!(state.restore_continuation_checkpoint(), Some(4));
        assert_eq!(state.continuation_stack_depth(), 1);

        state.clear_continuation_indents();
        assert_eq!(state.continuation_stack_depth(), 0);
    }

    #[test]
    fn closing_scopes_restore_continuation_checkpoints() {
        let mut state = IndentationState::default();

        state.enter_paren();
        state.register_continuation_indent(4);
        state.enter_bracket();
        state.register_continuation_indent(8);
        state.exit_bracket();
        assert_eq!(state.current_continuation_indent(), Some(4));
        state.exit_paren();
        assert_eq!(state.current_continuation_indent(), None);

        state.enter_block();
        state.register_continuation_indent(4);
        state.exit_block();
        assert_eq!(state.current_continuation_indent(), None);
    }

    #[test]
    fn tracks_preprocessor_indent_stack() {
        let mut state = IndentationState::default();

        state.push_preproc_indent(1, None);
        state.push_preproc_indent(2, Some(9));
        assert_eq!(
            state.current_preproc_indent(),
            Some(PreprocessorIndent {
                level: 2,
                spaces: Some(9)
            })
        );
        assert_eq!(
            state.pop_preproc_indent(),
            Some(PreprocessorIndent {
                level: 2,
                spaces: Some(9)
            })
        );
        assert_eq!(
            state.current_preproc_indent(),
            Some(PreprocessorIndent {
                level: 1,
                spaces: None
            })
        );
    }

    #[test]
    fn labels_are_one_level_out_when_enabled() {
        let mut state = IndentationState::default();
        let mut options = FormatOptions::default();
        options.indent_labels = true;
        state.enter_block();
        state.enter_block();

        assert_eq!(state.line_indent(LineKind::Normal, &options), 2);
        assert_eq!(state.line_indent(LineKind::Label, &options), 1);
    }

    #[test]
    fn labels_flush_left_by_default() {
        let mut state = IndentationState::default();
        let options = FormatOptions::default();
        state.enter_block();
        state.enter_block();

        assert_eq!(state.line_indent(LineKind::Label, &options), 0);
    }

    #[test]
    fn switch_labels_follow_switch_indent_option() {
        let mut state = IndentationState::default();
        let mut options = FormatOptions::default();
        state.enter_block();
        state.enter_block();

        assert_eq!(state.line_indent(LineKind::SwitchLabel, &options), 1);
        options.indent_switches = true;
        assert_eq!(state.line_indent(LineKind::SwitchLabel, &options), 2);
    }
}
