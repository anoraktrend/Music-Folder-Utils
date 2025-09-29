use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::Print,
    terminal::{self, Clear, ClearType},
};
use std::io::{self, stdout, Write};
use std::sync::{
    mpsc,
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

const TOTAL_PROGRESS_WIDTH: u16 = 50;

pub fn run_tui(rx: mpsc::Receiver<String>, cancel_token: Arc<AtomicBool>) -> Result<(), io::Error> {
    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, cursor::Hide)?;

    let mut last_message = String::new();
    let mut total_files = 0;
    let mut completed_files = 0;

    loop {
        if !cancel_token.load(Ordering::SeqCst) {
            break;
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
                if (modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('c')) || code == KeyCode::Char('q') {
                    cancel_token.store(false, Ordering::SeqCst);
                    break;
                }
            }
        }

        match rx.try_recv() {
            Ok(msg) => {
                if msg.starts_with("TOTAL_FILES:") {
                    if let Ok(num) = msg.replace("TOTAL_FILES:", "").parse::<usize>() {
                        total_files = num;
                    }
                } else if msg.starts_with("COMPLETED:") {
                    completed_files += 1;
                    last_message = msg;
                } else {
                    last_message = msg;
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                break;
            }
        }

        let main_progress = if total_files > 0 {
            completed_files as f32 / total_files as f32
        } else {
            0.0
        };

        let filled_width = (main_progress * TOTAL_PROGRESS_WIDTH as f32) as u16;
        let empty_width = TOTAL_PROGRESS_WIDTH.saturating_sub(filled_width);

        let progress_bar = format!(
            "[{}{}] {:.1}%",
            "▓".repeat(filled_width as usize),
            "░".repeat(empty_width as usize),
            main_progress * 100.0
        );

        let (width, _) = terminal::size()?;
        let available_width = width.saturating_sub(progress_bar.len() as u16 + 1);
        let truncated_message = if last_message.len() > available_width as usize {
            if available_width > 3 {
                format!("{}...", &last_message[..available_width as usize - 3])
            } else {
                "".to_string()
            }
        } else {
            last_message.clone()
        };

        execute!(
            stdout,
            cursor::MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Print(format!("{} {}", progress_bar, truncated_message)),
        )?;

        stdout.flush()?;
    }

    execute!(stdout, cursor::Show)?;
    terminal::disable_raw_mode()?;
    println!();
    Ok(())
}