use anyhow::Result;
use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Gauge},
    Terminal,
};
use std::{
    io::{self, Stdout},
    time::{Duration, Instant},
};

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

pub fn run_tui<F>(title: &str, total_items: usize, mut process_item: F) -> Result<()> 
where
    F: FnMut(usize) -> Result<()>, 
{
    let mut terminal = setup_terminal()?;
    let mut current_item = 0;
    let _start_time = Instant::now();

    loop {
        terminal.draw(|frame| {
            let size = frame.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Percentage(10),
                        Constraint::Percentage(80),
                        Constraint::Percentage(10),
                    ]
                    .as_ref(),
                )
                .split(size);

            let block = Block::default().title(title).borders(Borders::ALL);
            frame.render_widget(block, chunks[0]);

            let progress = if total_items > 0 { current_item as f64 / total_items as f64 } else { 0.0 };
            let gauge = Gauge::default()
                .block(Block::default().title("Progress").borders(Borders::ALL))
                .gauge_style(ratatui::style::Style::default().fg(ratatui::style::Color::Green))
                .percent((progress * 100.0) as u16);
            frame.render_widget(gauge, chunks[1]);
        })?;

        if current_item < total_items {
            process_item(current_item)?;
            current_item += 1;
        } else {
            break;
        }

        if event::poll(Duration::from_millis(250))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    break;
                }
            }
        }
    }

    restore_terminal(terminal)?;
    Ok(())
}
