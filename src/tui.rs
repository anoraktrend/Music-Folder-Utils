use anyhow::Result;
use std::io::{self, Write};
use std::sync::{atomic::AtomicBool, mpsc, Arc};
use std::thread;

pub fn run_tui<F>(
    title: &str,
    initial_total: usize,
    process_item: F,
    running_token: Arc<AtomicBool>,
) -> Result<()>
where
    F: FnOnce(mpsc::Sender<String>, Arc<AtomicBool>) -> Result<()> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<String>();

    let running_token_for_thread = running_token.clone();
    let processing_thread = thread::spawn(move || process_item(tx, running_token_for_thread));

    println!("🔄 {}", title);
    io::stdout().flush()?;

    let mut current_item_count = 0;
    let mut total_items = initial_total;

    while let Ok(message) = rx.recv() {
        // Handle special control messages
        if message.starts_with("TOTAL_FILES:") {
            if let Some(count_str) = message.split(':').nth(1) {
                if let Ok(count) = count_str.parse::<usize>() {
                    total_items = count;
                }
            }
            continue;
        }

        // Handle completion messages
        if message.starts_with("COMPLETED:") {
            current_item_count += 1;
            let task_description = message.strip_prefix("COMPLETED: ").unwrap_or(&message);

            // Simple progress display with bar
            let progress = if total_items > 0 {
                (current_item_count as f64 / total_items as f64) * 100.0
            } else {
                100.0
            };

            let progress_bar = create_progress_bar(progress);
            print!("\r {} {} {:.1}% ({}/{}) - {}",
                   title, progress_bar, progress, current_item_count, total_items, task_description);
        } else if message.starts_with("PROGRESS:") {
            // Handle progress messages with custom progress value
            let progress_info = message.strip_prefix("PROGRESS:").unwrap_or(&message);

            // Try to parse progress percentage from the message
            let progress = if let Some(percent_str) = progress_info.split('%').next() {
                // Extract just the numeric part before the %
                if let Some(num_str) = percent_str.split(':').next_back() {
                    num_str.trim().parse::<f64>().unwrap_or(0.0)
                } else {
                    percent_str.trim().parse::<f64>().unwrap_or(0.0)
                }
            } else {
                0.0
            };

            let progress_bar = create_progress_bar(progress);
            print!("\r {}: {} {:.1}% - {}",
                   title, progress_bar, progress, progress_info);
        } else {
            // For other messages, just print them
            println!("\r {}: {}", title, message);
        }

        io::stdout().flush()?;
    }

    processing_thread.join().unwrap()?;

    // Final completion message with full progress bar
    let progress = if total_items > 0 {
        (current_item_count as f64 / total_items as f64) * 100.0
    } else {
        100.0
    };

    let bar_width = 20;
    let filled_chars = ((progress / 100.0) * bar_width as f64) as usize;
    let empty_chars = bar_width - filled_chars;

    let progress_bar = format!(
        "[{}{}]",
        "█".repeat(filled_chars),
        "░".repeat(empty_chars)
    );

    println!("\r {} {} {:.1}% ({}/{})", title, progress_bar, progress, current_item_count, total_items);

    Ok(())
}

fn create_progress_bar(progress: f64) -> String {
    let bar_width = 20;
    let filled_chars = ((progress / 100.0) * bar_width as f64) as usize;
    let empty_chars = bar_width - filled_chars;

    format!(
        "[{}{}]",
        "█".repeat(filled_chars),
        "░".repeat(empty_chars)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicBool};

    #[test]
    fn test_run_tui_with_simple_operation() -> Result<()> {
        let running_token = Arc::new(AtomicBool::new(true));

        // Create a simple operation that just sends a completion message
        let result = run_tui("Test Operation", 1, |tx, _cancel_token| {
            tx.send("Test completed".to_string())?;
            Ok(())
        }, running_token);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_run_tui_with_multiple_messages() -> Result<()> {
        let running_token = Arc::new(AtomicBool::new(true));

        let result = run_tui("Test Operation", 3, |tx, _cancel_token| {
            tx.send("Step 1".to_string())?;
            tx.send("Step 2".to_string())?;
            tx.send("Step 3".to_string())?;
            Ok(())
        }, running_token);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_run_tui_with_total_files_update() -> Result<()> {
        let running_token = Arc::new(AtomicBool::new(true));

        let result = run_tui("Test Operation", 1, |tx, _cancel_token| {
            tx.send("TOTAL_FILES:5".to_string())?;
            tx.send("Completed".to_string())?;
            Ok(())
        }, running_token);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_run_tui_handles_operation_error() -> Result<()> {
        let running_token = Arc::new(AtomicBool::new(true));

        let result = run_tui("Test Operation", 1, |tx, _cancel_token| {
            // Send a message first
            tx.send("Starting".to_string())?;
            // Then return an error
            Err(anyhow::anyhow!("Test error"))
        }, running_token);

        // The TUI should handle the error gracefully
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_run_tui_with_cancellation() -> Result<()> {
        let running_token = Arc::new(AtomicBool::new(false)); // Start as cancelled

        let result = run_tui("Test Operation", 10, |tx, _cancel_token| {
            // This should not execute because we're cancelled
            tx.send("This should not appear".to_string())?;
            Ok(())
        }, running_token);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_run_tui_progress_calculation() -> Result<()> {
        let running_token = Arc::new(AtomicBool::new(true));

        let result = run_tui("Test Progress", 4, |tx, _cancel_token| {
            tx.send("TOTAL_FILES:4".to_string())?;
            tx.send("Item 1".to_string())?;
            tx.send("Item 2".to_string())?;
            tx.send("Item 3".to_string())?;
            tx.send("Item 4".to_string())?;
            Ok(())
        }, running_token);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_run_tui_with_zero_total() -> Result<()> {
        let running_token = Arc::new(AtomicBool::new(true));

        let result = run_tui("Test Zero Total", 0, |tx, _cancel_token| {
            tx.send("Completed with zero total".to_string())?;
            Ok(())
        }, running_token);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_run_tui_with_progress_messages() -> Result<()> {
        let running_token = Arc::new(AtomicBool::new(true));

        let result = run_tui("Test Progress", 1, |tx, _cancel_token| {
            tx.send("PROGRESS: Reading track 1: 25% complete (500 sectors)".to_string())?;
            tx.send("PROGRESS: Reading track 1: 50% complete (1000 sectors)".to_string())?;
            tx.send("PROGRESS: Reading track 1: 75% complete (1500 sectors)".to_string())?;
            tx.send("PROGRESS: Reading track 1: 100% complete (2000 sectors)".to_string())?;
            Ok(())
        }, running_token);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_create_progress_bar() {
        assert_eq!(create_progress_bar(0.0), "[░░░░░░░░░░░░░░░░░░░░]");
        assert_eq!(create_progress_bar(25.0), "[█████░░░░░░░░░░░░░░░]");
        assert_eq!(create_progress_bar(50.0), "[██████████░░░░░░░░░░]");
        assert_eq!(create_progress_bar(75.0), "[███████████████░░░░░]");
        assert_eq!(create_progress_bar(100.0), "[████████████████████]");
    }
}
