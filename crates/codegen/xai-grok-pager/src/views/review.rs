//! Session / message code-review modal (PSM code-review → native Pager).
//!
//! Left: filterable file list. Right: embedded [`BlockViewerPane`] (same TUI as
//! Enter-on-edit: scroll, search, filter, wrap, copy, line-numbered diffs).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use similar::ChangeTag;

use crate::render::line_utils::truncate_str;
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::ToolCallBlock;
use crate::scrollback::entry::{EntryId, ScrollbackEntry};
use crate::scrollback::state::ScrollbackState;
use crate::theme::Theme;
use crate::views::block_viewer::BlockViewerPane;

/// Which tool ops appear in the review list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReviewKindFilter {
    #[default]
    Changes,
    #[allow(dead_code)]
    Reads,
    #[allow(dead_code)]
    Shell,
    #[allow(dead_code)]
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewFileKind {
    Edit,
    Write,
    #[allow(dead_code)]
    Read,
    #[allow(dead_code)]
    Shell,
}

#[derive(Debug, Clone)]
pub struct ReviewFileItem {
    pub path: String,
    pub kind: ReviewFileKind,
    /// Scrollback entry id for the primary (latest) edit op on this path.
    pub entry_id: EntryId,
    pub additions: usize,
    pub deletions: usize,
    pub is_error: bool,
    pub op_count: usize,
    /// Fallback plain text if `for_edit` cannot open (empty hunks).
    pub plain_fallback: String,
}

/// Focus target inside the review modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewFocus {
    /// Left file list (j/k change files; `/` filters list).
    List,
    /// Right BlockViewerPane (full edit-viewer TUI).
    Preview,
}

/// One visible row in the left pane (flat or tree).
#[derive(Debug, Clone)]
pub struct ReviewTreeRow {
    pub depth: u16,
    pub label: String,
    /// `Some(i)` → `files[i]` leaf; `None` → directory header.
    pub file_idx: Option<usize>,
    pub additions: usize,
    pub deletions: usize,
    pub kind: Option<ReviewFileKind>,
    pub is_error: bool,
    pub is_dir: bool,
}

pub struct ReviewState {
    pub title: String,
    pub files: Vec<ReviewFileItem>,
    /// Indices into `files` after list filter.
    pub filtered: Vec<usize>,
    /// Cursor into `nav_rows()` (filtered flat indices or tree rows).
    pub selected: usize,
    pub focus: ReviewFocus,
    pub filter: ReviewKindFilter,
    /// File-list filter query (`/` when list focused).
    pub list_query: String,
    /// True while typing into the file-list filter bar.
    pub list_filter_active: bool,
    /// Tree layout (cwd-relative + compact Java packages). Default from F2.
    pub tree_mode: bool,
    /// Session cwd used to strip absolute prefixes in tree mode.
    pub cwd: String,
    /// Built when `tree_mode` (and after filter changes).
    pub tree_rows: Vec<ReviewTreeRow>,
    /// Right-pane viewer (rebuilt when selection changes).
    pub viewer: Option<BlockViewerPane>,
    /// Hit areas from last render.
    pub list_area: Rect,
    /// Exact rows rect used for file items (inside border/title/filter bar).
    pub list_body_area: Rect,
    /// First nav-row index drawn at `list_body_area.y` last frame.
    pub list_view_start: usize,
    pub preview_area: Rect,
    pub popup_area: Rect,
}

impl ReviewState {
    pub fn new(
        title: impl Into<String>,
        files: Vec<ReviewFileItem>,
        filter: ReviewKindFilter,
    ) -> Self {
        Self::with_options(title, files, filter, false, String::new())
    }

    pub fn with_options(
        title: impl Into<String>,
        files: Vec<ReviewFileItem>,
        filter: ReviewKindFilter,
        tree_mode: bool,
        cwd: impl Into<String>,
    ) -> Self {
        let filtered: Vec<usize> = (0..files.len()).collect();
        let mut state = Self {
            title: title.into(),
            files,
            filtered,
            selected: 0,
            focus: ReviewFocus::List,
            filter,
            list_query: String::new(),
            list_filter_active: false,
            tree_mode,
            cwd: cwd.into(),
            tree_rows: Vec::new(),
            viewer: None,
            list_area: Rect::default(),
            list_body_area: Rect::default(),
            list_view_start: 0,
            preview_area: Rect::default(),
            popup_area: Rect::default(),
        };
        state.rebuild_tree();
        state
    }

    pub fn set_tree_mode(&mut self, enabled: bool) {
        if self.tree_mode == enabled {
            return;
        }
        let keep = self.current_file().map(|f| f.path.clone());
        self.tree_mode = enabled;
        self.rebuild_tree();
        if let Some(path) = keep {
            self.select_path(&path);
        } else {
            self.selected = 0;
        }
        self.viewer = None;
    }

