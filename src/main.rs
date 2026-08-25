// anyhow enables context trait for error handling
use anyhow::{Context, Result};
use clap::Parser;
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};
use regex::Regex;

const DROP_FILES: &[&str] = &[
    "oceanofpdf.com"
];
const EXCLUDE_MATCHING_PATTERNS: &[&str] = &[
    r"<div .*[Oo]ceanofPDF.*<\/div>"
];
const APPLIED_SPACER: &str = " ";
const SYMBOLS: &[&str] = &[
    // symbol + html entities encoding, compare: https://icl.utk.edu/~mgates3/docs/html-entities.html
    "–", "&ndash;", "&#8211;", "&#x2013;",          // 	en dash
    "—", "&mdash;", "&#8212;", "&#x2014;",          // 	em dash
    "…", "&hellip;", "&#8230;", "&#x2026;",         // 	horizontal ellipsis
];
const PERMITTED_NEIGHBORS: &[&str] = &[
    "\n", "\t", "<", ">",".", ",", "?", "!",
    // symbol + html entities encoding, compare: https://icl.utk.edu/~mgates3/docs/html-entities.html
    " ", "&#32;", "&#x20;",                         // regular spaces
    "\u{00A0}", "&nbsp;", "&#160;", "&#xA0;",       //  non-breaking space
	" ", "&thinsp;", "&#8201;", "&#x2009;",         // thin space
	" ", "&ensp;", "&#8194;", "&#x2002;",           //  en space
	" ", "&emsp;", "&#8195;", "&#x2003;",           //   em space
    " ", "&#8192;", "&#x2000;",                     // En Quad
    " ", "&#8193;", "&#x2001;",                     // Em Quad
];
const FILE_EXTENSIONS: &[&str] = &[
    ".html",
    ".xhtml",
    ".css",
    ".xml",
    ".htm",
];

#[derive(Parser, Debug)]
struct Args {
    // input file
    input: PathBuf,
    
    // output file
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    repack_epub(&args.input, &args.output)
}

fn repack_epub(input: &PathBuf, output: &PathBuf) -> Result<()> {
    let input_file = File::open(input)
        // adds additional info to the error case
        .with_context(|| format!("opening {}", input.display()))?;
    
    let mut archive = ZipArchive::new(input_file).context("reading EPUB archive")?;
    
    let output_file =
        File::create(output).with_context(|| format!("creating {}", output.display()))?;
    
    let mut writer = ZipWriter::new(output_file);
    
    for i in 0..archive.len() {
        // per file/directory loop
        
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("reading ZIP entry {i}"))?;
        
        let filepath = entry.name().to_owned();
        // filepath e.g. looks like "META-INF/calibre_bookmarks.txt"
        
        // filename is the filename without the path, e.g. "calibre_bookmarks.txt"
        let filename = Path::new(&filepath)
            .file_name()
            .and_then(|name| name.to_str());
        
        if entry.is_dir() {
            writer.add_directory(&filepath, SimpleFileOptions::default())?;
            println!("< dir >: {filepath}");
            continue;
        }
        
        if filename.is_some_and(|filename| DROP_FILES.contains(&filename)){
            // skip and dont write this file to output
            println!("< remove file >: {filepath}");
            continue;
        }
        
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;
        
        if FILE_EXTENSIONS.iter().any(|ext| filepath.ends_with(ext)){
            println!("< reformat >: {filepath}");
            let text = String::from_utf8(contents)?;
            let processed_text = apply_text_edits(&text);
            contents = processed_text.into_bytes();
        } else {
            println!("< no edit >: {filepath}");
        }
        
        // mimetype file in epub files usually is the first file of
        // the archive and indicates the media type of the archive,
        // this file is not being compressed
        let options = if filepath == "mimetype" {
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
        } else {
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
        };
        
        writer.start_file(&filepath, options)?;
        writer.write_all(&contents)?;
    }
    writer.finish()?;
    
    println!("created {}", output.display());
    
    Ok(())
}

