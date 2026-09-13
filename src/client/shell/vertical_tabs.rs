use super::*;
use ratatui::text::Span;
use render::{display_width, put_text, ShellRenderState};

const HEADER_ROWS: u16 = 1;
const CONTROL_ROWS: u16 = 1;
const FOOTER_ROWS: u16 = 1;
const MIN_TAB_HEIGHT: u16 = HEADER_ROWS + CONTROL_ROWS + 1 + FOOTER_ROWS;
const TITLE_COLUMN: u16 = 3; // Padding, status icon, then space.

fn wrapped_title_lines(title: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut used = 0usize;
    let mut separator = None;
    let span = Span::raw(title);
    for grapheme in span.styled_graphemes(Style::default()) {
        let text = grapheme.symbol;
        let whitespace = text.chars().all(char::is_whitespace);
        let grapheme_width = usize::from(display_width(text));
        while !line.is_empty() && used + grapheme_width > usize::from(width) {
            let end = if whitespace {
                line.len()
            } else {
                separator.unwrap_or(line.len())
            };
            let remaining = line[end..].trim_start().to_owned();
            lines.push(line[..end].trim_end().to_owned());
            line = remaining;
            used = usize::from(display_width(&line));
            separator = None;
        }
        if whitespace && line.is_empty() {
            continue;
        }
        // Keep a grapheme intact even when it is wider than the viewport.
        line.push_str(text);
        used += grapheme_width;
        if text == "·" || text == "-" || whitespace {
            separator = Some(line.len());
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line.trim_end().to_owned());
    }
    lines
}

pub(super) fn reserve_section(
    area: Rect,
    snapshot: Option<&ClientShellSnapshot>,
    config: &ClientShellConfig,
    state: &ShellRenderState<'_>,
    hits: &mut ShellHitMap,
) -> Rect {
    let minimum_spaces = WORKSPACE_HEADER_ROWS + 1 + 1;
    if !config.vertical_tabs
        || state.sidebar_collapsed
        || area.height < minimum_spaces + MIN_TAB_HEIGHT
    {
        return area;
    }
    let Some(snapshot) = snapshot else {
        return area;
    };
    let count = snapshot
        .tabs
        .iter()
        .filter(|tab| Some(tab.workspace_id.as_str()) == snapshot.focused_workspace_id.as_deref())
        .count();
    if config.hide_tab_bar_when_single_tab && count == 1 {
        return area;
    }
    let ratio = state
        .tab_section_split
        .unwrap_or(state.sidebar_section_split);
    let height = ((area.height as f32 * ratio).round() as u16)
        .clamp(minimum_spaces, area.height - MIN_TAB_HEIGHT);
    hits.tab_section_area = area;
    hits.vertical_tabs_area = Rect::new(area.x, area.y + height, area.width, area.height - height);
    hits.tab_section_divider = Rect::new(area.x, area.y + height, area.width, HEADER_ROWS);
    Rect { height, ..area }
}

