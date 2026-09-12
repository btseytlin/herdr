use super::*;

fn vertical_state(agents: bool, count: usize) -> ClientShellState {
    let config: Config = toml::from_str(&format!(
        "[ui]\nvertical_tabs = true\n[ui.sidebar.agents]\nenabled = {agents}\n"
    ))
    .unwrap();
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    let mut projected = snapshot();
    let template = projected.tabs[0].clone();
    projected.tabs = (0..count)
        .map(|index| ClientShellTab {
            tab_id: format!("tab_{}", index + 1),
            label: format!("task {}", index + 1),
            focused: index == 0,
            ..template.clone()
        })
        .collect();
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state
}

#[test]
fn vertical_tabs_wrap_titles_at_separators_and_keep_continuations_clickable() {
    for (label, first_line) in [
        ("alpha·bravo·charlie", "alpha·bravo·"),
        ("alpha-bravo-charlie", "alpha-bravo-"),
        ("alpha bravo charlie", "alpha bravo"),
    ] {
        let mut state = vertical_state(false, 2);
        state.sidebar_width = 17;
        let mut projected = state.snapshot.as_deref().unwrap().clone();
        projected.tabs[1].label = label.into();
        state.set_snapshot(Box::new(projected));
        let frame = state.compose(106, 40).unwrap();
        let buffer = frame.to_ratatui_buffer().unwrap();
        let rect = state.hits.tabs[1].0;
        assert_eq!(rect.height, 2, "{label}");
        for (row, expected) in [(rect.y, first_line), (rect.y + 1, "charlie")] {
            let text = (rect.x + 3..rect.right())
                .map(|x| buffer.cell((x, row)).unwrap().symbol())
                .collect::<String>();
            assert_eq!(text.trim_end(), expected);
        }
        assert_eq!(state.snapshot.as_ref().unwrap().tabs[1].label, label);
        mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            rect.x + 3,
            rect.y + 1,
        );
        let click = mouse(
            &mut state,
            MouseEventKind::Up(MouseButton::Left),
            rect.x + 3,
            rect.y + 1,
        );
        assert!(
            matches!(&click.actions[0], ClientShellAction::Endpoint { request, .. }
            if matches!(&request.method, crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_2"))
        );
    }
}

#[test]
fn vertical_tabs_wrapped_rows_scroll_reveal_and_place_drag_indicators() {
    let mut state = vertical_state(false, 8);
    state.sidebar_width = 17;
    let mut projected = state.snapshot.as_deref().unwrap().clone();
    for tab in &mut projected.tabs {
        tab.label = "alpha bravo charlie delta echo".into();
    }
    state.set_snapshot(Box::new(projected));
    state.compose(106, 40).unwrap();
    assert!(!state.hits.tab_scrollbar.is_empty());
    assert_eq!(state.hits.tabs[0].0.height, 3);
    assert_eq!(state.hits.tabs[1].0.y, state.hits.tabs[0].0.bottom());
    let first = state.hits.tabs[0].0;
    let second = state.hits.tabs[1].0;
    mouse(
        &mut state,
        MouseEventKind::Down(MouseButton::Left),
        first.x,
        first.y,
    );
    mouse(
        &mut state,
        MouseEventKind::Drag(MouseButton::Left),
        second.x,
        second.y + 1,
    );
    let frame = state.compose(106, 40).unwrap();
    let buffer = frame.to_ratatui_buffer().unwrap();
    assert_eq!(buffer.cell((second.x, second.y)).unwrap().symbol(), "▸");
    let release = mouse(
        &mut state,
        MouseEventKind::Up(MouseButton::Left),
        second.x,
        second.y + 1,
    );
    assert!(
        matches!(&release.actions[0], ClientShellAction::Endpoint { request, .. }
        if matches!(&request.method, crate::api::schema::Method::TabMove(params) if params.tab_id == "tab_1" && params.insert_index == 1))
    );

    let outcome = mouse(&mut state, MouseEventKind::ScrollDown, first.x, first.y);
    assert!(outcome.actions.is_empty());
    state.compose(106, 40).unwrap();
    assert_eq!(state.hits.tabs[0].1, "tab_2");
    let mut focused = state.snapshot.as_deref().unwrap().clone();
    focused.focused_tab_id = Some("tab_8".into());
    for tab in &mut focused.tabs {
        tab.focused = tab.tab_id == "tab_8";
    }
    state.set_snapshot(Box::new(focused));
    state.compose(106, 40).unwrap();
    assert_eq!(state.hits.tabs.last().unwrap().1, "tab_8");
    assert!(state.hits.tabs.last().unwrap().0.bottom() <= state.hits.tab_body.bottom());
    state.compose(106, 16).unwrap();
    assert_eq!(state.hits.tabs.last().unwrap().1, "tab_8");
    assert!(state.hits.tabs.last().unwrap().0.bottom() <= state.hits.tab_body.bottom());
}

#[test]
fn vertical_tabs_oversized_title_stays_inside_its_body() {
    let mut state = vertical_state(false, 1);
    state.sidebar_width = 17;
    state.compose(106, 20).unwrap();
    let mut projected = state.snapshot.as_deref().unwrap().clone();
    projected.tabs[0].label =
        "alpha bravo charlie ".repeat(usize::from(state.hits.tab_body.height));
    state.set_snapshot(Box::new(projected));
    state.compose(106, 20).unwrap();
    assert_eq!(state.hits.tabs[0].0.height, state.hits.tab_body.height);
    assert_eq!(state.hits.tabs[0].0.bottom(), state.hits.new_tab.y);
    assert_eq!(state.hits.tab_max_scroll, 0);
    assert!(state.hits.tab_scrollbar.is_empty());
}

#[test]
fn vertical_tabs_render_below_spaces_with_independent_scrolling() {
    for agents in [false, true] {
        let mut state = vertical_state(agents, 20);
        state.compose(106, 40).unwrap();
        let first = state.hits.tabs[0].0;
        assert!(first.x < state.sidebar_width);
        assert!(first.y > state.hits.workspace_body.bottom());
        assert_eq!(state.hits.tabs[1].0.y, first.y + 1);
        if agents {
            assert!(state.hits.tabs.last().unwrap().0.bottom() <= state.hits.agent_body.y);
        }
        let workspace_scroll = state.workspace_scroll;
        let outcome = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: first.x,
            row: first.y,
            modifiers: KeyModifiers::NONE,
        })]);
        assert!(outcome.actions.is_empty());
        assert!(outcome.requests.is_empty());
        assert_eq!(state.tab_scroll, 1);
        assert_eq!(state.workspace_scroll, workspace_scroll);
        state.compose(106, 40).unwrap();
        assert_eq!(state.hits.tabs[0].1, "tab_2");
    }
}

