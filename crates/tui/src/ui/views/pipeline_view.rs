use std::{ops::Range, time::Instant};

use action_editor_view::{ActionEditorView, EditorOutcome as ActionOutcome};
use event_editor_view::{EditorOutcome as EventOutcome, EventEditorView};

use chrono::{Local, TimeZone};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};
use simple_core::{Action, ActionState, Event};
use simple_parser::HighlightKind;
use tokio::sync::mpsc::UnboundedSender;
use tui_input::{Input, InputRequest};
use tui_widget_list::{ListBuilder, ListState, ListView};

use crate::app::{AppAction, SyncStatus};
use crate::ui::AppView;

mod action_editor_view;
mod event_editor_view;

// ---------------------------------------------------------------------------
// What kind of entity is in each list slot
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum ListEntry {
    Action(Action),
    Event(Event),
}

// ---------------------------------------------------------------------------
// Input mode + entity type toggle
// ---------------------------------------------------------------------------

enum InputMode {
    Normal,
    InsertAction,
    InsertEvent,
}

impl InputMode {
    fn is_insert(&self) -> bool {
        matches!(self, Self::InsertAction | Self::InsertEvent)
    }

    fn label(&self) -> &'static str {
        match self {
            Self::InsertAction => " New action: ",
            Self::InsertEvent => " New event: ",
            Self::Normal => "",
        }
    }

    fn border_color(&self) -> Color {
        match self {
            Self::InsertAction => Color::Green,
            Self::InsertEvent => Color::Cyan,
            Self::Normal => Color::DarkGray,
        }
    }
}

// ---------------------------------------------------------------------------
// PipelineView
// ---------------------------------------------------------------------------

enum ActivePane {
    List,
    EditAction(ActionEditorView),
    EditEvent(EventEditorView),
    NewItemPopup(usize), // selected index: 0 = Action, 1 = Event
}

pub struct PipelineView {
    tx: UnboundedSender<AppAction>,
    actions: Vec<Action>,
    events: Vec<Event>,
    entries: Vec<ListEntry>, // merged + sorted view
    list_state: ListState,
    mode: InputMode,
    input: Input,
    sync_status: SyncStatus,
    active_pane: ActivePane,
    /// Total width of the list area, updated each draw — used to compute
    /// right-aligned metadata column widths inside items.
    list_width: u16,
}

impl PipelineView {
    pub fn new(tx: UnboundedSender<AppAction>) -> Self {
        Self {
            tx,
            actions: Vec::new(),
            events: Vec::new(),
            entries: Vec::new(),
            list_state: ListState::default(),
            mode: InputMode::Normal,
            input: Input::default(),
            sync_status: SyncStatus::Idle,
            active_pane: ActivePane::List,
            list_width: 80,
        }
    }

    pub fn set_actions(&mut self, actions: Vec<Action>) {
        self.actions = actions;
        self.rebuild_entries();
    }

    pub fn set_events(&mut self, events: Vec<Event>) {
        self.events = events;
        self.rebuild_entries();
    }

    pub fn set_sync_status(&mut self, status: SyncStatus) {
        self.sync_status = status;
    }

