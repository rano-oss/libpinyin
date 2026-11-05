// Generate word-level bigram model from upstream's interpolation2.text
//
// Usage:
//   cargo run -p gen_word_bigrams [interpolation_file] [output_dir]
//
// Examples:
//   cargo run -p gen_word_bigrams data/interpolation2.text data/converted/simplified
//   cargo run -p gen_word_bigrams data/zhuyin/interpolation2.text data/converted/traditional
//
// Outputs word_bigram.dat + word_bigram_words.fst in the output directory.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tools_common::{WordBigram, WordBigramBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let interpolation_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("data/interpolation2.text")
    };

    let output_dir = if args.len() > 4 {
        PathBuf::from(&args[4])
    } else if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        PathBuf::from("data/converted/simplified")
    };

    println!(
        "Extracting unigrams and bigrams from {}...",
        interpolation_path.display()
    );
    let (unigram_counts, bigram_counts) = extract_from_interpolation(&interpolation_path)?;

    println!("Building word bigram model...");
    let word_bigram = build_word_bigram_model(&unigram_counts, &bigram_counts);

    let dat_path = output_dir.join("word_bigram.dat");
    let fst_path = output_dir.join("word_bigram_words.fst");
    println!("Writing to {:?}...", output_dir);
    word_bigram.write_mmap(&dat_path, &fst_path)?;

    println!("\nWord bigram model generated!");
    println!("  Total unigrams: {}", unigram_counts.len());
    println!("  Total unique first words: {}", word_bigram.len());

    let total_bigrams = word_bigram.total_bigrams();
    println!("  Total bigram entries: {}", total_bigrams);

    Ok(())
}

/// Parse interpolation2.text and extract both unigrams and bigrams
fn extract_from_interpolation(
    path: &PathBuf,
) -> Result<(HashMap<String, u32>, HashMap<String, HashMap<String, u32>>), Box<dyn std::error::Error>>
{
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut unigram_counts: HashMap<String, u32> = HashMap::new();
    let mut bigram_counts: HashMap<String, HashMap<String, u32>> = HashMap::new();
    let mut in_unigram_section = false;
    let mut in_bigram_section = false;
    let mut unigram_lines = 0;
    let mut bigram_lines = 0;
    let mut extracted_unigrams = 0;
    let mut extracted_bigrams = 0;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        // Check for section markers
        if trimmed == "\\1-gram" {
            in_unigram_section = true;
            in_bigram_section = false;
            println!("  Found \\1-gram section...");
            continue;
        } else if trimmed == "\\2-gram" {
            in_unigram_section = false;
            in_bigram_section = true;
            println!("  Found \\2-gram section...");
            continue;
        } else if trimmed.starts_with("\\end") || trimmed.starts_with("\\3-gram") {
            println!("  Reached end of sections");
            break;
        }

        // Parse unigram entries
        if in_unigram_section && trimmed.starts_with("\\item ") {
            unigram_lines += 1;

            // Format: \item token phrase count N
            // Example: \item 16867717 南京大屠杀 count 51
            if let Some((phrase, count)) = parse_unigram_line(trimmed) {
                // Include all unigrams from interpolation2.text
                // (they're already filtered to lexicon words by upstream training)
                *unigram_counts.entry(phrase).or_insert(0) += count;
                extracted_unigrams += 1;
            }

            if unigram_lines % 10000 == 0 {
                println!("    Processed {} unigram lines...", unigram_lines);
            }
        }

        // Parse bigram entries
        if in_bigram_section && trimmed.starts_with("\\item ") {
            bigram_lines += 1;

            // Parse format: \item token1 phrase1 token2 phrase2 count N
            // Example: \item 16867717 南京大屠杀 16778715 的 count 16
            if let Some((phrase1, phrase2, count)) = parse_bigram_line(trimmed) {
                bigram_counts
                    .entry(phrase1)
                    .or_default()
                    .entry(phrase2)
                    .and_modify(|c| *c += count)
                    .or_insert(count);

                extracted_bigrams += 1;

                if extracted_bigrams % 10000 == 0 {
                    println!("    Extracted {} bigrams...", extracted_bigrams);
                }
            }
        }
    }

    println!(
        "  Processed {} unigram lines, extracted {}",
        unigram_lines, extracted_unigrams
    );
    println!(
        "  Processed {} bigram lines, extracted {}",
        bigram_lines, extracted_bigrams
    );

    Ok((unigram_counts, bigram_counts))
}

/// Parse a unigram line from interpolation2.text
/// Format: \item token phrase count N
/// Returns (phrase, count)
fn parse_unigram_line(line: &str) -> Option<(String, u32)> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    // Need at least: \item token phrase count N
    if parts.len() < 5 || parts[0] != "\\item" {
        return None;
    }

    // Find "count" keyword
    let count_idx = parts.iter().position(|&p| p == "count")?;
    if count_idx + 1 >= parts.len() {
        return None;
    }

    // Parse count value
    let count: u32 = parts[count_idx + 1].parse().ok()?;

    // The phrase is between token and "count"
    // parts[1] is token, parts[2..count_idx] is the phrase
    if count_idx < 3 {
        return None;
    }

    let phrase = parts[2..count_idx].join("");

    Some((phrase, count))
}

/// Parse a bigram line from interpolation2.text
/// Format: \item token1 phrase1 token2 phrase2 count N
/// Returns (phrase1, phrase2, count)
fn parse_bigram_line(line: &str) -> Option<(String, String, u32)> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    // Must have at least: \item token1 phrase1 token2 phrase2 count N
    if parts.len() < 6 || parts[parts.len() - 2] != "count" {
        return None;
    }

    // Parse count (last element)
    let count = parts[parts.len() - 1].parse::<u32>().ok()?;

    // Strategy: Find token2 by looking for a number after phrase1
    // token1 is at index 1
    // phrase1 starts at index 2
    // token2 is a number that appears after phrase1
    // phrase2 follows token2

    let mut phrase1_parts = Vec::new();
    let mut phrase2_parts = Vec::new();
    let mut found_token2 = false;
    let mut seen_phrase1 = false;

    for i in 2..(parts.len() - 2) {
        // Stop before "count N"
        if !seen_phrase1 {
            // Check if this could be token2 (a number after we've seen at least one phrase part)
            if let Ok(_token) = parts[i].parse::<u32>() {
                if !phrase1_parts.is_empty() {
                    seen_phrase1 = true;
                    found_token2 = true;
                    continue; // Skip token2 itself
                }
            }
            phrase1_parts.push(parts[i]);
        } else {
            phrase2_parts.push(parts[i]);
        }
    }

    if !found_token2 || phrase1_parts.is_empty() || phrase2_parts.is_empty() {
        return None;
    }

    let phrase1 = phrase1_parts.join("");
    let phrase2 = phrase2_parts.join("");

    Some((phrase1, phrase2, count))
}

/// Build WordBigram model from unigram and bigram counts
fn build_word_bigram_model(
    unigram_counts: &HashMap<String, u32>,
    bigram_counts: &HashMap<String, HashMap<String, u32>>,
) -> WordBigram {
    let mut builder = WordBigramBuilder::new();

    // Add unigrams
    for (word, &count) in unigram_counts {
        builder.add_unigram(word.clone(), count);
    }

    // Add bigrams
    for (word1, following_words) in bigram_counts {
        for (word2, &count) in following_words {
            builder.add_bigram(word1.clone(), word2.clone(), count);
        }
    }

    builder.build()
}
