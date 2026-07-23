//! `/review-session` / `/review-message` dispatch.

use crate::app::actions::Effect;
use crate::app::app_view::{ActiveView, AppView};
use crate::scrollback::entry::EntryId;
use crate::views::jump::{JumpPurpose, JumpRestore, JumpState};
use crate::views::review::{
    ReviewKindFilter, ReviewState, extract_review_files, turn_range_for_prompt,
};

pub(super) fn dispatch_review_show_session(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    if agent.review_state.is_some() {
        return vec![];
    }

    let filter = if app.current_ui.review_include_reads {
        ReviewKindFilter::All
    } else {
        ReviewKindFilter::Changes
    };
    let files = extract_review_files(&agent.scrollback, None, filter);
    if files.is_empty() {
        app.show_toast(if filter.includes_reads() {
            "No file ops to review in this session"
        } else {
            "No file changes to review in this session"
        });
        return vec![];
    }
    let tree = app.current_ui.review_file_tree;
    let cwd = agent.session.cwd.display().to_string();
    let mut state = ReviewState::with_options_range(
        format!("Session review · {} file(s)", files.len()),
        files,
        filter,
        tree,
        cwd,
        None,
    );
    state.ensure_viewer(&agent.scrollback);
    agent.review_state = Some(state);
    vec![]
}

pub(super) fn dispatch_review_show_message_picker(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    if agent.jump_slot_taken() || agent.review_state.is_some() {
        return vec![];
    }

    let entries = agent.scrollback.timeline_entries();
    if entries.is_empty() {
        app.show_toast("Nothing to review yet");
        return vec![];
    }

    let restore = JumpRestore {
        bookmark: agent.scrollback.capture_scroll_bookmark(),
        selected: agent.scrollback.selected(),
        follow_mode: agent.scrollback.is_follow_mode(),
    };
    let selected = agent
        .scrollback
        .active_turn_for_viewport()
        .unwrap_or(entries.len() - 1)
        .min(entries.len() - 1);

    let preview_id = entries[selected].prompt_entry_id;
    agent.jump_state = Some(JumpState::with_purpose(
        entries,
        selected,
        restore,
        JumpPurpose::Review,
    ));
    if let Some(index) = agent.scrollback.index_of_id(preview_id) {
        agent.scrollback.scroll_to_entry_top(index);
    }
    vec![]
}

pub(super) fn dispatch_review_open_for_turn(app: &mut AppView, prompt_id: EntryId) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };

    // Close the jump picker first (keep viewport at jumped turn).
    if let Some(state) = agent.jump_state.take() {
        // Drop restore intentionally: selection already scrolled to the turn.
        let _ = state;
    }

    let range = turn_range_for_prompt(&agent.scrollback, prompt_id);
    let filter = if app.current_ui.review_include_reads {
        ReviewKindFilter::All
    } else {
        ReviewKindFilter::Changes
    };
    let files = extract_review_files(&agent.scrollback, range.clone(), filter);
    if files.is_empty() {
        app.show_toast(if filter.includes_reads() {
            "No file ops in this turn"
        } else {
            "No file changes in this turn"
        });
        return vec![];
    }
    let tree = app.current_ui.review_file_tree;
    let cwd = agent.session.cwd.display().to_string();
    let mut state = ReviewState::with_options_range(
        format!("Turn review · {} file(s)", files.len()),
        files,
        filter,
        tree,
        cwd,
        range,
    );
    state.ensure_viewer(&agent.scrollback);
    agent.review_state = Some(state);
    vec![]
}

pub(super) fn dispatch_review_dismiss(app: &mut AppView) -> Vec<Effect> {
    let ActiveView::Agent(id) = app.active_view else {
        return vec![];
    };
    let Some(agent) = app.agents.get_mut(&id) else {
        return vec![];
    };
    agent.review_state = None;
    vec![]
}