    pub fn toggle_tree_mode(&mut self) -> bool {
        self.set_tree_mode(!self.tree_mode);
        self.tree_mode
    }

    fn rebuild_tree(&mut self) {
        if self.tree_mode {
            self.tree_rows = build_tree_rows(&self.files, &self.filtered, &self.cwd);
        } else {
            self.tree_rows.clear();
        }
        let max = self.nav_len().saturating_sub(1);
        self.selected = self.selected.min(max);
    }

    pub fn nav_len(&self) -> usize {
        if self.tree_mode {
            self.tree_rows.len()
        } else {
            self.filtered.len()
        }
    }

    pub fn current_file(&self) -> Option<&ReviewFileItem> {
        if self.tree_mode {
            let row = self.tree_rows.get(self.selected)?;
            let idx = row.file_idx.or_else(|| self.first_file_under_tree(self.selected))?;
            self.files.get(idx)
        } else {
            self.filtered
                .get(self.selected)
                .and_then(|&i| self.files.get(i))
        }
    }

    fn first_file_under_tree(&self, start: usize) -> Option<usize> {
        self.tree_rows[start..]
            .iter()
            .find_map(|r| r.file_idx)
    }

    fn select_path(&mut self, path: &str) {
        if self.tree_mode {
            if let Some(i) = self.tree_rows.iter().position(|r| {
                r.file_idx
                    .and_then(|fi| self.files.get(fi))
                    .is_some_and(|f| f.path == path)
            }) {
                self.selected = i;
            }
        } else if let Some(i) = self.filtered.iter().position(|&fi| {
            self.files.get(fi).is_some_and(|f| f.path == path)
        }) {
            self.selected = i;
        }
    }

