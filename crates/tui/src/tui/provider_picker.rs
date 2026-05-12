use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::config::Config;
use crate::palette;
use crate::tui::views::{ModalKind, ModalView, ViewAction, ViewEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    List,
    KeyEntry,
    ConfirmDelete,
    AddProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddField {
    Name,
    BaseUrl,
    Protocol,
    ApiKey,
}

struct ProviderRow {
    name: String,
    display_name: String,
    has_key: bool,
    is_builtin: bool,
}

const PROTOCOL_OPTIONS: &[&str] = &["openai_compatible", "anthropic_messages", "google_gemini"];

pub struct ProviderPickerView {
    providers: Vec<ProviderRow>,
    active_provider: String,
    selected_idx: usize,
    stage: Stage,
    api_key_input: String,
    add_name: String,
    add_base_url: String,
    add_protocol_idx: usize,
    add_api_key: String,
    add_field: AddField,
}

impl ProviderPickerView {
    #[must_use]
    pub fn new(
        active_provider: &str,
        registry: &crate::provider_registry::ProviderRegistry,
        _config: &Config,
    ) -> Self {
        let providers: Vec<ProviderRow> = registry
            .all_providers()
            .iter()
            .map(|p| ProviderRow {
                name: p.name.clone(),
                display_name: p.display_name.clone(),
                has_key: p.api_key.is_some(),
                is_builtin: p.is_builtin,
            })
            .collect();

        let selected_idx = providers
            .iter()
            .position(|p| p.name == active_provider)
            .unwrap_or(0);

        Self {
            providers,
            active_provider: active_provider.to_string(),
            selected_idx,
            stage: Stage::List,
            api_key_input: String::new(),
            add_name: String::new(),
            add_base_url: String::new(),
            add_protocol_idx: 0,
            add_api_key: String::new(),
            add_field: AddField::Name,
        }
    }

    fn move_up(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.selected_idx + 1 < self.providers.len() {
            self.selected_idx += 1;
        }
    }

    fn selected_name(&self) -> &str {
        &self.providers[self.selected_idx].name
    }

    fn selected_has_key(&self) -> bool {
        self.providers[self.selected_idx].has_key
    }

    fn env_var_for(name: &str) -> String {
        let upper = name.to_ascii_uppercase().replace('-', "_");
        format!("DEEPSEEK_PROVIDER_{}_API_KEY", upper)
    }

    fn provider_hint(row: &ProviderRow) -> String {
        if row.name == "ollama" {
            return "self-hosted; defaults to http://localhost:11434".to_string();
        }
        if row.name == "sglang" || row.name == "vllm" {
            return if row.has_key {
                "(configured; optional key)".to_string()
            } else {
                "(optional key)".to_string()
            };
        }
        if row.has_key {
            "(configured)".to_string()
        } else {
            "(needs API key)".to_string()
        }
    }

    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        let outer = Block::default()
            .title(Line::from(Span::styled(
                " Provider ",
                Style::default()
                    .fg(palette::DEEPSEEK_SKY)
                    .add_modifier(Modifier::BOLD),
            )))
            .title_bottom(Line::from(vec![
                Span::styled(" ↑↓ ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("move "),
                Span::styled(" Enter ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("apply "),
                Span::styled(" d ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("delete "),
                Span::styled(" a ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("add "),
                Span::styled(" Esc ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("cancel "),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_COLOR))
            .style(Style::default());
        let inner = outer.inner(area);
        outer.render(area, buf);

        let mut lines: Vec<Line> = Vec::with_capacity(self.providers.len());
        for (idx, row) in self.providers.iter().enumerate() {
            let is_selected = idx == self.selected_idx;
            let is_active = row.name == self.active_provider;
            let arrow = if is_selected { "▸" } else { " " };
            let active_dot = if is_active { " *" } else { "  " };
            let label_style = if is_selected {
                Style::default()
                    .fg(palette::SELECTION_TEXT)
                    .bg(palette::SELECTION_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_PRIMARY)
            };
            let hint_style = if is_selected {
                Style::default()
                    .fg(palette::SELECTION_TEXT)
                    .bg(palette::SELECTION_BG)
            } else if row.has_key {
                Style::default().fg(palette::TEXT_MUTED)
            } else {
                Style::default().fg(palette::STATUS_WARNING)
            };
            let hint = Self::provider_hint(row);
            let badge = if !row.is_builtin {
                " (user-defined)"
            } else {
                ""
            };
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(arrow, label_style),
                Span::raw(" "),
                Span::styled(row.display_name.clone(), label_style),
                Span::styled(badge.to_string(), hint_style),
                Span::styled(active_dot, label_style),
                Span::raw("  "),
                Span::styled(hint, hint_style),
            ]));
        }
        Paragraph::new(lines).render(inner, buf);
    }

    fn render_key_entry(&self, area: Rect, buf: &mut Buffer) {
        let display_name = &self.providers[self.selected_idx].display_name;
        let outer = Block::default()
            .title(Line::from(Span::styled(
                format!(" API key — {} ", display_name),
                Style::default()
                    .fg(palette::DEEPSEEK_SKY)
                    .add_modifier(Modifier::BOLD),
            )))
            .title_bottom(Line::from(vec![
                Span::styled(" Enter ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("save & switch "),
                Span::styled(" Esc ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("back "),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_COLOR))
            .style(Style::default());
        let inner = outer.inner(area);
        outer.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Min(1),
            ])
            .split(inner);

        let masked = mask_key(&self.api_key_input);
        let display = if masked.is_empty() {
            "(paste key here)".to_string()
        } else {
            masked
        };
        let key_lines = vec![Line::from(vec![
            Span::styled("Key: ", Style::default().fg(palette::TEXT_MUTED)),
            Span::styled(
                display,
                Style::default()
                    .fg(palette::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
        ])];
        Paragraph::new(key_lines).render(layout[0], buf);

        let hint = format!(
            "Or set the {} environment variable and re-open /provider.",
            Self::env_var_for(self.selected_name()),
        );
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(palette::TEXT_MUTED),
        )))
        .render(layout[1], buf);
    }
}

fn mask_key(input: &str) -> String {
    let trimmed = input.trim();
    let len = trimmed.chars().count();
    if len == 0 {
        return String::new();
    }
    if len <= 4 {
        return "*".repeat(len);
    }
    let visible: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{}{}", "*".repeat(len - 4), visible)
}

impl ModalView for ProviderPickerView {
    fn kind(&self) -> ModalKind {
        ModalKind::ProviderPicker
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_paste(&mut self, text: &str) -> bool {
        if self.stage == Stage::KeyEntry {
            let sanitized: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            if !sanitized.is_empty() {
                self.api_key_input.push_str(&sanitized);
            }
            true
        } else {
            false
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
        match self.stage {
            Stage::List => match key.code {
                KeyCode::Esc => ViewAction::Close,
                KeyCode::Up => {
                    self.move_up();
                    ViewAction::None
                }
                KeyCode::Down => {
                    self.move_down();
                    ViewAction::None
                }
                KeyCode::Enter => {
                    if self.selected_has_key() {
                        ViewAction::EmitAndClose(ViewEvent::ProviderPickerApplied {
                            provider_name: self.selected_name().to_string(),
                        })
                    } else {
                        self.stage = Stage::KeyEntry;
                        self.api_key_input.clear();
                        ViewAction::None
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    if !self.providers[self.selected_idx].is_builtin {
                        self.stage = Stage::ConfirmDelete;
                        ViewAction::None
                    } else {
                        ViewAction::None
                    }
                }
                KeyCode::Char('a') => {
                    self.stage = Stage::AddProvider;
                    self.add_name.clear();
                    self.add_base_url.clear();
                    self.add_protocol_idx = 0;
                    self.add_api_key.clear();
                    self.add_field = AddField::Name;
                    ViewAction::None
                }
                _ => ViewAction::None,
            },
            Stage::KeyEntry => match key.code {
                KeyCode::Esc => {
                    self.stage = Stage::List;
                    self.api_key_input.clear();
                    ViewAction::None
                }
                KeyCode::Backspace => {
                    self.api_key_input.pop();
                    ViewAction::None
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.api_key_input.pop();
                    ViewAction::None
                }
                KeyCode::Enter => {
                    let key = self.api_key_input.trim().to_string();
                    if key.is_empty() {
                        ViewAction::None
                    } else {
                        ViewAction::EmitAndClose(ViewEvent::ProviderPickerApiKeySubmitted {
                            provider_name: self.selected_name().to_string(),
                            api_key: key,
                        })
                    }
                }
                KeyCode::Char(c) => {
                    if !c.is_whitespace() {
                        self.api_key_input.push(c);
                    }
                    ViewAction::None
                }
                _ => ViewAction::None,
            },
            Stage::ConfirmDelete => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    ViewAction::EmitAndClose(ViewEvent::ProviderPickerDeleteRequested {
                        provider_name: self.selected_name().to_string(),
                    })
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.stage = Stage::List;
                    ViewAction::None
                }
                _ => ViewAction::None,
            },
            Stage::AddProvider => match key.code {
                KeyCode::Esc => {
                    self.stage = Stage::List;
                    ViewAction::None
                }
                KeyCode::Tab => {
                    self.add_field = match self.add_field {
                        AddField::Name => AddField::BaseUrl,
                        AddField::BaseUrl => AddField::Protocol,
                        AddField::Protocol => AddField::ApiKey,
                        AddField::ApiKey => AddField::Name,
                    };
                    ViewAction::None
                }
                KeyCode::Up if self.add_field == AddField::Protocol => {
                    if self.add_protocol_idx > 0 {
                        self.add_protocol_idx -= 1;
                    }
                    ViewAction::None
                }
                KeyCode::Down if self.add_field == AddField::Protocol => {
                    if self.add_protocol_idx + 1 < PROTOCOL_OPTIONS.len() {
                        self.add_protocol_idx += 1;
                    }
                    ViewAction::None
                }
                KeyCode::Backspace => {
                    match self.add_field {
                        AddField::Name => {
                            self.add_name.pop();
                        }
                        AddField::BaseUrl => {
                            self.add_base_url.pop();
                        }
                        AddField::ApiKey => {
                            self.add_api_key.pop();
                        }
                        AddField::Protocol => {}
                    }
                    ViewAction::None
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    match self.add_field {
                        AddField::Name => {
                            self.add_name.pop();
                        }
                        AddField::BaseUrl => {
                            self.add_base_url.pop();
                        }
                        AddField::ApiKey => {
                            self.add_api_key.pop();
                        }
                        AddField::Protocol => {}
                    }
                    ViewAction::None
                }
                KeyCode::Enter => {
                    if !self.add_name.is_empty() && !self.add_base_url.is_empty() {
                        ViewAction::EmitAndClose(ViewEvent::ProviderPickerAddRequested {
                            name: self.add_name.trim().to_string(),
                            base_url: self.add_base_url.trim().to_string(),
                            protocol: PROTOCOL_OPTIONS[self.add_protocol_idx].to_string(),
                            api_key: if self.add_api_key.trim().is_empty() {
                                None
                            } else {
                                Some(self.add_api_key.trim().to_string())
                            },
                        })
                    } else {
                        ViewAction::None
                    }
                }
                KeyCode::Char(c) => {
                    match self.add_field {
                        AddField::Name => {
                            self.add_name.push(c);
                        }
                        AddField::BaseUrl => {
                            self.add_base_url.push(c);
                        }
                        AddField::ApiKey if !c.is_whitespace() => {
                            self.add_api_key.push(c);
                        }
                        _ => {}
                    }
                    ViewAction::None
                }
                _ => ViewAction::None,
            },
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let popup_width = 64.min(area.width.saturating_sub(4)).max(40);
        let popup_height = match self.stage {
            Stage::List => 12,
            Stage::KeyEntry => 10,
            Stage::ConfirmDelete => 8,
            Stage::AddProvider => 14,
        }
        .min(area.height.saturating_sub(4))
        .max(8);
        let popup_area = Rect {
            x: area.x + (area.width.saturating_sub(popup_width)) / 2,
            y: area.y + (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        };
        Clear.render(popup_area, buf);
        match self.stage {
            Stage::List => self.render_list(popup_area, buf),
            Stage::KeyEntry => self.render_key_entry(popup_area, buf),
            Stage::ConfirmDelete => self.render_confirm_delete(popup_area, buf),
            Stage::AddProvider => self.render_add_provider(popup_area, buf),
        }
    }
}

impl ProviderPickerView {
    fn render_add_provider(&self, area: Rect, buf: &mut Buffer) {
        let outer = Block::default()
            .title(Line::from(Span::styled(
                " Add Provider ",
                Style::default()
                    .fg(palette::DEEPSEEK_SKY)
                    .add_modifier(Modifier::BOLD),
            )))
            .title_bottom(Line::from(vec![
                Span::styled(" Tab ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("next "),
                Span::styled(" Enter ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("save "),
                Span::styled(" Esc ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("cancel "),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_COLOR));
        let inner = outer.inner(area);
        outer.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Min(1),
            ])
            .split(inner);

        let field_style = |active: bool| {
            if active {
                Style::default()
                    .fg(palette::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_MUTED)
            }
        };
        let label_style = |active: bool| {
            if active {
                Style::default()
                    .fg(palette::DEEPSEEK_SKY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_MUTED)
            }
        };

        let name_val = if self.add_name.is_empty() {
            "(name)"
        } else {
            &self.add_name
        };
        let base_val = if self.add_base_url.is_empty() {
            "(base url)"
        } else {
            &self.add_base_url
        };
        let proto_val = PROTOCOL_OPTIONS[self.add_protocol_idx];
        let key_display = if self.add_api_key.is_empty() {
            "(optional)".to_string()
        } else {
            mask_key(&self.add_api_key)
        };

        Paragraph::new(Line::from(vec![
            Span::styled("Name: ", label_style(self.add_field == AddField::Name)),
            Span::styled(name_val, field_style(self.add_field == AddField::Name)),
        ]))
        .render(layout[0], buf);

        Paragraph::new(Line::from(vec![
            Span::styled("URL:  ", label_style(self.add_field == AddField::BaseUrl)),
            Span::styled(base_val, field_style(self.add_field == AddField::BaseUrl)),
        ]))
        .render(layout[1], buf);

        Paragraph::new(Line::from(vec![
            Span::styled("Proto:", label_style(self.add_field == AddField::Protocol)),
            Span::raw(" "),
            Span::styled(proto_val, field_style(self.add_field == AddField::Protocol)),
            if self.add_field == AddField::Protocol {
                Span::styled(" ↑↓", Style::default().fg(palette::TEXT_MUTED))
            } else {
                Span::raw("")
            },
        ]))
        .render(layout[2], buf);

        Paragraph::new(Line::from(vec![
            Span::styled("Key:  ", label_style(self.add_field == AddField::ApiKey)),
            Span::styled(
                &key_display,
                field_style(self.add_field == AddField::ApiKey),
            ),
        ]))
        .render(layout[3], buf);
    }

    fn render_confirm_delete(&self, area: Rect, buf: &mut Buffer) {
        let name = &self.providers[self.selected_idx].display_name;
        let outer = Block::default()
            .title(Line::from(Span::styled(
                format!(" Delete {}? ", name),
                Style::default()
                    .fg(palette::STATUS_WARNING)
                    .add_modifier(Modifier::BOLD),
            )))
            .title_bottom(Line::from(vec![
                Span::styled(" y ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("confirm "),
                Span::styled(" n/Esc ", Style::default().fg(palette::TEXT_MUTED)),
                Span::raw("cancel "),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_COLOR));
        let inner = outer.inner(area);
        outer.render(area, buf);
        let msg = format!("This will remove '{}' from your config.", name);
        Paragraph::new(Line::from(Span::styled(
            msg,
            Style::default().fg(palette::TEXT_MUTED),
        )))
        .render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> crate::provider_registry::ProviderRegistry {
        crate::provider_registry::ProviderRegistry::new()
    }

    fn make_config() -> crate::config::Config {
        crate::config::Config::default()
    }

    #[test]
    fn test_picker_new_selects_active() {
        let registry = make_registry();
        let config = make_config();
        let picker = ProviderPickerView::new("deepseek", &registry, &config);
        assert_eq!(picker.selected_name(), "deepseek");
    }

    #[test]
    fn test_picker_navigation() {
        let registry = make_registry();
        let config = make_config();
        let mut picker = ProviderPickerView::new("deepseek", &registry, &config);
        picker.move_down();
        assert_ne!(picker.selected_name(), "deepseek");
        picker.move_up();
        assert_eq!(picker.selected_name(), "deepseek");
    }

    #[test]
    fn test_picker_enter_without_key_goes_to_key_entry() {
        let registry = make_registry();
        let config = make_config();
        let mut picker = ProviderPickerView::new("ollama", &registry, &config);
        let action = picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(action, ViewAction::None));
    }

    #[test]
    fn test_picker_enter_with_key_applies() {
        let registry = make_registry();
        let config = make_config();
        let mut picker = ProviderPickerView::new("deepseek", &registry, &config);
        picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }

    #[test]
    fn test_picker_key_entry_submit() {
        let registry = make_registry();
        let config = make_config();
        let mut picker = ProviderPickerView::new("ollama", &registry, &config);
        picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        picker.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        picker.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        let action = picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match action {
            ViewAction::EmitAndClose(ViewEvent::ProviderPickerApiKeySubmitted {
                provider_name,
                api_key,
            }) => {
                assert_eq!(provider_name, "ollama");
                assert_eq!(api_key, "sk");
            }
            other => panic!(
                "expected EmitAndClose(ProviderPickerApiKeySubmitted), got {:?}",
                other
            ),
        }
    }
}
