//! `/jump` picker: an overlay listing every turn in the conversation.
//!
//! Pure client-side navigation over the scrollback timeline
//! ([`crate::scrollback::state::TimelineEntry`]): moving the cursor
//! live-scrolls the transcript to the hovered turn, Enter jumps there,
//! Esc restores the viewport the picker opened from. Unlike `/rewind`
//! nothing is fetched and nothing is mutated.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::render::line_utils::truncate_str;
use crate::scrollback::entry::EntryId;
use crate::scrollback::state::{ScrollAnchor, TimelineEntry};
use crate::theme::Theme;
use crate::views::overlay_list::ListOverlay;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JumpRestore {
    pub(crate) bookmark: Option<ScrollAnchor>,
    pub selected: Option<usize>,
    pub follow_mode: bool,
}

#[derive(Debug)]
pub struct JumpState {
    pub entries: Vec<TimelineEntry>,
    pub selected: usize,
    pub restore: JumpRestore,
}

impl JumpState {
    fn list(&self) -> ListOverlay {
        ListOverlay {
            len: self.entries.len(),
            selected: self.selected,
        }
    }
}

pub enum JumpInput {
    Select(EntryId),
    Dismissed,
    MoveUp,
    MoveDown,
    Consumed,
}

pub fn handle_jump_key(state: &JumpState, key: &KeyEvent) -> JumpInput {
    if key.kind == crossterm::event::KeyEventKind::Release {
        return JumpInput::Consumed;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => JumpInput::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => JumpInput::MoveUp,
        KeyCode::Enter => jump_activate(state),
        KeyCode::Esc => JumpInput::Dismissed,
        _ => JumpInput::Consumed,
    }
}

pub fn move_cursor(state: &mut JumpState, delta: i32) {
    if state.entries.is_empty() {
        return;
    }
    let max = state.entries.len() as i32 - 1;
    state.selected = (state.selected as i32 + delta).clamp(0, max) as usize;
}

pub fn set_jump_cursor(state: &mut JumpState, idx: usize) -> bool {
    if state.entries.is_empty() {
        return false;
    }
    let new = idx.min(state.entries.len() - 1);
    if state.selected != new {
        state.selected = new;
        true
    } else {
        false
    }
}

pub fn jump_activate(state: &JumpState) -> JumpInput {
    state
        .entries
        .get(state.selected)
        .map(|entry| JumpInput::Select(entry.prompt_entry_id))
        .unwrap_or(JumpInput::Consumed)
}

pub fn jump_row_at(state: &JumpState, area: Rect, col: u16, row: u16) -> Option<usize> {
    state.list().row_at(area, col, row)
}

pub fn jump_overlay_height(state: &JumpState, screen_h: u16) -> u16 {
    state.list().height(screen_h)
}

pub fn render_jump_overlay(buf: &mut Buffer, area: Rect, state: &JumpState, focused: bool) {
    let theme = Theme::current();
    let ord_width = state.entries.len().to_string().len();

    state
        .list()
        .render(buf, area, "Jump to which turn?", focused, |index, ctx| {
            let entry = &state.entries[index];
            let ordinal = format!("{:>ord_width$} ", entry.turn_idx + 1);
            let ord_style = Style::default().fg(theme.gray).bg(ctx.row_bg);
            let preview = if entry.preview.is_empty() {
                "(no preview)".to_string()
            } else {
                truncate_str(
                    &entry.preview,
                    ctx.content_width.saturating_sub(ord_width as u16 + 3) as usize,
                )
            };
            let text_style = Style::default()
                .fg(theme.text_primary)
                .bg(ctx.row_bg)
                .add_modifier(if ctx.is_cursor {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
            Line::from(vec![
                Span::styled(ordinal, ord_style),
                Span::styled(preview, text_style),
            ])
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyModifiers};

    fn state(turns: usize) -> JumpState {
        JumpState {
            entries: (0..turns)
                .map(|turn_idx| TimelineEntry {
                    turn_idx,
                    prompt_entry_id: EntryId::new(turn_idx as u64),
                    preview: format!("turn {turn_idx}"),
                })
                .collect(),
            selected: 0,
            restore: JumpRestore {
                bookmark: None,
                selected: None,
                follow_mode: false,
            },
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }
    }

    #[test]
    fn keyboard_maps_navigation_and_selection() {
        let state = state(3);
        assert!(matches!(
            handle_jump_key(&state, &key(KeyCode::Down)),
            JumpInput::MoveDown
        ));
        assert!(matches!(
            handle_jump_key(&state, &key(KeyCode::Char('k'))),
            JumpInput::MoveUp
        ));
        assert!(matches!(
            handle_jump_key(&state, &key(KeyCode::Enter)),
            JumpInput::Select(id) if id == EntryId::new(0)
        ));
        assert!(matches!(
            handle_jump_key(&state, &key(KeyCode::Esc)),
            JumpInput::Dismissed
        ));
    }

    #[test]
    fn cursor_is_clamped_to_available_turns() {
        let mut state = state(3);
        move_cursor(&mut state, 10);
        assert_eq!(state.selected, 2);
        move_cursor(&mut state, -10);
        assert_eq!(state.selected, 0);
        assert!(set_jump_cursor(&mut state, 2));
        assert!(!set_jump_cursor(&mut state, 99));
    }
}