fn apply_text_edits(text: &str) -> String {
    let applied_exclusions = exclude_unwanted_patterns(text, EXCLUDE_MATCHING_PATTERNS);
    let applied_spacers = ensure_surrounding_spacers(&applied_exclusions, SYMBOLS, PERMITTED_NEIGHBORS, APPLIED_SPACER);
    applied_spacers
}

fn exclude_unwanted_patterns(
    text: &str,
    patterns: &[&str],
) -> String {
    let mut edited_text = text.to_owned();
    for pattern in patterns {
        edited_text = Regex::new(pattern)
            .unwrap()
            .replace_all(&edited_text, "")
            .into_owned();
    }
    edited_text
}

fn ensure_surrounding_spacers(
    text: &str,
    symbols: &[&str],
    neighbors: &[&str],
    desired_spacer: &str,
) -> String {
    let mut edited_text = text.to_owned();
    
    let neighbors_matcher = neighbors
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");
    
    for symbol in symbols {
        // searches for the symbol and its surrounding spacers
        // overwrites the symbol with the the symbol and the desired spacers while keeping existing whitespace-characters if present
        
        // Single regex replacement step fails when "A— —B" because the space is being consumed on first pattern match and missing for the second match.
        
        // handle missing spacer before match
        let before_re = Regex::new(
            &format!(
                r"(?P<before>(?:{})*){}",
                neighbors_matcher,
                regex::escape(symbol)
            )
        ).unwrap();
        edited_text = before_re.replace_all(
            &edited_text,
            |caps: &regex::Captures| {
                // replacement is assembled here
                let before = &caps[1];
                format!(
                    // inserts spacers in the replacement only if the respective captures are empty
                    "{}{}",
                    if before.is_empty() {desired_spacer} else {before},
                    symbol
                )
            }
        ).into_owned();
        
        // handle missing spacer after match
        let behind_re = Regex::new(
            &format!(
                r"{}(?P<after>(?:{})*)",
                regex::escape(symbol),
                neighbors_matcher
            )
        ).unwrap();
        edited_text = behind_re.replace_all(
            &edited_text,
            |caps: &regex::Captures| {
                // replacement is assembled here
                let after = &caps[1];
                format!(
                    // inserts spacers in the replacement only if the respective captures are empty
                    "{}{}",
                    symbol,
                    if after.is_empty() {desired_spacer} else {after}
                )
            }
        ).into_owned();
    }
    edited_text
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TextCase {
        name: &'static str,
        input: &'static str,
        expected: &'static str,
    }
    const CASES: &[TextCase] = &[
        TextCase {
            name: "case1",
            input: "text…text, text—text, text–text, text-text",
            expected: "text … text, text — text, text – text, text-text"
        },
        TextCase {
            name: "case2",
            input: "hello —world, hello… world, hello – world, hello  –  world",
            expected: "hello — world, hello … world, hello – world, hello  –  world"
        },
        TextCase {
            name: "case3",
            input: "<div style=\"float: none; margin: 10px 0px 10px 0px; text-align: center;\"><p><a href=\"https://oceanofpdf.com\"><i>OceanofPDF.com</i></a></p></div></body></html>",
            expected: "</body></html>"
        },
        TextCase {
            name: "case4",
            input: "— —, A….…A, B—,—B",
            expected: " — —, A ….… A, B —,— B"
        },
        TextCase {
            name: "case5",
            input: "\n—\t",
            expected: "\n—\t"
        }
    ];
    
    #[test]
    fn text_edits_produce_expected_output() {
        for case in CASES {
            let result = apply_text_edits(case.input);
            assert_eq!(
                result,
                case.expected,
                "test case: {}",
                case.name
            );
        }
    }
    
    #[test]
    fn text_edits_are_idempotent() {
        for case in CASES {
            let once = apply_text_edits(case.input);
            let twice = apply_text_edits(&once);
            
            assert_eq!(
                once,
                twice,
                "test case: {}",
                case.name
            );
        }
    }
}