    /// Merge actions + events into a single sorted list.
    /// Items with a target/time come first (chronological); backlog items
    /// (actions with no target date) follow, sorted alphabetically.
    fn rebuild_entries(&mut self) {
        let mut entries: Vec<ListEntry> = Vec::new();

        for a in &self.actions {
            entries.push(ListEntry::Action(a.clone()));
        }
        let now = chrono::Utc::now();
        for e in &self.events {
            let end = e.time + e.duration.unwrap_or(chrono::Duration::zero());
            if end >= now {
                entries.push(ListEntry::Event(e.clone()));
            }
        }

        entries.sort_by(|a, b| {
            let time_a = match a {
                ListEntry::Action(action) => {
                    if let ActionState::Queued(t) = action.state {
                        Some(t.time.timestamp())
                    } else {
                        None
                    }
                }
                ListEntry::Event(event) => Some(event.time.timestamp()),
            };
            let time_b = match b {
                ListEntry::Action(action) => {
                    if let ActionState::Queued(t) = action.state {
                        Some(t.time.timestamp())
                    } else {
                        None
                    }
                }
                ListEntry::Event(event) => Some(event.time.timestamp()),
            };
            match (time_a, time_b) {
                (Some(a), Some(b)) => a.cmp(&b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => {
                    let title_a = match a {
                        ListEntry::Action(x) => x.title.as_str(),
                        ListEntry::Event(x) => x.title.as_str(),
                    };
                    let title_b = match b {
                        ListEntry::Action(x) => x.title.as_str(),
                        ListEntry::Event(x) => x.title.as_str(),
                    };
                    title_a.cmp(title_b)
                }
            }
        });

        // Clamp selection so it stays valid after the list changes.
        let len = entries.len();
        if let Some(sel) = self.list_state.selected() {
            if len == 0 {
                self.list_state.select(None);
            } else if sel >= len {
                self.list_state.select(Some(len - 1));
            }
        }

        self.entries = entries;
    }

    fn select_next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(i) => (i + 1).min(self.entries.len() - 1),
            None => 0,
        };
        self.list_state.select(Some(next));
    }

    fn select_prev(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let prev = match self.list_state.selected() {
            Some(0) | None => 0,
            Some(i) => i - 1,
        };
        self.list_state.select(Some(prev));
    }

    fn enter_insert(&mut self, mode: InputMode) {
        self.mode = mode;
        self.input.reset();
    }

    fn commit_input(&mut self) {
        let input = self.input.value().trim().to_string();
        if !input.is_empty() {
            let action = match self.mode {
                InputMode::InsertAction => AppAction::CreateAction(input),
                InputMode::InsertEvent => AppAction::CreateEvent(input),
                InputMode::Normal => return,
            };
            let _ = self.tx.send(action);
        }
        self.input.reset();
        self.mode = InputMode::Normal;
    }

    fn cancel_insert(&mut self) {
        self.input.reset();
        self.mode = InputMode::Normal;
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

impl AppView for PipelineView {
    fn handle_key_event(&mut self, key: KeyEvent, tx: &UnboundedSender<AppAction>) -> bool {
        // Delegate to popup / editor panes first.
        match &mut self.active_pane {
            ActivePane::NewItemPopup(sel) => {
                match key.code {
                    KeyCode::Esc => {
                        self.active_pane = ActivePane::List;
                    }
                    KeyCode::Char('a') => {
                        self.active_pane = ActivePane::List;
                        self.enter_insert(InputMode::InsertAction);
                    }
                    KeyCode::Char('e') => {
                        self.active_pane = ActivePane::List;
                        self.enter_insert(InputMode::InsertEvent);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *sel = sel.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *sel = (*sel + 1).min(1);
                    }
                    KeyCode::Enter => {
                        let chosen = *sel;
                        self.active_pane = ActivePane::List;
                        if chosen == 0 {
                            self.enter_insert(InputMode::InsertAction);
                        } else {
                            self.enter_insert(InputMode::InsertEvent);
                        }
                    }
                    _ => {}
                }
                return true;
            }
            ActivePane::EditAction(editor) => {
                match editor.handle_key(key) {
                    ActionOutcome::Stay => {}
                    ActionOutcome::Back => {
                        self.active_pane = ActivePane::List;
                    }
                    ActionOutcome::Complete(id) => {
                        let _ = tx.send(AppAction::CompleteAction(id));
                        self.active_pane = ActivePane::List;
                    }
                    ActionOutcome::Delete(id) => {
                        let _ = tx.send(AppAction::DeleteAction(id));
                        self.active_pane = ActivePane::List;
                    }
                }
                return true;
            }
            ActivePane::EditEvent(editor) => {
                match editor.handle_key(key) {
                    EventOutcome::Stay => {}
                    EventOutcome::Back => {
                        self.active_pane = ActivePane::List;
                    }
                    EventOutcome::Delete(id) => {
                        let _ = tx.send(AppAction::DeleteEvent(id));
                        self.active_pane = ActivePane::List;
                    }
                }
                return true;
            }
            ActivePane::List => {}
        }

        if self.mode.is_insert() {
            match key.code {
                KeyCode::Enter => self.commit_input(),
                KeyCode::Esc => self.cancel_insert(),
                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.input.handle(InputRequest::DeletePrevWord);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.input.handle(InputRequest::DeleteLine);
                }
                KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.input.handle(InputRequest::GoToStart);
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.input.handle(InputRequest::GoToEnd);
                }
                KeyCode::Left => {
                    self.input.handle(InputRequest::GoToPrevChar);
                }
                KeyCode::Right => {
                    self.input.handle(InputRequest::GoToNextChar);
                }
                KeyCode::Home => {
                    self.input.handle(InputRequest::GoToStart);
                }
                KeyCode::End => {
                    self.input.handle(InputRequest::GoToEnd);
                }
                KeyCode::Backspace => {
                    self.input.handle(InputRequest::DeletePrevChar);
                }
                KeyCode::Delete => {
                    self.input.handle(InputRequest::DeleteNextChar);
                }
                KeyCode::Char(c) => {
                    self.input.handle(InputRequest::InsertChar(c));
                }
                _ => {}
            }
            return true;
        }

        match key.code {
            KeyCode::Char('n') => {
                self.active_pane = ActivePane::NewItemPopup(0);
                true
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_prev();
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next();
                true
            }
            KeyCode::Char('g') | KeyCode::Home => {
                if !self.entries.is_empty() {
                    self.list_state.select(Some(0));
                }
                true
            }
            KeyCode::Char('G') | KeyCode::End => {
                if !self.entries.is_empty() {
                    self.list_state.select(Some(self.entries.len() - 1));
                }
                true
            }
            KeyCode::Char('d') => {
                if let Some(i) = self.list_state.selected() {
                    match self.entries.get(i) {
                        Some(ListEntry::Action(a)) => {
                            let _ = tx.send(AppAction::DeleteAction(a.id));
                        }
                        Some(ListEntry::Event(e)) => {
                            let _ = tx.send(AppAction::DeleteEvent(e.id));
                        }
                        None => {}
                    }
                }
                true
            }
            KeyCode::Enter => {
                if let Some(i) = self.list_state.selected() {
                    match self.entries.get(i) {
                        Some(ListEntry::Action(a)) => {
                            self.active_pane =
                                ActivePane::EditAction(ActionEditorView::new(a.clone()));
                        }
                        Some(ListEntry::Event(e)) => {
                            self.active_pane =
                                ActivePane::EditEvent(EventEditorView::new(e.clone()));
                        }
                        None => {}
                    }
                }
                true
            }
            KeyCode::Char('c') => {
                if let Some(i) = self.list_state.selected() {
                    if let Some(ListEntry::Action(a)) = self.entries.get(i) {
                        let _ = tx.send(AppAction::CompleteAction(a.id));
                    }
                }
                true
            }
            KeyCode::Char('r') => {
                let _ = tx.send(AppAction::RefreshActions);
                let _ = tx.send(AppAction::RefreshEvents);
                true
            }
            KeyCode::Esc => {
                self.list_state.select(None);
                true
            }
            _ => false,
        }
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        // If an editor pane is active, render it instead of the list.
        match &mut self.active_pane {
            ActivePane::EditAction(editor) => {
                editor.draw(f, area, last_frame);
                return;
            }
            ActivePane::EditEvent(editor) => {
                editor.draw(f, area, last_frame);
                return;
            }
            ActivePane::NewItemPopup(_) | ActivePane::List => {}
        }

        // Layout: list | preview line | input bar
        let has_preview = self.mode.is_insert() && !self.input.value().is_empty();

        let constraints: Vec<Constraint> = if has_preview {
            vec![
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Length(3),
            ]
        } else {
            vec![Constraint::Fill(1), Constraint::Length(3)]
        };

        let chunks = Layout::new(Direction::Vertical, constraints).split(area);

        // Store width so item renderers can compute column alignment.
        self.list_width = chunks[0].width;

        if has_preview {
            draw_list(
                f,
                chunks[0],
                &self.entries,
                &mut self.list_state,
                self.list_width,
            );
            draw_preview(f, chunks[1], &self.mode, self.input.value());
            draw_input_bar(f, chunks[2], &self.mode, &self.input);
        } else {
            draw_list(
                f,
                chunks[0],
                &self.entries,
                &mut self.list_state,
                self.list_width,
            );
            draw_input_bar(f, chunks[1], &self.mode, &self.input);
        }

        // Draw the new-item popup over the list if active.
        if let ActivePane::NewItemPopup(sel) = &self.active_pane {
            draw_new_item_popup(f, area, *sel);
        }
    }
}