#[test]
fn vertical_tabs_settings_save_locally_and_keep_other_settings() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tmp")
        .join(format!("vertical-tabs-settings-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    std::fs::write(&path, "[ui]\ncopy_on_select = true\n").unwrap();
    let mut state = vertical_state(false, 3);
    state.config.local_config_path = path.clone();
    state.open_settings_overlay();
    let section = ClientSettingsSection::ALL
        .iter()
        .copied()
        .find(|section| section.label() == "vertical tabs")
        .expect("vertical tabs setting");
    state.select_settings_section(section, &mut ClientShellInput::default());
    for (index, enabled) in [(1, false), (0, true)] {
        state.select_settings_choice(index);
        let mut outcome = ClientShellInput::default();
        state.apply_settings_choice(&mut outcome);
        assert!(outcome.actions.is_empty());
        assert!(outcome.requests.is_empty());
        assert!(state.endpoint_error.is_none());
        let saved: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["ui"]["vertical_tabs"].as_bool(), Some(enabled));
        assert_eq!(saved["ui"]["copy_on_select"].as_bool(), Some(true));
    }
    state.config.local_config_path = directory.clone();
    state.select_settings_choice(1);
    let mut failed = ClientShellInput::default();
    state.apply_settings_choice(&mut failed);
    assert!(state.endpoint_error.is_some());
    assert!(state.config.vertical_tabs);
    state.compose(106, 30).unwrap();
    assert_eq!(
        state.hits.settings_tabs.len(),
        ClientSettingsSection::ALL.len()
    );
    assert!(state
        .hits
        .settings_tabs
        .iter()
        .all(|(rect, _)| !rect.is_empty()));
    std::fs::remove_dir_all(directory).unwrap();
}

fn mouse(state: &mut ClientShellState, kind: MouseEventKind, x: u16, y: u16) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })])
}