    pub fn apply_list_filter(&mut self) {
        let keep = self.current_file().map(|f| f.path.clone());
        let q = self.list_query.to_lowercase();
        if q.is_empty() {
            self.filtered = (0..self.files.len()).collect();
        } else {
            self.filtered = self
                .files
                .iter()
                .enumerate()
                .filter(|(_, f)| f.path.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
        self.rebuild_tree();
        if let Some(path) = keep {
            self.select_path(&path);
        } else {
            self.selected = 0;
        }
        self.viewer = None;
    }

    pub fn move_sel(&mut self, delta: i32) {
        let len = self.nav_len();
        if len == 0 {
            return;
        }
        let max = len as i32 - 1;
        let next = (self.selected as i32 + delta).clamp(0, max) as usize;
        if next != self.selected {
            self.selected = next;
            self.viewer = None;
        }
    }

    pub fn select_filtered(&mut self, idx: usize) {
        let len = self.nav_len();
        if len == 0 {
            return;
        }
        let new = idx.min(len - 1);
        if new != self.selected {
            self.selected = new;
            self.viewer = None;
        }
    }

    /// Build / refresh the right-pane BlockViewerPane for the current file.
    pub fn ensure_viewer(&mut self, scrollback: &ScrollbackState) {
        let Some(file) = self.current_file().cloned() else {
            self.viewer = None;
            return;
        };
        if self
            .viewer
            .as_ref()
            .is_some_and(|v| v.entry_id == file.entry_id)
        {
            return;
        }
        let entry = scrollback.get_by_id(file.entry_id);
        self.viewer = match entry {
            // Prefer dual-gutter edit viewer (search/filter/wrap/copy + line nos).
            Some(e) => BlockViewerPane::for_edit_review(file.entry_id, e).or_else(|| {
                Some(plain_review_fallback(&file.path, &file.plain_fallback))
            }),
            None => Some(plain_review_fallback(&file.path, &file.plain_fallback)),
        };
    }
}

/// Plain-text fallback when an edit entry has no hunks (still searchable/wrappable).
fn plain_review_fallback(path: &str, text: &str) -> BlockViewerPane {
    // Number each line so empty-hunk writes still show a line gutter.
    let numbered = text
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:>4} │ {}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");
    let body = if numbered.is_empty() {
        text.to_string()
    } else {
        numbered
    };
    BlockViewerPane::for_plain_text(path, &body)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewInput {
    Dismissed,
    /// Selection / focus changed — caller should ensure_viewer.
    Changed,
    /// Tree/flat toggled — caller persists `SetReviewFileTree(tree_mode)`.
    ToggleTree,
    Consumed,
}

pub fn handle_review_list_key(state: &mut ReviewState, key: &KeyEvent) -> ReviewInput {
    if key.kind == crossterm::event::KeyEventKind::Release {
        return ReviewInput::Consumed;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return ReviewInput::Dismissed;
    }

    // List filter bar active.
    if state.list_filter_active {
        match key.code {
            KeyCode::Esc => {
                state.list_filter_active = false;
                state.list_query.clear();
                state.apply_list_filter();
                return ReviewInput::Changed;
            }
            KeyCode::Enter => {
                state.list_filter_active = false;
                return ReviewInput::Changed;
            }
            KeyCode::Backspace => {
                state.list_query.pop();
                state.apply_list_filter();
                return ReviewInput::Changed;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.list_query.push(c);
                state.apply_list_filter();
                return ReviewInput::Changed;
            }
            _ => return ReviewInput::Consumed,
        }
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => ReviewInput::Dismissed,
        KeyCode::Tab | KeyCode::Right | KeyCode::Enter => {
            state.focus = ReviewFocus::Preview;
            ReviewInput::Changed
        }
        KeyCode::Char('/') => {
            state.list_filter_active = true;
            ReviewInput::Changed
        }
        // t toggles tree/flat; caller persists via SetReviewFileTree.
        KeyCode::Char('t') if key.modifiers == KeyModifiers::NONE => ReviewInput::ToggleTree,
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_sel(-1);
            ReviewInput::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_sel(1);
            ReviewInput::Changed
        }
        KeyCode::PageUp => {
            state.move_sel(-10);
            ReviewInput::Changed
        }
        KeyCode::PageDown => {
            state.move_sel(10);
            ReviewInput::Changed
        }
        KeyCode::Home | KeyCode::Char('g') => {
            state.select_filtered(0);
            ReviewInput::Changed
        }
        KeyCode::End | KeyCode::Char('G') => {
            let last = state.nav_len().saturating_sub(1);
            state.select_filtered(last);
            ReviewInput::Changed
        }
        KeyCode::Char('n') => {
            state.move_sel(1);
            ReviewInput::Changed
        }
        KeyCode::Char('p') => {
            state.move_sel(-1);
            ReviewInput::Changed
        }
        _ => ReviewInput::Consumed,
    }
}

/// Keys handled at the review shell when preview is focused (before routing to viewer).
pub fn handle_review_preview_shell_key(state: &mut ReviewState, key: &KeyEvent) -> Option<ReviewInput> {
    if key.kind == crossterm::event::KeyEventKind::Release {
        return Some(ReviewInput::Consumed);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(ReviewInput::Dismissed);
    }

    // Don't steal keys while viewer is in search/filter/visual mode.
    if let Some(v) = state.viewer.as_ref() {
        if v.list_state.input_mode().is_some() || v.list_state.visual_mode {
            return None;
        }
    }

    match key.code {
        KeyCode::Left | KeyCode::Char('h') => {
            state.focus = ReviewFocus::List;
            Some(ReviewInput::Changed)
        }
        // n/p switch files without leaving preview focus.
        KeyCode::Char('n') if key.modifiers == KeyModifiers::NONE => {
            state.move_sel(1);
            Some(ReviewInput::Changed)
        }
        KeyCode::Char('p') if key.modifiers == KeyModifiers::NONE => {
            state.move_sel(-1);
            Some(ReviewInput::Changed)
        }
        KeyCode::Char('t') if key.modifiers == KeyModifiers::NONE => {
            Some(ReviewInput::ToggleTree)
        }
        KeyCode::Tab => {
            state.focus = ReviewFocus::List;
            Some(ReviewInput::Changed)
        }
        _ => None,
    }
}

fn count_hunk_changes(hunks: &[crate::diff::DiffHunk]) -> (usize, usize) {
    let mut additions = 0;
    let mut deletions = 0;
    for hunk in hunks {
        for line in hunk {
            match line.tag {
                ChangeTag::Insert => additions += 1,
                ChangeTag::Delete => deletions += 1,
                ChangeTag::Equal => {}
            }
        }
    }
    (additions, deletions)
}

/// Extract reviewable file ops. `range` = entry indices; `None` = whole session.
pub fn extract_review_files(
    scrollback: &ScrollbackState,
    range: Option<std::ops::Range<usize>>,
    filter: ReviewKindFilter,
) -> Vec<ReviewFileItem> {
    if matches!(filter, ReviewKindFilter::Reads | ReviewKindFilter::Shell) {
        return Vec::new();
    }

    let (start, end) = match range {
        Some(r) => (r.start, r.end.min(scrollback.len())),
        None => (0, scrollback.len()),
    };

    let mut by_path: Vec<(String, ReviewFileItem)> = Vec::new();
    let mut index_of = std::collections::HashMap::<String, usize>::new();

    for (idx, (entry_id, entry)) in scrollback.iter_entries().enumerate() {
        if idx < start || idx >= end {
            continue;
        }
        let RenderBlock::ToolCall(ToolCallBlock::Edit(edit)) = &entry.block else {
            continue;
        };

        let is_write = edit.prefix.starts_with("Creating");
        let kind = if is_write {
            ReviewFileKind::Write
        } else {
            ReviewFileKind::Edit
        };
        let path = edit.path.clone();
        let (additions, deletions) = count_hunk_changes(&edit.hunks);
        let plain_fallback = if edit.hunks.is_empty() {
            if is_write {
                format!("(new file) {path}\n")
            } else {
                format!("(no diff captured) {path}\n")
            }
        } else {
            edit.copy_text()
        };

        if let Some(&i) = index_of.get(&path) {
            let item = &mut by_path[i].1;
            item.op_count += 1;
            item.additions += additions;
            item.deletions += deletions;
            if edit.error.is_some() {
                item.is_error = true;
            }
            // Prefer latest op as the openable entry.
            item.entry_id = entry_id;
            item.kind = kind;
            item.plain_fallback = plain_fallback;
        } else {
            index_of.insert(path.clone(), by_path.len());
            by_path.push((
                path.clone(),
                ReviewFileItem {
                    path,
                    kind,
                    entry_id,
                    additions,
                    deletions,
                    is_error: edit.error.is_some(),
                    op_count: 1,
                    plain_fallback,
                },
            ));
        }
    }

    by_path.into_iter().map(|(_, item)| item).collect()
}

pub fn turn_range_for_prompt(
    scrollback: &ScrollbackState,
    prompt_id: EntryId,
) -> Option<std::ops::Range<usize>> {
    let entry_idx = scrollback.index_of_id(prompt_id)?;
    scrollback
        .turns()
        .iter()
        .find(|t| t.prompt_index == entry_idx)
        .map(|t| t.range())
}

/// Render modal chrome + file list + BlockViewer content.
pub fn render_review_modal(
    buf: &mut Buffer,
    area: Rect,
    state: &mut ReviewState,
    scrollback: &ScrollbackState,
) {
    let theme = Theme::current();
    if area.width < 20 || area.height < 6 {
        return;
    }

    state.ensure_viewer(scrollback);
    state.popup_area = area;

    let base = Style::default().fg(theme.text_primary).bg(theme.bg_base);
    Clear.render(area, buf);
    buf.set_style(area, base);

    let footer = match state.focus {
        ReviewFocus::List if state.list_filter_active => {
            format!(" filter: {}  Enter=done  Esc=clear ", state.list_query)
        }
        ReviewFocus::List => {
            let layout = if state.tree_mode { "tree" } else { "flat" };
            format!(
                " j/k  /=filter  t={layout}  Enter→preview  n/p  Esc "
            )
        }
        ReviewFocus::Preview => {
            " j/k scroll  /=search  f=filter  w=wrap  y=copy  n/p file  ← list  Esc close "
                .into()
        }
    };

    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.gray_dim))
        .title(Span::styled(
            format!(" {} ", state.title),
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(footer, Style::default().fg(theme.gray)));
    let inner = border.inner(area);
    border.render(area, buf);
    if inner.width < 10 || inner.height < 3 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
        .split(inner);

    state.list_area = chunks[0];
    state.preview_area = chunks[1];

    render_file_list(buf, chunks[0], state, &theme);
    render_preview_pane(buf, chunks[1], state, scrollback, &theme);
}

