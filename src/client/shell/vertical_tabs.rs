use super::*;
use render::{display_width, put_text, ShellRenderState};

const HEADER_ROWS: u16 = 1;
const FOOTER_ROWS: u16 = 1;
const MIN_TAB_HEIGHT: u16 = HEADER_ROWS + 1 + FOOTER_ROWS;

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
        area.y + HEADER_ROWS,
        area.width,
        area.height.saturating_sub(HEADER_ROWS + FOOTER_ROWS),
    );
    hits.tab_body = body;
    let heights = vec![1; tabs.len()];
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
    let scrollbar = metrics.max_offset_from_bottom > 0 && body.width > 1;
    let width = body.width.saturating_sub(u16::from(scrollbar));
    for (row, tab) in tabs
        .iter()
        .skip(*state.tab_scroll)
        .take(body.height as usize)
        .enumerate()
    {
        let rect = Rect::new(body.x, body.y + row as u16, width, 1);
        let style = render::tab_style(tab, palette);
        buffer.set_style(rect, style);
        let text = format!(" {}", render::tab_label(tab, config.status_indicators));
        put_text(buffer, rect.x, rect.y, rect.width, &text, style);
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
        // Leave the sidebar's existing collapse control at the right edge.
        hits.new_tab = Rect::new(
            area.x,
            area.bottom() - FOOTER_ROWS,
            area.width.saturating_sub(2),
            FOOTER_ROWS,
        );
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
            let row = body
                .y
                .saturating_add(index.saturating_sub(*state.tab_scroll) as u16);
            if row < area.bottom() {
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
