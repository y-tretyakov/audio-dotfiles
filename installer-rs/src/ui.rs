use std::cell::Cell;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::{AppMode, AppStep, AppState, StepStatus};

// ── Color Scheme ──────────────────────────────────────────────────────────────
const COLOR_PRIMARY: Color = Color::Cyan;
const COLOR_SUCCESS: Color = Color::Green;
const COLOR_WARNING: Color = Color::Yellow;
const COLOR_ERROR: Color = Color::Red;

// ── Spinner frame counter ─────────────────────────────────────────────────────
thread_local! {
    static FRAME: Cell<u64> = const { Cell::new(0) };
}

fn running_step_index(step: &AppStep) -> Option<usize> {
    match step {
        AppStep::BackingUp => Some(0),
        AppStep::DeployingPresets => Some(1),
        AppStep::DeployingIRs => Some(2),
        AppStep::DeployingPipeWire => Some(3),
        AppStep::DeployingNiri => Some(4),
        AppStep::RestartingServices => Some(5),
        _ => None,
    }
}

// ── Public API ────────────────────────────────────────────────────────────────
pub fn render(f: &mut Frame<'_>, app: &mut AppState) {
    FRAME.with(|c| c.set(c.get() + 1));

    let area = f.size();
    let chunks = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(f, chunks[0], app);
    draw_main(f, chunks[1], app);
    draw_footer(f, chunks[2], app);
}

// ── Header Zone ───────────────────────────────────────────────────────────────
fn draw_header(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::bordered()
        .title(" System Info ")
        .title_style(Style::new().fg(COLOR_PRIMARY).add_modifier(Modifier::BOLD))
        .border_style(Style::new().fg(COLOR_PRIMARY));

    let mode_tag = match app.mode {
        AppMode::Deploy => "DEPLOY",
        AppMode::Rollback => "ROLLBACK",
        AppMode::DryRun => "DRY-RUN",
    };
    let mode_color = match app.mode {
        AppMode::Deploy => COLOR_SUCCESS,
        AppMode::Rollback => COLOR_WARNING,
        AppMode::DryRun => COLOR_PRIMARY,
    };

    let mut lines = vec![
        Line::from(Span::styled(
            "audio-manager v0.1.0",
            Style::new()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::raw("Host: "),
            Span::styled("HP Pavilion 15", Style::new().fg(COLOR_SUCCESS)),
            Span::raw("  |  OS: "),
            Span::styled("CachyOS (Arch Linux)", Style::new().fg(COLOR_SUCCESS)),
            Span::raw("  |  Audio: "),
            Span::styled(
                "PipeWire + WirePlumber",
                Style::new().fg(COLOR_SUCCESS),
            ),
        ]),
        Line::from(vec![
            Span::raw("Mode: "),
            Span::styled(mode_tag, Style::new().fg(mode_color).add_modifier(Modifier::BOLD)),
        ]),
    ];

    if app.mode == AppMode::Rollback || app.mode == AppMode::DryRun {
        let warning = match app.mode {
            AppMode::Rollback => {
                "⚠ ROLLBACK MODE — existing configs will be restored from backup"
            }
            AppMode::DryRun => "⚠ DRY-RUN MODE — no changes will be made to the system",
            _ => unreachable!(),
        };
        lines.push(Line::from(Span::styled(
            warning,
            Style::new()
                .fg(COLOR_WARNING)
                .add_modifier(Modifier::BOLD),
        )));
    }

    let para = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

// ── Main Content Zone ─────────────────────────────────────────────────────────
fn draw_main(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let chunks = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_left(f, chunks[0], app);
    draw_right(f, chunks[1], app);
}

fn draw_left(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let chunks = Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).split(area);

    draw_gauge(f, chunks[0], app);
    draw_steps(f, chunks[1], app);
}

fn draw_gauge(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::bordered()
        .title(" Progress ")
        .border_style(Style::new().fg(COLOR_PRIMARY));

    let gauge_color = if app.progress >= 1.0 {
        COLOR_WARNING
    } else {
        COLOR_SUCCESS
    };

    let gauge = Gauge::default()
        .block(block)
        .gauge_style(Style::new().fg(gauge_color))
        .ratio(app.progress as f64)
        .label(format!("Progress: {:.0}%", app.progress * 100.0));

    f.render_widget(gauge, area);
}