fn render_file_list(buf: &mut Buffer, area: Rect, state: &mut ReviewState, theme: &Theme) {
    let focused = state.focus == ReviewFocus::List;
    // TOP|RIGHT so the title owns a real top border row — content y matches hit-test.
    let list_border = Block::default()
        .borders(Borders::TOP | Borders::RIGHT)
        .border_style(Style::default().fg(theme.gray_dim))
        .title(Span::styled(
            format!(
                " {} ({}/{}) ",
                if state.tree_mode { "Tree" } else { "Files" },
                state.nav_len(),
                state.files.len()
            ),
            Style::default().fg(if focused {
                theme.accent_tool
            } else {
                theme.gray
            }),
        ));
    let list_inner = list_border.inner(area);
    list_border.render(area, buf);

    // Filter bar row at bottom of list when active / non-empty query.
    let (rows_area, filter_row) = if state.list_filter_active || !state.list_query.is_empty() {
        if list_inner.height < 2 {
            (list_inner, None)
        } else {
            let body = Rect::new(
                list_inner.x,
                list_inner.y,
                list_inner.width,
                list_inner.height.saturating_sub(1),
            );
            let bar = Rect::new(
                list_inner.x,
                list_inner.y + list_inner.height.saturating_sub(1),
                list_inner.width,
                1,
            );
            (body, Some(bar))
        }
    } else {
        (list_inner, None)
    };

    // Cache exact body geometry for mouse hit-testing (must match draw loop).
    state.list_body_area = rows_area;

    if let Some(bar) = filter_row {
        let label = if state.list_filter_active {
            format!("/{}", state.list_query)
        } else {
            format!("~{}", state.list_query)
        };
        let line = Line::from(Span::styled(
            truncate_str(&label, bar.width as usize),
            Style::default().fg(theme.accent_tool).bg(theme.bg_dark),
        ));
        buf.set_style(bar, Style::default().bg(theme.bg_dark));
        buf.set_line(bar.x, bar.y, &line, bar.width);
    }

    let nav_len = state.nav_len();
    if nav_len == 0 {
        state.list_view_start = 0;
        let empty = Paragraph::new(Line::from(Span::styled(
            if state.files.is_empty() {
                "No file changes"
            } else {
                "No match"
            },
            Style::default().fg(theme.gray),
        )));
        empty.render(rows_area, buf);
        return;
    }

    let visible = rows_area.height as usize;
    let start = state
        .selected
        .saturating_sub(visible.saturating_sub(1) / 2);
    state.list_view_start = start;
    let end = (start + visible).min(nav_len);

    for (row, nav_i) in (start..end).enumerate() {
        let y = rows_area.y + row as u16;
        if y >= rows_area.y + rows_area.height {
            break;
        }
        let selected = nav_i == state.selected;
        let row_bg = if selected {
            theme.bg_highlight
        } else {
            theme.bg_base
        };
        let row_area = Rect::new(rows_area.x, y, rows_area.width, 1);
        buf.set_style(row_area, Style::default().bg(row_bg));

        if state.tree_mode {
            render_tree_row(buf, rows_area.x, y, rows_area.width, &state.tree_rows[nav_i], selected, row_bg, theme);
        } else {
            let file_i = state.filtered[nav_i];
            let file = &state.files[file_i];
            render_flat_row(buf, rows_area.x, y, rows_area.width, file, selected, row_bg, theme);
        }
    }
}