// ---------------------------------------------------------------------------
// Item height helpers
// ---------------------------------------------------------------------------

/// Consistent height for every item in the list.
const ITEM_HEIGHT: u16 = 7;

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// Render a single action item into `area`.
fn render_action_item(f: &mut Frame, area: Rect, action: &Action, selected: bool, list_width: u16) {
    // Colours
    // let (bg, fg, border_fg, meta_fg) = if selected {
    //     (Color::Reset, Color::Black, Color::White, Color::DarkGray)
    // } else {
    //     (Color::Reset, Color::White, Color::DarkGray, Color::DarkGray)
    // };

    let block = if selected {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green))
            .padding(Padding::horizontal(1))
    } else {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green).dim())
            .padding(Padding::horizontal(1))
            .style(Style::default().dim())
    };

    // Meta string: date/time + duration + recurrence — right-aligned.
    let when_str = match action.state {
        ActionState::Queued(t) => {
            let local = Local.from_utc_datetime(&t.time.naive_utc());
            local.format("\u{f017} %b %d %H:%M").to_string()
        }
        ActionState::Backlogged(Some(date)) => format!("\u{f073} {}", date.format("%b %d")),
        _ => String::new(),
    };
    let dur_str = match action.duration {
        Some(d) => {
            let m = d.num_minutes();
            if m >= 60 {
                format!("  \u{f254} {}h{}m", m / 60, m % 60)
            } else {
                format!("  \u{f254} {}m", m)
            }
        }
        None => String::new(),
    };
    let rec_str = if action.recurrence.is_some() {
        format!("  \u{f021}") // nf-fa-refresh
    } else {
        String::new()
    };
    let meta = format!("{}{}{}", when_str, dur_str, rec_str);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Split inner area: title row + optional detail rows.
    // Title row: left = title, right = meta (fixed 22 cols).
    let meta_width = 22u16.min(inner.width.saturating_sub(4));
    let title_width = inner.width.saturating_sub(meta_width);

    let title_row = Rect { height: 1, ..inner };
    let [title_area, meta_area] = Layout::horizontal([
        Constraint::Length(title_width),
        Constraint::Length(meta_width),
    ])
    .areas(title_row);

    // Title
    let title_style = if selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    f.render_widget(
        Paragraph::new(action.title.clone()).style(title_style),
        title_area,
    );

    // Meta (right-aligned)
    if !meta.is_empty() {
        f.render_widget(
            Paragraph::new(meta)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Right),
            meta_area,
        );
    }

    // Detail rows.
    if inner.height > 1 {
        let detail_area = Rect {
            y: inner.y + 1,
            height: inner.height - 1,
            ..inner
        };
        let mut detail_lines: Vec<Line> = Vec::new();

        if let Some(content) = &action.content {
            detail_lines.push(Line::from(Span::styled(
                content.clone(),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if action.saved {
            detail_lines.push(Line::from(Span::styled(
                "saved",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
        }

        if !detail_lines.is_empty() {
            f.render_widget(Paragraph::new(detail_lines), detail_area);
        }
    }
}

/// Render a single event item into `area`.
fn render_event_item(f: &mut Frame, area: Rect, event: &Event, selected: bool, _list_width: u16) {
    // let (bg, title_fg, border_fg) = if selected {
    //     (Color::Cyan, Color::Black, Color::Cyan)
    // } else {
    //     (Color::Reset, Color::Cyan, Color::DarkGray)
    // };

    let block = if selected {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue))
            .padding(Padding::horizontal(1))
    } else {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue).dim())
            .padding(Padding::horizontal(1))
            .style(Style::default().dim())
    };

    let local = Local.from_utc_datetime(&event.time.naive_utc());
    let when_str = local.format("\u{f017} %b %d %H:%M").to_string();
    let dur_str = match event.duration {
        Some(d) => {
            let m = d.num_minutes();
            if m >= 60 {
                format!("  \u{f254} {}h{}m", m / 60, m % 60)
            } else {
                format!("  \u{f254} {}m", m)
            }
        }
        None => String::new(),
    };
    let rec_str = if event.recurrence.is_some() {
        format!("  \u{f021}")
    } else {
        String::new()
    };
    let meta = format!("{}{}{}", when_str, dur_str, rec_str);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let meta_width = 22u16.min(inner.width.saturating_sub(4));
    let title_width = inner.width.saturating_sub(meta_width);
    let title_row = Rect { height: 1, ..inner };

    let [title_area, meta_area] = Layout::horizontal([
        Constraint::Length(title_width),
        Constraint::Length(meta_width),
    ])
    .areas(title_row);

    // Calendar icon prefix + title
    let title_line = Line::from(vec![
        Span::styled("\u{f073} ", Style::default().fg(Color::Blue)),
        Span::styled(
            event.title.clone(),
            Style::default().add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        ),
    ]);
    f.render_widget(Paragraph::new(title_line), title_area);

    // Meta right-aligned
    if !meta.is_empty() {
        f.render_widget(
            Paragraph::new(meta)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Right),
            meta_area,
        );
    }

    // Detail rows.
    if inner.height > 1 {
        let detail_area = Rect {
            y: inner.y + 1,
            height: inner.height - 1,
            ..inner
        };
        let mut detail_lines: Vec<Line> = Vec::new();
        if let Some(content) = &event.content {
            detail_lines.push(Line::from(Span::styled(
                content.clone(),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if event.saved {
            detail_lines.push(Line::from(Span::styled(
                "saved",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
        }
        if !detail_lines.is_empty() {
            f.render_widget(Paragraph::new(detail_lines), detail_area);
        }
    }
}

fn draw_list(
    f: &mut Frame,
    area: Rect,
    entries: &[ListEntry],
    list_state: &mut ListState,
    list_width: u16,
) {
    if entries.is_empty() {
        let empty = Paragraph::new("No items. Press 'a' to add an action, 'e' for an event.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    // Snapshot entries into an Arc so both the ListView closure and the
    // overdraw pass can access them without lifetime conflicts.
    let entries_overdraw: std::sync::Arc<Vec<ListEntry>> = std::sync::Arc::new(entries.to_vec());

    let builder = ListBuilder::new(move |_context| {
        let height = ITEM_HEIGHT;

        // We return a placeholder widget — actual rendering happens below via
        // a custom PreRender approach. tui-widget-list renders the widget into
        // the allocated area, so we return an empty Paragraph and do our own
        // rendering in a second pass.
        //
        // Actually, tui-widget-list calls render() on the returned widget, so
        // we return a closure-rendered Paragraph as a stand-in. The real
        // rendering needs the Frame, so we use a different strategy: return a
        // Paragraph with the correct height to drive layout, then do a manual
        // render pass below.
        (Paragraph::new(""), height)
    });

    // Because tui-widget-list renders the widget itself (we can't inject
    // Frame-based rendering into the closure), we use it purely for layout
    // and scroll tracking, then manually render each visible item using the
    // areas tui-widget-list would have used.
    //
    // Strategy: render the ListView (which draws empty Paragraphs), then
    // compute each item's Rect and overdraw with our rich rendering.
    let item_count = entries.len();
    let list = ListView::new(builder, item_count).scroll_padding(1);
    f.render_stateful_widget(list, area, list_state);

    // -----------------------------------------------------------------------
    // Manual overdraw pass: compute each item's Rect and render it properly.
    // We replicate tui-widget-list's vertical layout logic.
    // -----------------------------------------------------------------------
    let selected_idx = list_state.selected().unwrap_or(usize::MAX);

    // Figure out the first visible index (scroll offset). tui-widget-list
    // doesn't expose this directly, so we compute it ourselves using the same
    // height logic. We find the first index such that the sum of heights from
    // [first..selected] fits within the area, working backwards from selected.
    let first_visible = {
        let mut remaining = area.height as i32;
        let mut first = selected_idx.min(item_count.saturating_sub(1));
        loop {
            remaining -= ITEM_HEIGHT as i32;
            if remaining <= 0 || first == 0 {
                break;
            }
            first -= 1;
        }
        first
    };

    let mut y = area.y;
    for idx in first_visible..item_count {
        if y >= area.y + area.height {
            break;
        }
        let selected = idx == selected_idx;
        let height = ITEM_HEIGHT;
        let visible_height = height.min(area.y + area.height - y);
        let item_area = Rect {
            x: area.x,
            y,
            width: area.width,
            height: visible_height,
        };

        match &entries_overdraw[idx] {
            ListEntry::Action(a) => render_action_item(f, item_area, a, selected, list_width),
            ListEntry::Event(e) => render_event_item(f, item_area, e, selected, list_width),
        }

        y += height;
    }
}

/// One-line parse preview shown between the list and the input bar while
/// typing. Summarises the parsed properties (when, duration, recurrence,
/// location, tags, people) so the user can see what will be created.
///
/// TODO: replace this preview line with span-based inline coloring of the
/// input text once `simple_parser::ParseDraft` exposes `highlights` ranges
/// through the public API and the TUI input widget supports styled spans.
/// See `ParseDraft::highlights: Vec<(Range<usize>, HighlightKind)>`.
fn draw_preview(f: &mut Frame, area: Rect, mode: &InputMode, input: &str) {
    use simple_parser::ast::WhenSpec;

    let draft_result = match mode {
        InputMode::InsertAction => simple_parser::parse_action_input(input).ok(),
        InputMode::InsertEvent => simple_parser::parse_event_input(input).ok(),
        InputMode::Normal => return,
    };

    let preview = draft_result
        .map(|draft| {
            let mut parts: Vec<String> = Vec::new();

            if let Some(when) = &draft.when {
                match when {
                    WhenSpec::DateTime(dt) => {
                        let local = dt.with_timezone(&Local);
                        // Format like "Mon Jan 6 2pm" or "Mon Jan 6 2:30pm"
                        let time_fmt = if local.format("%M").to_string() == "00" {
                            local.format("%-I%P").to_string()
                        } else {
                            local.format("%-I:%M%P").to_string()
                        };
                        parts.push(format!(
                            "\u{f017} {}", // nf-fa-clock_o
                            local.format(&format!("%a %b %-d {}", time_fmt)).to_string()
                        ));
                    }
                    WhenSpec::NaiveDate(date) => {
                        parts.push(format!("\u{f073} {}", date.format("%a %b %-d"))); // nf-fa-calendar
                    }
                }
            }

            if let Some(dur) = draft.duration {
                let mins = dur.num_minutes();
                if mins % 60 == 0 {
                    parts.push(format!("\u{f254} {}h", mins / 60)); // nf-fa-hourglass_o
                } else if mins >= 60 {
                    parts.push(format!("\u{f254} {}h {}m", mins / 60, mins % 60));
                } else {
                    parts.push(format!("\u{f254} {}m", mins));
                }
            }

            if let Some(rec) = &draft.recurrence {
                use simple_parser::RecurrenceSpec;
                let s = match rec {
                    RecurrenceSpec::EveryDays(1) => "daily".into(),
                    RecurrenceSpec::EveryDays(n) => format!("every {n} days"),
                    RecurrenceSpec::EveryWeeks(1) => "weekly".into(),
                    RecurrenceSpec::EveryWeeks(n) => format!("every {n} weeks"),
                    RecurrenceSpec::EveryMonths(1) => "monthly".into(),
                    RecurrenceSpec::EveryMonths(n) => format!("every {n} months"),
                    RecurrenceSpec::EveryYears(1) => "yearly".into(),
                    RecurrenceSpec::EveryYears(n) => format!("every {n} years"),
                    RecurrenceSpec::OnMonthDay(d) => format!("the {d}th"),
                    RecurrenceSpec::OnWeekdays(_) => "weekly (weekdays)".into(),
                };
                parts.push(format!("\u{f021} {s}")); // nf-fa-refresh
            }

            if let Some(loc) = &draft.location {
                parts.push(format!("\u{f3c5} {loc}")); // nf-fa-map_marker
            }
            if !draft.tags.is_empty() {
                parts.push(format!("# {}", draft.tags.join(", ")));
            }
            if !draft.people.is_empty() {
                parts.push(format!("\u{f007} {}", draft.people.join(", "))); // nf-fa-user
            }

            parts.join("  ·  ")
        })
        .unwrap_or_default();

    let color = match mode {
        InputMode::InsertAction => Color::Green,
        InputMode::InsertEvent => Color::Cyan,
        InputMode::Normal => Color::DarkGray,
    };

    let p = Paragraph::new(format!("  {preview}")).style(Style::default().fg(color));
    f.render_widget(p, area);
}

fn highlight_color(kind: &HighlightKind) -> Color {
    match kind {
        HighlightKind::Title => Color::White,
        HighlightKind::When => Color::Blue,
        HighlightKind::Duration => Color::Yellow,
        HighlightKind::Recurrence => Color::Magenta,
        HighlightKind::Tag => Color::Green,
        HighlightKind::Location => Color::Cyan,
        HighlightKind::People => Color::LightGreen,
        HighlightKind::Priority => Color::Red,
        HighlightKind::Sigil => Color::DarkGray,
    }
}

/// Build a colored `Line` from a (possibly scrolled) input string slice using
/// highlight spans from the parser. `scroll_offset` is the byte offset into
/// the original input that `input` starts at, used to map highlight ranges
/// onto the visible slice correctly.
///
/// Gaps between recognized spans render in `DarkGray`.
/// The real terminal cursor is positioned by `draw_input_bar` via
/// `frame.set_cursor_position()` — no block-cursor character is appended here.
fn highlighted_input_line(input: &str, mode: &InputMode, scroll_offset: usize) -> Line<'static> {
    use simple_parser::{parse_action_input, parse_event_input};

    // Parse the *full* value (not just the visible slice) to get correct spans.
    // We reconstruct the full value by prepending the scrolled-off prefix.
    // Since we only have the visible slice here, we parse it directly and
    // adjust ranges by subtracting scroll_offset.
    let highlights: Vec<(Range<usize>, HighlightKind)> = match mode {
        InputMode::InsertAction => parse_action_input(input)
            .map(|d| d.highlights)
            .unwrap_or_default(),
        InputMode::InsertEvent => parse_event_input(input)
            .map(|d| d.highlights)
            .unwrap_or_default(),
        InputMode::Normal => vec![],
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let len = input.len();
    let mut pos = 0usize;

    // Sort spans by start position so we can walk left-to-right.
    let mut sorted = highlights;
    sorted.sort_by_key(|(r, _)| r.start);

    for (range, kind) in &sorted {
        let start = range.start.min(len);
        let end = range.end.min(len);

        // Gap before this span — unrecognized text.
        if pos < start {
            let gap = input[pos..start].to_string();
            if !gap.is_empty() {
                spans.push(Span::styled(gap, Style::default().fg(Color::DarkGray)));
            }
        }

        // The recognized span.
        if start < end {
            let text = input[start..end].to_string();
            spans.push(Span::styled(
                text,
                Style::default().fg(highlight_color(kind)),
            ));
        }

        pos = pos.max(end);
    }

    // Trailing unrecognized text.
    if pos < len {
        let tail = input[pos..].to_string();
        if !tail.is_empty() {
            spans.push(Span::styled(tail, Style::default().fg(Color::DarkGray)));
        }
    }

    Line::from(spans)
}

fn draw_input_bar(f: &mut Frame, area: Rect, mode: &InputMode, input: &Input) {
    let border_color = mode.border_color();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    match mode {
        InputMode::InsertAction | InputMode::InsertEvent => {
            // Label prefix + syntax-colored input tokens on one line,
            // with horizontal scrolling when the input exceeds the available width.
            let label_text = mode.label();
            let label_width = label_text.len() as u16;
            // Available width for the input portion (minus label, minus 1 for cursor).
            let input_width = inner.width.saturating_sub(label_width).saturating_sub(1) as usize;
            let scroll = input.visual_scroll(input_width);

            let label = Span::styled(label_text.to_string(), Style::default().fg(border_color));
            let mut spans = vec![label];
            // Build colored spans from the scrolled portion of the input value.
            let visible_value = &input.value()[scroll.min(input.value().len())..];
            let input_line = highlighted_input_line(visible_value, mode, scroll);
            spans.extend(input_line.spans);
            f.render_widget(Paragraph::new(Line::from(spans)), inner);

            // Position the real terminal cursor.
            let cursor_x =
                inner.x + label_width + (input.visual_cursor().max(scroll) - scroll) as u16;
            let cursor_y = inner.y;
            f.set_cursor_position((cursor_x, cursor_y));
        }
        InputMode::Normal => {
            let help = Paragraph::new(
                " [n] new  [j/k] move  [Enter] open  [c] complete  [d] delete  [r] refresh  [q] quit"
            )
            .style(Style::default().fg(Color::DarkGray));
            f.render_widget(help, inner);
        }
    }
}

// ---------------------------------------------------------------------------
// New-item popup
// ---------------------------------------------------------------------------

fn draw_new_item_popup(f: &mut Frame, area: Rect, selected: usize) {
    // Each option is 3 lines tall, plus border (2) + blank (1) + help row (1) = 10 tall.
    let popup_width = 26u16.min(area.width);
    let popup_height = 10u16.min(area.height);
    let popup_area = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height) / 2,
        width: popup_width,
        height: popup_height,
    };

    // Clear the background so the popup is legible over the list.
    f.render_widget(ratatui::widgets::Clear, popup_area);

    let block = Block::default()
        .title(" New ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.height < 5 {
        return;
    }

    // Each option occupies 3 rows (blank / label / blank) with horizontal padding.
    let items = [
        ("\u{f144}  Action", Color::Green),
        ("\u{f073}  Event", Color::Blue),
    ];

    let h_pad = 2u16;
    for (i, (label, color)) in items.iter().enumerate() {
        let block_y = inner.y + i as u16 * 3;
        let block_area = Rect {
            y: block_y,
            height: 3,
            x: inner.x,
            width: inner.width,
        };
        let is_sel = i == selected;
        let style = if is_sel {
            Style::default()
                .fg(*color)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(*color)
        };
        // Fill all 3 rows with the style so the highlight spans the full height.
        f.render_widget(Paragraph::new("").style(style), block_area);
        // Label on the middle row, indented.
        let label_row = Rect {
            y: block_y + 1,
            height: 1,
            x: inner.x + h_pad,
            width: inner.width.saturating_sub(h_pad),
        };
        f.render_widget(Paragraph::new(*label).style(style), label_row);
    }

    // Help text pinned to the bottom of the inner area, with a blank line above.
    let help_row = Rect {
        y: inner.y + inner.height.saturating_sub(1),
        height: 1,
        ..inner
    };
    f.render_widget(
        Paragraph::new(" [a] [e]  [esc] cancel").style(Style::default().fg(Color::DarkGray)),
        help_row,
    );
}