fn draw_steps(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let running_idx = running_step_index(&app.current_step);

    let items: Vec<ListItem> = app
        .step_statuses
        .iter()
        .enumerate()
        .map(|(i, (name, status))| {
            let (symbol, base_style) = match status {
                StepStatus::Pending => {
                    (" ", Style::new().fg(Color::DarkGray))
                }
                StepStatus::Running => {
                    ("►", Style::new().fg(COLOR_PRIMARY).add_modifier(Modifier::BOLD))
                }
                StepStatus::Done => {
                    ("✓", Style::new().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD))
                }
                StepStatus::Warning(_) => {
                    ("⚠", Style::new().fg(COLOR_WARNING).add_modifier(Modifier::BOLD))
                }
                StepStatus::Error(_) => {
                    ("✗", Style::new().fg(COLOR_ERROR).add_modifier(Modifier::BOLD))
                }
            };

            let item_style = if Some(i) == running_idx {
                base_style.bg(Color::Rgb(30, 30, 60))
            } else {
                base_style
            };

            ListItem::new(Line::from(Span::styled(
                format!(" {} {}", symbol, name),
                item_style,
            )))
        })
        .collect();

    let list = List::new(items).block(
        Block::bordered()
            .title(" Steps ")
            .border_style(Style::new().fg(COLOR_PRIMARY)),
    );

    f.render_widget(list, area);
}

fn draw_right(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lines: Vec<Line> = app
        .log_messages
        .iter()
        .rev()
        .take(15)
        .rev()
        .map(|msg| {
            let style = if msg.contains("Error")
                || msg.contains("error")
                || msg.starts_with("Error:")
            {
                Style::new().fg(COLOR_ERROR)
            } else if msg.contains("Warning") || msg.contains("warning") {
                Style::new().fg(COLOR_WARNING)
            } else {
                Style::new().fg(Color::DarkGray)
            };
            Line::from(Span::styled(msg.clone(), style))
        })
        .collect();

    let para = Paragraph::new(lines)
        .block(
            Block::bordered()
                .title(" Log ")
                .border_style(Style::new().fg(COLOR_PRIMARY)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);
}

// ── Footer Zone ───────────────────────────────────────────────────────────────
fn draw_footer(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let chunks = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let spinner_chars = ['|', '/', '-', '\\'];
    let frame = FRAME.with(|c| c.get());
    let spinner = spinner_chars[(frame / 5) as usize % 4];

    let status_line = match &app.current_step {
        AppStep::Idle => {
            let mut spans = vec![Span::styled(
                " [Q] Quit ",
                Style::new()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )];
            if app.mode != AppMode::Rollback {
                spans.push(Span::styled(
                    "[D] Deploy ",
                    Style::new()
                        .fg(COLOR_SUCCESS)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            spans.push(Span::styled(
                "[R] Rollback ",
                Style::new()
                    .fg(COLOR_WARNING)
                    .add_modifier(Modifier::BOLD),
            ));
            Line::from(spans)
        }
        AppStep::BackingUp
        | AppStep::DeployingPresets
        | AppStep::DeployingIRs
        | AppStep::DeployingPipeWire
        | AppStep::DeployingNiri
        | AppStep::RestartingServices => {
            let step_name = format!("{:?}", app.current_step);
            Line::from(vec![
                Span::styled(
                    format!(" {} ", spinner),
                    Style::new()
                        .fg(COLOR_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(step_name, Style::new().fg(Color::White)),
            ])
        }
        AppStep::Done => Line::from(Span::styled(
            " ✓ Deployment complete! ",
            Style::new()
                .fg(COLOR_SUCCESS)
                .add_modifier(Modifier::BOLD),
        )),
        AppStep::Error(msg) => Line::from(Span::styled(
            format!(" ✗ Error: {}", msg),
            Style::new()
                .fg(COLOR_ERROR)
                .add_modifier(Modifier::BOLD),
        )),
    };

    let status_para = Paragraph::new(status_line)
        .style(Style::new().fg(Color::White));
    f.render_widget(status_para, chunks[0]);

    let branding = Paragraph::new(Line::from(Span::styled(
        " audio-dotfiles ~ code-warlord ",
        Style::new().fg(Color::White),
    )))
    .style(Style::new())
    .alignment(Alignment::Right);

    f.render_widget(branding, chunks[1]);
}