pub(super) fn render(
    buffer: &mut Buffer,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    state: &mut ShellRenderState<'_>,
    hits: &mut ShellHitMap,
) {
    let area = hits.vertical_tabs_area;
    let palette = &config.palette;
    put_text(
        buffer,
        area.x,
        area.y,
        area.width,
        " tabs",
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );
    let tabs = snapshot
        .tabs
        .iter()
        .filter(|tab| Some(tab.workspace_id.as_str()) == snapshot.focused_workspace_id.as_deref())
        .collect::<Vec<_>>();
    let body = Rect::new(
        area.x,
        area.y + HEADER_ROWS + CONTROL_ROWS,
        area.width,
        area.height
            .saturating_sub(HEADER_ROWS + CONTROL_ROWS + FOOTER_ROWS),
    );
    hits.tab_body = body;
    let titles = tabs
        .iter()
        .map(|tab| render::tab_title(tab))
        .collect::<Vec<_>>();
    let wrap_titles = |width: u16| {
        titles
            .iter()
            .map(|title| wrapped_title_lines(title, width.saturating_sub(TITLE_COLUMN)))
            .collect::<Vec<_>>()
    };
    let mut lines = wrap_titles(body.width);
    let scrollbar = body.width > 1
        && lines
            .iter()
            .map(|lines| lines.len().min(usize::from(body.height)))
            .sum::<usize>()
            > usize::from(body.height);
    let width = body.width.saturating_sub(u16::from(scrollbar));
    if scrollbar {
        lines = wrap_titles(width);
    }
    let heights = lines
        .iter()
        .map(|lines| lines.len().min(usize::from(body.height)) as u16)
        .collect::<Vec<_>>();
    let gaps = vec![0; tabs.len()];
    if std::mem::take(state.reveal_focused_tab) {
        if let Some(target) = tabs.iter().position(|tab| tab.focused) {
            *state.tab_scroll = scroll::list_scroll_start_to_reveal(
                &heights,
                &gaps,
                body.height,
                *state.tab_scroll,
                target,
            );
        }
    }
    let metrics = scroll::list_scroll_metrics(&heights, &gaps, body.height, *state.tab_scroll);
    *state.tab_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);
    hits.tab_max_scroll = metrics.max_offset_from_bottom;
    hits.tab_scroll_metrics = Some(metrics);
    let mut y = body.y;
    for (index, tab) in tabs
        .iter()
        .enumerate()
        .skip(*state.tab_scroll)
        .take(metrics.viewport_rows)
    {
        let rect = Rect::new(body.x, y, width, heights[index]);
        y = rect.bottom();
        let style = render::tab_style(tab, palette);
        buffer.set_style(rect, style);
        for (row, line) in lines[index]
            .iter()
            .take(usize::from(rect.height))
            .enumerate()
        {
            put_text(
                buffer,
                rect.x + TITLE_COLUMN,
                rect.y + row as u16,
                rect.width.saturating_sub(TITLE_COLUMN),
                line,
                style,
            );
        }
        let marker_x = rect.x + u16::from(rect.width > 1);
        if rect.width > 0 {
            let icon = status_icon(tab.agent_status, config.status_indicators);
            put_text(
                buffer,
                marker_x,
                rect.y,
                display_width(icon).min(rect.right() - marker_x),
                icon,
                style
                    .fg(status_color(tab.agent_status, palette))
                    .bg(palette.surface0)
                    .remove_modifier(Modifier::DIM),
            );
        }
        hits.tabs.push((rect, tab.tab_id.clone()));
    }
    if scrollbar {
        hits.tab_scrollbar = Rect::new(body.right() - 1, body.y, 1, body.height);
        scroll::render_list_scrollbar(buffer, hits.tab_scrollbar, metrics, palette);
    }
    if config.mouse_capture {
        hits.new_tab = Rect::new(area.x, area.y + HEADER_ROWS, area.width, CONTROL_ROWS);
        put_text(
            buffer,
            hits.new_tab.x,
            hits.new_tab.y,
            hits.new_tab.width,
            " + new tab",
            Style::default().fg(palette.overlay0),
        );
    }
    if let Some(index) = state.tab_drag_insert_index {
        if index >= *state.tab_scroll && index <= state.tab_scroll.saturating_add(hits.tabs.len()) {
            let row = hits
                .tabs
                .get(index - *state.tab_scroll)
                .map(|(rect, _)| rect.y)
                .or_else(|| hits.tabs.last().map(|(rect, _)| rect.bottom()));
            if let Some(row) = row.filter(|row| *row < area.bottom()) {
                put_text(
                    buffer,
                    body.x,
                    row,
                    body.width.min(1),
                    "▸",
                    Style::default().fg(palette.accent),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_wrapping_preserves_words_and_graphemes() {
        for (title, width, expected) in [
            ("alpha beta gamma", 10, vec!["alpha beta", "gamma"]),
            (
                "alpha·bravo-charlie delta",
                13,
                vec!["alpha·bravo-", "charlie delta"],
            ),
            ("abcdefghijk", 4, vec!["abcd", "efgh", "ijk"]),
            ("界界·a\u{301}-bbb", 5, vec!["界界·", "a\u{301}-bbb"]),
            ("", 4, vec![""]),
            ("title", 0, vec![""]),
            ("界a", 1, vec!["界", "a"]),
        ] {
            assert_eq!(wrapped_title_lines(title, width), expected, "{title}");
        }
    }
}
