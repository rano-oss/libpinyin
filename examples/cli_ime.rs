//! Simple CLI IME demo to test the editor architecture and mode switching.
//!
//! This is a terminal-based IME that demonstrates the complete flow:
//! - Pluggable editor architecture
//! - Mode switching (Phonetic, Punctuation)
//! - Key input processing with auxiliary text
//! - Candidate display and selection
//! - State management
//!
//! Run with: cargo run --example cli_ime

use libpinyin::{Editor, Engine, KeyEvent, KeyResult};
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== libpinyin CLI IME Demo (Phase 2: Editor Architecture) ===");
    println!();
    println!("✨ Features:");
    println!("  • Pluggable editor architecture");
    println!("  • Mode switching: Phonetic (拼音) ↔ Punctuation (标点)");
    println!("  • Auxiliary text with helpful hints");
    println!();
    println!("📝 Commands:");
    println!("  [Phonetic Mode]");
    println!("    - Type a-z: input pinyin");
    println!("    - Space: select first candidate");
    println!("    - 1-9: select candidate by number");
    println!("    - Enter: commit selection or raw input");
    println!("    - ,: switch to punctuation mode");
    println!("  [Punctuation Mode]");
    println!("    - 1-9/Space: select punctuation variant");
    println!("    - Esc: cancel and use original");
    println!("  [General]");
    println!("    - Backspace: delete previous character");
    println!("    - Esc: cancel input");
    println!("    - 'quit' or Ctrl+C: exit");
    println!();

    // Load the IME engine
    println!("Loading engine from data directory...");
    let data_dir = std::env::current_dir()?.join("data/converted/simplified");
    let backend = Engine::from_data_dir(&data_dir)?;
    let mut ime = Editor::with_page_size(backend, 9);

    println!("✓ Engine loaded successfully!");
    println!();
    println!("Try typing 'nihao,' to see phonetic → punctuation mode switching!");
    println!();

    // Main input loop
    loop {
        print!("> ");
        io::stdout().flush()?;

        // Read one line of input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input == "quit" || input.is_empty() {
            println!("Goodbye!");
            break;
        }

        // Process each character as a key event
        for ch in input.chars() {
            let key = match ch {
                ' ' => KeyEvent::Space,
                '\n' | '\r' => KeyEvent::Enter,
                '1'..='9' => KeyEvent::Number((ch as u8) - b'0'),
                ',' | '.' | ';' | '!' | '?' | '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}'
                | '<' | '>' | ':' => KeyEvent::Char(ch),
                _ if ch.is_ascii_lowercase() => KeyEvent::Char(ch),
                _ => {
                    println!("  ⚠ Ignoring unsupported character: {}", ch);
                    continue;
                }
            };

            let result = ime.process_key(key);

            // Display IME state after each key
            display_ime_state(&ime);

            // If there's commit text, show it
            if !ime.context.commit_text.is_empty() {
                let commit = ime.context.commit_text.clone();
                println!("  ✓ Committed: 「{}」", commit);
                println!();
                // Note: In a real IME, we would clear via platform callback
                // For demo purposes, we keep it to show what was committed
            }

            if result == KeyResult::NotHandled {
                println!("  ⓘ Key not handled by IME (passed through)");
            }
        }
    }

    Ok(())
}

fn display_ime_state(ime: &Editor) {
    let context = &ime.context;
    let session = &ime.session;

    // Show mode and auxiliary text prominently
    if session.active {
        println!("  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Mode indicator
        let mode_icon = match session.mode {
            libpinyin::InputMode::Phonetic => "🔤",
            libpinyin::InputMode::Punctuation => "🔣",
            libpinyin::InputMode::Suggestion => "💡",
            libpinyin::InputMode::Init => "⏸",
            libpinyin::InputMode::Passthrough => "🔄",
        };
        println!("  {} Mode: {:?}", mode_icon, session.mode);

        // Auxiliary text (helpful hints)
        if !context.auxiliary_text.is_empty() {
            println!("  ℹ {}", context.auxiliary_text);
        }

        println!("  ──────────────────────────────────────");
    }

    // Show preedit if present
    if !context.preedit_text.is_empty() {
        println!("  📝 Input: {}", context.preedit_text);
    }

    // Show candidates if present
    if !context.candidates.is_empty() {
        println!("  🎯 Candidates:");
        for (i, candidate) in context.candidates.iter().enumerate() {
            let marker = if i == context.candidate_cursor {
                "▶"
            } else {
                " "
            };
            println!("     {} {}. {}", marker, i + 1, candidate);
        }
    }

    if session.active {
        println!("  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
}
