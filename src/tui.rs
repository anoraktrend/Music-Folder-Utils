use anyhow::Result;
use std::sync::{mpsc, Arc, atomic::AtomicBool};
use std::thread;
use std::io::{self, Write};

pub fn run_tui<F>(title: &str, total_items: usize, process_item: F, running_token: Arc<AtomicBool>) -> Result<()> 
where
    F: FnOnce(mpsc::Sender<String>, Arc<AtomicBool>) -> Result<()> + Send + 'static, 
{
    let (tx, rx) = mpsc::channel::<String>();

    let running_token_for_thread = running_token.clone();
    let processing_thread = thread::spawn(move || {
        process_item(tx, running_token_for_thread)
    });

    let mut current_item_count = 0;
    let mut current_item_name: String;

    // Print initial display (title + 2 empty lines for progress display)
    // Title line, then two empty lines (one for the progress bar, one for the current item)
    print!("Starting: {}\n\n\n", title);
    io::stdout().flush()?;

    while let Ok(name) = rx.recv() {
        // Primary recv - handle this message, then quickly drain any additional
        // messages that arrived so the UI updates to the latest finished command
        current_item_name = name;
        current_item_count += 1;

        // Drain any queued messages to keep the UI up-to-date when producers send
        // bursts of updates. For each drained message increment the count and
        // update the displayed name to the most recent.
        for queued in rx.try_iter() {
            current_item_name = queued;
            current_item_count += 1;
        }

        // Avoid divide-by-zero when there are no items
        let progress = if total_items == 0 {
            100.0
        } else {
            (current_item_count as f64 / total_items as f64) * 100.0
        };

    // Determine terminal width and compute bar length to fill available space.
    // Reserve space for the percentage and counts text and a small padding.
    let cols = crossterm::terminal::size().map(|(c, _r)| c as usize).unwrap_or(80);
    let meta = format!(" {:.2}% - {}/{}", progress, current_item_count, total_items);
    // Reserve for brackets + one space padding
    let reserved = meta.len().saturating_add(3);
    let bar_length = if cols > reserved { cols - reserved } else { 10 };
    let mut filled_length = ((progress / 100.0) * bar_length as f64).round() as usize;
    if filled_length > bar_length { filled_length = bar_length; }
    let bar = "#".repeat(filled_length) + &"-".repeat(bar_length - filled_length);

        // Move cursor up to the progress bar line (two lines above the cursor) and clear it
        // After initial header we have two blank lines reserved (bar + item), so move up 2
        print!("\x1B[2A");
        // Clear bar line, print bar
        print!("\x1B[2K\r[{}] {:.2}% - {}/{}\n", 
            bar, progress, current_item_count, total_items);
        // Clear item line and print current item
        print!("\x1B[2K\r{}\n", current_item_name);
        io::stdout().flush()?;
    }

    processing_thread.join().unwrap()?;

    // Clear both progress lines (bar + item) then show finished message
    print!("\x1B[2A\x1B[2K\x1B[1B\x1B[2K"); // move up 2, clear bar, move down 1, clear item
    println!("Finished: {}", title);
    io::stdout().flush()?;
    Ok(())
}
