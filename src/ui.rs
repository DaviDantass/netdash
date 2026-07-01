use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
    Terminal,
};
use tokio::sync::watch;

use crate::{
    app::AppState,
    config::{PARALLEL_STREAMS, WARMUP_DURATION},
};

pub async fn run_tui(rx: watch::Receiver<AppState>) -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app_loop(&mut terminal, rx).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rx: watch::Receiver<AppState>,
) -> Result<()> {
    let mut tick = tokio::time::interval(Duration::from_millis(100));

    loop {
        tick.tick().await;

        let state = rx.borrow().clone();

        terminal.draw(|frame| {
            let area = frame.area();

            let main = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Length(9),
                    Constraint::Length(7),
                    Constraint::Min(3),
                ])
                .split(area);

            let status_text = if state.error.is_some() {
                "Erro"
            } else if state.done {
                "Finalizado"
            } else if state.elapsed_secs < WARMUP_DURATION.as_secs_f64() {
                "Aquecendo..."
            } else {
                "Medindo download..."
            };

            let header = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(
                        "⚡ NetDash",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "Rust CLI Speed Test",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        status_text,
                        Style::default().fg(if state.error.is_some() {
                            Color::Red
                        } else if state.done {
                            Color::Green
                        } else {
                            Color::Yellow
                        }),
                    ),
                    Span::raw(" | "),
                    Span::styled(
                        format!("{:.1}s / 8.0s", state.elapsed_secs.min(8.0)),
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Status "),
            );

            frame.render_widget(header, main[0]);

            let speed_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(main[1]);

            let big_speed = Paragraph::new(format!("{:.2} Mbps", state.average_mbps))
                .style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green))
                        .title(" Média final/parcial "),
                );

            frame.render_widget(big_speed, speed_area[0]);

            let percent = (state.average_mbps / 1000.0).min(1.0);

            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta))
                        .title(" Escala até 1 Gbps "),
                )
                .gauge_style(
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
                .ratio(percent)
                .label(format!("{:.0}%", percent * 100.0));

            frame.render_widget(gauge, speed_area[1]);

            let info = Paragraph::new(format!(
                "Instantâneo: {:.2} Mbps | Baixado medido: {:.2} MB | Streams: {}",
                state.download_mbps, state.total_mb, PARALLEL_STREAMS
            ))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Dados "),
            );

            frame.render_widget(info, speed_area[2]);

            let sparkline = Sparkline::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title(" Variação em tempo real "),
                )
                .data(&state.history)
                .style(Style::default().fg(Color::Yellow))
                .bar_set(symbols::bar::NINE_LEVELS);

            frame.render_widget(sparkline, main[2]);

            let footer_text = if let Some(error) = &state.error {
                format!("Erro: {error} | pressione 'q' para sair")
            } else {
                "Pressione 'q' para sair | 0.8s aquecimento + 7.2s medição real | sem salvar arquivos"
                    .to_string()
            };

            let footer = Paragraph::new(footer_text)
                .alignment(Alignment::Center)
                .style(if state.error.is_some() {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::DarkGray)
                })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray))
                        .title(" Ajuda "),
                );

            frame.render_widget(footer, main[3]);
        })?;

        if event::poll(Duration::from_millis(1))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    Ok(())
}