#[test]
fn vertical_tabs_keep_compact_mobile_and_short_layouts_usable() {
    for agents in [false, true] {
        for rows in [2, 6, 12, 40] {
            let mut state = vertical_state(agents, 20);
            state.compose(106, rows).unwrap();
            for (rect, _) in &state.hits.tabs {
                assert!(rect.bottom() <= rows);
            }
            if !state.hits.vertical_tabs_area.is_empty() {
                assert!(state.hits.tab_body.height >= 1);
                assert!(state.hits.workspace_body.height >= 1);
                assert!(state
                    .hits
                    .tabs
                    .iter()
                    .all(|(rect, _)| rect.right() <= state.sidebar_width));
            } else {
                assert!(state
                    .hits
                    .tabs
                    .iter()
                    .all(|(rect, _)| rect.x >= state.sidebar_width));
            }
            for mode in [
                SidebarCollapsedModeConfig::Compact,
                SidebarCollapsedModeConfig::Hidden,
            ] {
                state.sidebar_collapsed = true;
                state.config.sidebar_collapsed_mode = mode;
                state.compose(106, rows).unwrap();
                assert!(state.hits.vertical_tabs_area.is_empty());
                assert!(!state.hits.tabs.is_empty());
            }
        }
    }
    let mut state = vertical_state(false, 1);
    state.config.hide_tab_bar_when_single_tab = true;
    state.compose(106, 40).unwrap();
    assert!(state.hits.tabs.is_empty());
    state.compose(30, 20).unwrap();
    assert!(state.hits.tabs.is_empty());
    assert!(!state.hits.mobile_switch.is_empty());
    state.config.hide_tab_bar_when_single_tab = false;
    state.config.vertical_tabs = false;
    state.compose(106, 40).unwrap();
    assert_eq!(state.hits.tabs[0].0.y, state.layout(106, 40).tab_bar.y);
}

#[test]
fn vertical_tabs_keep_status_colors_zoom_and_saved_labels() {
    for indicators in [
        crate::config::StatusIndicatorStyle::Dots,
        crate::config::StatusIndicatorStyle::Symbols,
    ] {
        for status in [
            AgentStatus::Working,
            AgentStatus::Blocked,
            AgentStatus::Done,
            AgentStatus::Idle,
            AgentStatus::Unknown,
        ] {
            let mut state = vertical_state(false, 1);
            state.config.status_indicators = indicators;
            let mut projected = state.snapshot.as_deref().unwrap().clone();
            projected.tabs[0].agent_status = status;
            projected.tabs[0].zoomed = true;
            state.set_snapshot(Box::new(projected));
            state.set_pane_surface(surface());
            let frame = state.compose(106, 40).unwrap().to_ratatui_buffer().unwrap();
            let rect = state.hits.tabs[0].0;
            let marker = frame.cell((rect.x + 1, rect.y)).unwrap();
            assert_eq!(marker.symbol(), status_icon(status, indicators));
            assert_eq!(marker.fg, status_color(status, &state.config.palette));
            assert_eq!(marker.bg, state.config.palette.surface0);
            assert_eq!(
                frame.cell((rect.x + 3, rect.y)).unwrap().bg,
                state.config.palette.accent
            );
            let text = (rect.x..rect.right())
                .map(|x| frame.cell((x, rect.y)).unwrap().symbol())
                .collect::<String>();
            assert!(text.contains("task 1 Z"));
            assert_eq!(state.snapshot.as_ref().unwrap().tabs[0].label, "task 1");
        }
    }
}

#[test]
fn vertical_tabs_stay_clickable_while_bottom_mode_bar_is_visible() {
    let mut state = vertical_state(false, 3);
    state.config.tab_bar_position = TabBarPositionConfig::Bottom;
    state.compose(106, 40).unwrap();
    let tabs = state.hits.tabs.clone();
    state.handle_input_bytes(&[0x02]);
    state.compose(106, 40).unwrap();
    assert_eq!(state.hits.tabs, tabs);
    assert!(!state.hits.new_tab.is_empty());
}

#[test]
fn vertical_tabs_focus_reveal_clamps_after_height_changes() {
    let mut state = vertical_state(false, 20);
    state.compose(106, 40).unwrap();
    let mut projected = state.snapshot.as_deref().unwrap().clone();
    for tab in &mut projected.tabs {
        tab.focused = tab.tab_id == "tab_20";
    }
    projected.focused_tab_id = Some("tab_20".into());
    projected.workspaces[0].active_tab_id = "tab_20".into();
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    state.compose(106, 20).unwrap();
    assert!(state.hits.tabs.iter().any(|(_, id)| id == "tab_20"));
    assert_eq!(state.tab_scroll, state.hits.tab_max_scroll);
    state.compose(106, 60).unwrap();
    assert_eq!(state.tab_scroll, 0);
    assert_eq!(state.hits.tabs.len(), 20);
}