fn render_flat_row(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    file: &ReviewFileItem,
    selected: bool,
    row_bg: ratatui::style::Color,
    theme: &Theme,
) {
    let kind_tag = kind_tag(file.kind);
    let stats = format!(" +{} -{}", file.additions, file.deletions);
    let name = truncate_str(
        &basename(&file.path),
        width.saturating_sub(stats.len() as u16 + 4) as usize,
    );
    let kind_fg = kind_fg(file.kind, theme);
    let mut spans = vec![
        Span::styled(
            format!("{kind_tag} "),
            Style::default().fg(kind_fg).bg(row_bg),
        ),
        Span::styled(
            name,
            Style::default()
                .fg(if file.is_error {
                    theme.accent_error
                } else {
                    theme.text_primary
                })
                .bg(row_bg)
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ];
    if width > stats.len() as u16 + 6 {
        spans.push(Span::styled(
            stats,
            Style::default().fg(theme.gray).bg(row_bg),
        ));
    }
    buf.set_line(x, y, &Line::from(spans), width);
}

fn render_tree_row(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    row: &ReviewTreeRow,
    selected: bool,
    row_bg: ratatui::style::Color,
    theme: &Theme,
) {
    let indent = "  ".repeat(row.depth as usize);
    let glyph = if row.is_dir { "▸ " } else { "  " };
    let tag = row.kind.map(kind_tag).unwrap_or(" ");
    let stats = if row.additions + row.deletions > 0 {
        format!(" +{} -{}", row.additions, row.deletions)
    } else {
        String::new()
    };
    let prefix = format!("{indent}{glyph}{tag} ");
    let name_w = width.saturating_sub(prefix.len() as u16 + stats.len() as u16);
    let name = truncate_str(&row.label, name_w as usize);
    let kind_fg = row.kind.map(|k| kind_fg(k, theme)).unwrap_or(theme.gray_bright);
    let mut spans = vec![
        Span::styled(prefix, Style::default().fg(kind_fg).bg(row_bg)),
        Span::styled(
            name,
            Style::default()
                .fg(if row.is_error {
                    theme.accent_error
                } else if row.is_dir {
                    theme.gray_bright
                } else {
                    theme.text_primary
                })
                .bg(row_bg)
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ];
    if !stats.is_empty() && width > stats.len() as u16 + 6 {
        spans.push(Span::styled(
            stats,
            Style::default().fg(theme.gray).bg(row_bg),
        ));
    }
    buf.set_line(x, y, &Line::from(spans), width);
}

fn kind_tag(kind: ReviewFileKind) -> &'static str {
    match kind {
        ReviewFileKind::Write => "+",
        ReviewFileKind::Edit => "~",
        ReviewFileKind::Read => "r",
        ReviewFileKind::Shell => "$",
    }
}

fn kind_fg(kind: ReviewFileKind, theme: &Theme) -> ratatui::style::Color {
    match kind {
        ReviewFileKind::Write => theme.accent_success,
        ReviewFileKind::Edit => theme.warning,
        _ => theme.gray,
    }
}

fn render_preview_pane(
    buf: &mut Buffer,
    area: Rect,
    state: &mut ReviewState,
    scrollback: &ScrollbackState,
    theme: &Theme,
) {
    let focused = state.focus == ReviewFocus::Preview;
    let title = state
        .current_file()
        .map(|f| {
            let ops = if f.op_count > 1 {
                format!(" ×{}", f.op_count)
            } else {
                String::new()
            };
            format!(" {}{} ", f.path, ops)
        })
        .unwrap_or_else(|| " (no selection) ".into());

    let border = Block::default().title(Span::styled(
        title,
        Style::default().fg(if focused {
            theme.accent_tool
        } else {
            theme.gray
        }),
    ));
    let inner = border.inner(area);
    border.render(area, buf);

    let Some(viewer) = state.viewer.as_mut() else {
        let empty = Paragraph::new(Line::from(Span::styled(
            "Select a file",
            Style::default().fg(theme.gray),
        )));
        empty.render(inner, buf);
        return;
    };

    // Dummy entry for plain-text / missing; prefer live scrollback entry.
    let entry_owned;
    let entry: &ScrollbackEntry = if let Some(e) = scrollback.get_by_id(viewer.entry_id) {
        e
    } else {
        entry_owned = ScrollbackEntry::new(RenderBlock::system(String::new()));
        &entry_owned
    };

    // No preamble in review sidebar — full height for ListPane content.
    let prepend: &[Line<'static>] = &[];
    viewer.render_content(inner, buf, entry, focused, prepend);
}

/// Map a mouse position to a filtered file index using last-frame draw geometry.
pub fn list_row_at(state: &ReviewState, col: u16, row: u16) -> Option<usize> {
    let area = state.list_body_area;
    if area.width == 0 || area.height == 0 {
        return None;
    }
    if col < area.x || col >= area.x + area.width || row < area.y || row >= area.y + area.height {
        return None;
    }
    let offset = (row - area.y) as usize;
    let idx = state.list_view_start.saturating_add(offset);
    (idx < state.nav_len()).then_some(idx)
}

pub fn handle_review_mouse(state: &mut ReviewState, mouse: &MouseEvent) -> ReviewInput {
    match mouse.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let delta: i32 = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                -3
            } else {
                3
            };
            let step = delta.signum();
            // Scroll the pane under the cursor.
            if point_in(state.list_area, mouse.column, mouse.row) {
                state.move_sel(step);
                return ReviewInput::Changed;
            }
            if point_in(state.preview_area, mouse.column, mouse.row) {
                state.focus = ReviewFocus::Preview;
                if let Some(v) = state.viewer.as_mut() {
                    v.handle_scroll(delta);
                }
                return ReviewInput::Changed;
            }
            // Fallback: scroll focused pane.
            match state.focus {
                ReviewFocus::List => {
                    state.move_sel(step);
                    ReviewInput::Changed
                }
                ReviewFocus::Preview => {
                    if let Some(v) = state.viewer.as_mut() {
                        v.handle_scroll(delta);
                    }
                    ReviewInput::Changed
                }
            }
        }
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            if let Some(idx) = list_row_at(state, mouse.column, mouse.row) {
                state.select_filtered(idx);
                // Click selects and jumps to preview (edit-viewer TUI).
                state.focus = ReviewFocus::Preview;
                return ReviewInput::Changed;
            }
            if point_in(state.preview_area, mouse.column, mouse.row) {
                state.focus = ReviewFocus::Preview;
                if let Some(v) = state.viewer.as_mut() {
                    v.handle_mouse(mouse.kind, mouse.column, mouse.row);
                }
                return ReviewInput::Changed;
            }
            ReviewInput::Consumed
        }
        MouseEventKind::Drag(_) | MouseEventKind::Up(_) | MouseEventKind::Moved => {
            if state.focus == ReviewFocus::Preview
                && let Some(v) = state.viewer.as_mut()
            {
                v.handle_mouse(mouse.kind, mouse.column, mouse.row);
                return ReviewInput::Changed;
            }
            ReviewInput::Consumed
        }
        _ => ReviewInput::Consumed,
    }
}