#[test]
fn vertical_tabs_click_menu_and_drag_keep_stable_targets() {
    let mut state = vertical_state(false, 3);
    state.compose(106, 40).unwrap();
    let first = state.hits.tabs[0].0;
    let third = state.hits.tabs[2].0;
    mouse(
        &mut state,
        MouseEventKind::Down(MouseButton::Left),
        third.x + 1,
        third.y,
    );
    let click = mouse(
        &mut state,
        MouseEventKind::Up(MouseButton::Left),
        third.x + 1,
        third.y,
    );
    assert!(
        matches!(&click.actions[0], ClientShellAction::Endpoint { request, .. }
        if matches!(&request.method, crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_3"))
    );
    for valid in [true, false] {
        mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            first.x + 1,
            first.y,
        );
        mouse(
            &mut state,
            MouseEventKind::Drag(MouseButton::Left),
            third.x + 1,
            third.y + 1,
        );
        let x = if valid { third.x + 1 } else { 100 };
        mouse(
            &mut state,
            MouseEventKind::Drag(MouseButton::Left),
            x,
            third.y + 1,
        );
        let release = mouse(
            &mut state,
            MouseEventKind::Up(MouseButton::Left),
            x,
            third.y + 1,
        );
        if valid {
            assert!(
                matches!(&release.actions[0], ClientShellAction::Endpoint { request, .. }
                if matches!(&request.method, crate::api::schema::Method::TabMove(params) if params.tab_id == "tab_1" && params.insert_index == 3))
            );
        } else {
            assert!(release.actions.is_empty());
        }
    }
    mouse(
        &mut state,
        MouseEventKind::Down(MouseButton::Right),
        third.x + 1,
        third.y,
    );
    assert!(
        matches!(&state.overlay, Some(ClientShellOverlay::ContextMenu(ClientContextMenuOverlay {
        target: ClientContextMenuTarget::Tab { tab_id, .. }, ..
    })) if tab_id == "tab_3")
    );
    state.overlay = None;
    state.compose(106, 40).unwrap();
    let new_tab = state.hits.new_tab;
    mouse(
        &mut state,
        MouseEventKind::Down(MouseButton::Left),
        new_tab.x + 1,
        new_tab.y,
    );
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            target: ClientRenameTarget::NewTab { .. },
            ..
        }))
    ));
}

#[test]
fn vertical_tabs_divider_and_scrollbar_drag_are_client_local() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tmp")
        .join(format!("vertical-tabs-divider-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for agents in [false, true] {
        let mut state = vertical_state(agents, 40);
        state.config.preferences_path = Some(directory.join(format!("{agents}.json")));
        state.compose(106, 40).unwrap();
        let previous_body = state.hits.workspace_body;
        let divider = state.hits.tab_section_divider;
        mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            divider.x + 1,
            divider.y,
        );
        mouse(
            &mut state,
            MouseEventKind::Drag(MouseButton::Left),
            divider.x + 1,
            divider.y - 3,
        );
        let release = mouse(
            &mut state,
            MouseEventKind::Up(MouseButton::Left),
            divider.x + 1,
            divider.y - 3,
        );
        assert!(release.actions.is_empty());
        let saved = preferences::load(state.config.preferences_path.as_deref().unwrap()).unwrap();
        assert_eq!(saved.tab_section_split, state.tab_section_split);
        state.compose(106, 40).unwrap();
        assert!(state.hits.workspace_body.height < previous_body.height);
        let track = state.hits.tab_scrollbar;
        mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            track.x,
            track.y,
        );
        mouse(
            &mut state,
            MouseEventKind::Drag(MouseButton::Left),
            track.x,
            track.bottom() - 1,
        );
        mouse(
            &mut state,
            MouseEventKind::Up(MouseButton::Left),
            track.x,
            track.bottom() - 1,
        );
        assert_eq!(state.tab_scroll, state.hits.tab_max_scroll);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn vertical_tabs_multi_endpoint_sidebar_shows_only_active_space_tabs() {
    let mut state = vertical_state(false, 3);
    let profile = SavedSshEndpoint {
        id: crate::client::endpoint::ProfileId::parse("0123456789abcdef0123456789abcdef").unwrap(),
        label: "Remote".into(),
        target: "dev@build.example".into(),
        session: "agents".into(),
        enabled: true,
    };
    let endpoint_id = ClientEndpointId::Ssh(profile.id.clone());
    state.set_endpoint_catalog(&[profile]);
    state.set_endpoint_status(&endpoint_id, ClientEndpointStatus::Online);
    let mut remote = snapshot();
    remote.boot_id = "remote".into();
    remote.tabs[0].tab_id = "remote_tab".into();
    state.set_endpoint_snapshot(&endpoint_id, Box::new(remote));
    state.compose(106, 40).unwrap();
    assert!(!state.hits.vertical_tabs_area.is_empty());
    assert_eq!(
        state
            .hits
            .tabs
            .iter()
            .map(|(_, id)| id.as_str())
            .collect::<Vec<_>>(),
        vec!["tab_1", "tab_2", "tab_3"]
    );
    assert!(state.hits.tabs[0].0.y > state.hits.workspace_body.bottom());
}