fn point_in(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x
        && col < area.x + area.width
        && row >= area.y
        && row < area.y + area.height
}

fn basename(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_string()
}

// ---------------------------------------------------------------------------
// Tree builder: cwd-relative paths + compact single-child / Java packages
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TrieNode {
    children: std::collections::BTreeMap<String, TrieNode>,
    /// File leaf attached at this segment (basename).
    file_idx: Option<usize>,
    additions: usize,
    deletions: usize,
    kind: Option<ReviewFileKind>,
    is_error: bool,
}

/// Strip `cwd` prefix when path is under the session working directory.
pub fn strip_cwd_prefix(path: &str, cwd: &str) -> String {
    let path_n = path.replace('\\', "/");
    let cwd_n = cwd.replace('\\', "/").trim_end_matches('/').to_string();
    if cwd_n.is_empty() {
        return path_n;
    }
    if let Some(rest) = path_n.strip_prefix(&cwd_n) {
        let rest = rest.trim_start_matches('/');
        if !rest.is_empty() {
            return rest.to_string();
        }
    }
    path_n
}

fn is_java_package_seg(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    s.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Join compacted segment chain: consecutive java package segs with `.`, else `/`.
pub fn compact_join(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let mut out = parts[0].clone();
    for i in 1..parts.len() {
        let prev = &parts[i - 1];
        let cur = &parts[i];
        if is_java_package_seg(prev) && is_java_package_seg(cur) {
            out.push('.');
            out.push_str(cur);
        } else {
            out.push('/');
            out.push_str(cur);
        }
    }
    out
}

/// Build tree rows for the filtered file set.
pub fn build_tree_rows(
    files: &[ReviewFileItem],
    filtered: &[usize],
    cwd: &str,
) -> Vec<ReviewTreeRow> {
    let mut root = TrieNode::default();
    for &fi in filtered {
        let Some(file) = files.get(fi) else {
            continue;
        };
        let rel = strip_cwd_prefix(&file.path, cwd);
        let segs: Vec<String> = rel
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if segs.is_empty() {
            continue;
        }
        let mut node = &mut root;
        for (i, seg) in segs.iter().enumerate() {
            node = node.children.entry(seg.clone()).or_default();
            if i + 1 == segs.len() {
                node.file_idx = Some(fi);
                node.additions = file.additions;
                node.deletions = file.deletions;
                node.kind = Some(file.kind);
                node.is_error = file.is_error;
            }
        }
    }

    let mut rows = Vec::new();
    flatten_trie(&root, 0, &mut rows);
    rows
}

fn flatten_trie(node: &TrieNode, depth: u16, rows: &mut Vec<ReviewTreeRow>) {
    // Root is virtual — only walk children.
    for (name, child) in &node.children {
        flatten_node(name, child, depth, rows);
    }
}

fn flatten_node(name: &str, node: &TrieNode, depth: u16, rows: &mut Vec<ReviewTreeRow>) {
    // Compact single-child pure-dir chains (Java packages, nested folders).
    let mut parts = vec![name.to_string()];
    let mut cur = node;
    loop {
        if cur.file_idx.is_some() || cur.children.len() != 1 {
            break;
        }
        let (child_name, child) = cur.children.iter().next().unwrap();
        // Stop before absorbing a pure file leaf into the chain label.
        if child.file_idx.is_some() && child.children.is_empty() {
            break;
        }
        if child.file_idx.is_none() {
            parts.push(child_name.clone());
            cur = child;
            continue;
        }
        break;
    }

    let label = compact_join(&parts);

    // Pure file leaf.
    if cur.file_idx.is_some() && cur.children.is_empty() {
        rows.push(ReviewTreeRow {
            depth,
            label,
            file_idx: cur.file_idx,
            additions: cur.additions,
            deletions: cur.deletions,
            kind: cur.kind,
            is_error: cur.is_error,
            is_dir: false,
        });
        return;
    }

    // Directory header (aggregated stats), then children.
    let (add, del, err) = aggregate_stats(cur);
    rows.push(ReviewTreeRow {
        depth,
        label,
        file_idx: None,
        additions: add,
        deletions: del,
        kind: None,
        is_error: err,
        is_dir: true,
    });

    for (child_name, child) in &cur.children {
        flatten_node(child_name, child, depth + 1, rows);
    }
}

fn aggregate_stats(node: &TrieNode) -> (usize, usize, bool) {
    let mut add = node.additions;
    let mut del = node.deletions;
    let mut err = node.is_error;
    for child in node.children.values() {
        let (a, d, e) = aggregate_stats(child);
        add += a;
        del += d;
        err |= e;
    }
    (add, del, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::DiffLine;
    use crate::scrollback::blocks::tool::edit::EditToolCallBlock;
    use crate::scrollback::blocks::tool::execute::ExecuteToolCallBlock;
    use crate::scrollback::blocks::tool::read::ReadToolCallBlock;
    use crate::scrollback::entry::ScrollbackEntry;

    fn edit_entry(path: &str, write: bool) -> ScrollbackEntry {
        let hunk = vec![
            DiffLine {
                text: "old\n".into(),
                lo: 1,
                ln: 1,
                tag: ChangeTag::Delete,
            },
            DiffLine {
                text: "new\n".into(),
                lo: 1,
                ln: 1,
                tag: ChangeTag::Insert,
            },
        ];
        let mut block = EditToolCallBlock::new(path, vec![hunk]);
        if write {
            block = block.with_prefix("Creating ");
        }
        ScrollbackEntry::new(RenderBlock::ToolCall(ToolCallBlock::Edit(block)))
    }

    #[test]
    fn extract_skips_read_and_bash_by_default() {
        let mut sb = ScrollbackState::new();
        sb.push(ScrollbackEntry::new(RenderBlock::user_prompt("hi")));
        sb.push(edit_entry("a.rs", false));
        sb.push(edit_entry("b.rs", true));
        sb.push(ScrollbackEntry::new(RenderBlock::ToolCall(
            ToolCallBlock::Read(ReadToolCallBlock::new("c.rs")),
        )));
        sb.push(ScrollbackEntry::new(RenderBlock::ToolCall(
            ToolCallBlock::Execute(ExecuteToolCallBlock::new("ls")),
        )));

        let files = extract_review_files(&sb, None, ReviewKindFilter::Changes);
        let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a.rs", "b.rs"]);
        assert!(matches!(files[0].kind, ReviewFileKind::Edit));
        assert!(matches!(files[1].kind, ReviewFileKind::Write));
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[0].deletions, 1);
        assert!(sb.get_by_id(files[0].entry_id).is_some());
    }

    #[test]
    fn extract_merges_same_path_keeps_latest_entry() {
        let mut sb = ScrollbackState::new();
        let id1 = sb.push(edit_entry("a.rs", false));
        let id2 = sb.push(edit_entry("a.rs", false));
        let files = extract_review_files(&sb, None, ReviewKindFilter::Changes);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].op_count, 2);
        assert_eq!(files[0].entry_id, id2);
        assert_ne!(files[0].entry_id, id1);
    }

    #[test]
    fn list_filter_narrows_files() {
        let mut sb = ScrollbackState::new();
        sb.push(edit_entry("src/foo.rs", false));
        sb.push(edit_entry("src/bar.ts", true));
        let files = extract_review_files(&sb, None, ReviewKindFilter::Changes);
        let mut state = ReviewState::new("t", files, ReviewKindFilter::Changes);
        state.list_query = "foo".into();
        state.apply_list_filter();
        assert_eq!(state.filtered.len(), 1);
        assert!(state.current_file().unwrap().path.contains("foo"));
    }

    #[test]
    fn strip_cwd_and_compact_java_packages() {
        assert_eq!(
            strip_cwd_prefix("/proj/src/main/java/com/foo/Bar.java", "/proj"),
            "src/main/java/com/foo/Bar.java"
        );
        assert_eq!(
            compact_join(&[
                "com".into(),
                "example".into(),
                "app".into(),
            ]),
            "com.example.app"
        );
        assert_eq!(
            compact_join(&["src".into(), "main".into(), "java".into()]),
            "src/main/java"
        );
    }

    #[test]
    fn tree_compacts_java_chain_under_cwd() {
        let files = vec![
            ReviewFileItem {
                path: "/proj/src/main/java/com/example/app/Service.java".into(),
                kind: ReviewFileKind::Edit,
                entry_id: EntryId::new(1),
                additions: 1,
                deletions: 1,
                is_error: false,
                op_count: 1,
                plain_fallback: String::new(),
            },
            ReviewFileItem {
                path: "/proj/src/main/java/com/example/app/Repo.java".into(),
                kind: ReviewFileKind::Write,
                entry_id: EntryId::new(2),
                additions: 2,
                deletions: 0,
                is_error: false,
                op_count: 1,
                plain_fallback: String::new(),
            },
            ReviewFileItem {
                path: "/proj/utils/helper.rs".into(),
                kind: ReviewFileKind::Edit,
                entry_id: EntryId::new(3),
                additions: 1,
                deletions: 0,
                is_error: false,
                op_count: 1,
                plain_fallback: String::new(),
            },
        ];
        let filtered: Vec<usize> = (0..files.len()).collect();
        let rows = build_tree_rows(&files, &filtered, "/proj");
        let labels: Vec<_> = rows.iter().map(|r| r.label.as_str()).collect();
        // Single-child dir chain src→main→java compacted; java packages dotted.
        assert!(
            labels.iter().any(|l| l.contains("com.example.app") || *l == "com.example.app"),
            "expected compacted java package, got {labels:?}"
        );
        assert!(labels.iter().any(|l| *l == "Service.java"));
        assert!(labels.iter().any(|l| *l == "Repo.java"));
        assert!(labels.iter().any(|l| *l == "helper.rs" || *l == "utils"));
    }

    #[test]
    fn list_row_at_uses_body_geometry_not_outer_frame() {
        let mut sb = ScrollbackState::new();
        for name in ["a.rs", "b.rs", "c.rs", "d.rs"] {
            sb.push(edit_entry(name, false));
        }
        let files = extract_review_files(&sb, None, ReviewKindFilter::Changes);
        let mut state = ReviewState::new("t", files, ReviewKindFilter::Changes);
        // Simulate draw: outer list frame at y=5, body starts one row lower (title border).
        state.list_area = Rect::new(0, 5, 20, 10);
        state.list_body_area = Rect::new(0, 6, 19, 8);
        state.list_view_start = 1; // scrolled so first visible is index 1
        // Click first body row → filtered index 1
        assert_eq!(list_row_at(&state, 2, 6), Some(1));
        // Click third body row → index 3
        assert_eq!(list_row_at(&state, 2, 8), Some(3));
        // Click on title row (outside body) → miss
        assert_eq!(list_row_at(&state, 2, 5), None);
    }
}
