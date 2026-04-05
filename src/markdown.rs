use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

/// Parse a markdown/HTML string into styled spans.
/// Handles: **bold**, *italic*, [links](url), <b>, <em>, <i>, <strong>, <a href="...">.
/// Strips other HTML tags.
pub fn parse_md(input: &str) -> Vec<StyledSegment> {
    let mut segments = Vec::new();
    let mut chars = input.chars().peekable();
    let mut buf = String::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            '<' => {
                if !buf.is_empty() {
                    segments.push(StyledSegment::Plain(std::mem::take(&mut buf)));
                }
                // Parse HTML tag
                chars.next(); // consume '<'
                let mut tag = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '>' { chars.next(); break; }
                    tag.push(c);
                    chars.next();
                }
                let tag_lower = tag.to_lowercase();
                if tag_lower == "b" || tag_lower == "strong" {
                    let content = consume_until_tag(&mut chars, &["b", "strong"]);
                    segments.push(StyledSegment::Bold(content));
                } else if tag_lower == "em" || tag_lower == "i" {
                    let content = consume_until_tag(&mut chars, &["em", "i"]);
                    segments.push(StyledSegment::Italic(content));
                } else if tag_lower.starts_with("a ") || tag_lower == "a" {
                    let content = consume_until_tag(&mut chars, &["a"]);
                    segments.push(StyledSegment::Link(content));
                }
                // Other tags: silently consumed (stripped)
            }
            '[' => {
                if !buf.is_empty() {
                    segments.push(StyledSegment::Plain(std::mem::take(&mut buf)));
                }
                chars.next(); // consume '['
                let mut link_text = String::new();
                let mut found_close = false;
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == ']' { found_close = true; break; }
                    link_text.push(c);
                }
                if found_close && chars.peek() == Some(&'(') {
                    chars.next(); // consume '('
                    // Skip URL
                    let mut depth = 1;
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c == '(' { depth += 1; }
                        if c == ')' { depth -= 1; if depth == 0 { break; } }
                    }
                    segments.push(StyledSegment::Link(link_text));
                } else {
                    // Not a real link, put it back as plain text
                    buf.push('[');
                    buf.push_str(&link_text);
                    if found_close { buf.push(']'); }
                }
            }
            '*' => {
                chars.next();
                if chars.peek() == Some(&'*') {
                    // **bold**
                    chars.next();
                    if !buf.is_empty() {
                        segments.push(StyledSegment::Plain(std::mem::take(&mut buf)));
                    }
                    let mut bold_text = String::new();
                    loop {
                        match chars.peek() {
                            Some(&'*') => {
                                chars.next();
                                if chars.peek() == Some(&'*') {
                                    chars.next();
                                    break;
                                }
                                bold_text.push('*');
                            }
                            Some(&c) => { chars.next(); bold_text.push(c); }
                            None => break,
                        }
                    }
                    segments.push(StyledSegment::Bold(bold_text));
                } else {
                    // *italic*
                    if !buf.is_empty() {
                        segments.push(StyledSegment::Plain(std::mem::take(&mut buf)));
                    }
                    let mut italic_text = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '*' { chars.next(); break; }
                        italic_text.push(c);
                        chars.next();
                    }
                    segments.push(StyledSegment::Italic(italic_text));
                }
            }
            _ => {
                buf.push(ch);
                chars.next();
            }
        }
    }

    if !buf.is_empty() {
        segments.push(StyledSegment::Plain(buf));
    }

    segments
}

fn consume_until_tag(chars: &mut std::iter::Peekable<std::str::Chars>, close_tags: &[&str]) -> String {
    let mut content = String::new();
    loop {
        match chars.peek() {
            None => break,
            Some(&'<') => {
                // Check if this is a closing tag we're looking for
                let mut lookahead = String::new();
                let mut temp = Vec::new();
                chars.next(); // '<'
                temp.push('<');
                while let Some(&c) = chars.peek() {
                    temp.push(c);
                    if c == '>' {
                        chars.next();
                        break;
                    }
                    lookahead.push(c);
                    chars.next();
                }
                let la_lower = lookahead.to_lowercase();
                let is_close = close_tags.iter().any(|t| la_lower == format!("/{}", t));
                if is_close {
                    break;
                }
                // Nested tag - recurse or strip
                let tag_lower = la_lower.trim_start_matches('/');
                if tag_lower == "b" || tag_lower == "strong" || tag_lower == "em" || tag_lower == "i" || tag_lower.starts_with("a") {
                    // Just include nested content as plain text
                    content.push_str(&la_lower);
                } else {
                    // Strip unknown tag, keep going
                }
            }
            Some(&c) => {
                content.push(c);
                chars.next();
            }
        }
    }
    content
}

#[derive(Debug, Clone)]
pub enum StyledSegment {
    Plain(String),
    Bold(String),
    Italic(String),
    Link(String), // displayed as underlined text
}

impl StyledSegment {
    pub fn text(&self) -> &str {
        match self {
            Self::Plain(t) | Self::Bold(t) | Self::Italic(t) | Self::Link(t) => t,
        }
    }
}

/// Convert segments to ratatui Spans
pub fn segments_to_spans(segments: &[StyledSegment]) -> Vec<Span<'static>> {
    segments.iter().map(|seg| match seg {
        StyledSegment::Plain(t) => Span::raw(t.clone()),
        StyledSegment::Bold(t) => Span::styled(t.clone(), Style::default().add_modifier(Modifier::BOLD)),
        StyledSegment::Italic(t) => Span::styled(t.clone(), Style::default().add_modifier(Modifier::ITALIC)),
        StyledSegment::Link(t) => Span::styled(t.clone(), Style::default().add_modifier(Modifier::UNDERLINED)),
    }).collect()
}

/// Flatten segments to plain text (for wrapping calculations)
pub fn segments_to_plain(segments: &[StyledSegment]) -> String {
    segments.iter().map(|s| s.text()).collect()
}

/// Parse markdown, wrap to width, and return Lines with inline styles preserved.
/// Each returned element is a Vec<StyledSegment> representing one wrapped line.
pub fn wrap_md(input: &str, width: usize) -> Vec<Vec<StyledSegment>> {
    let segments = parse_md(input);
    let plain = segments_to_plain(&segments);

    // Use textwrap to determine line break positions on the plain text
    let wrapped = textwrap::wrap(&plain, width);

    let mut result = Vec::new();
    let mut seg_iter = segments.iter();
    let mut current_seg: Option<&StyledSegment> = seg_iter.next();
    let mut seg_offset: usize = 0; // how far into current segment we've consumed

    for wrapped_line in &wrapped {
        let line_len = wrapped_line.len();
        let mut line_segments = Vec::new();
        let mut remaining = line_len;

        // Handle leading whitespace trimmed by textwrap
        // textwrap may trim leading spaces on continuation lines
        while remaining > 0 {
            let seg = match current_seg {
                Some(s) => s,
                None => break,
            };
            let seg_text = seg.text();
            let available = seg_text.len() - seg_offset;

            if available == 0 {
                current_seg = seg_iter.next();
                seg_offset = 0;
                continue;
            }

            let take = remaining.min(available);
            let chunk = &seg_text[seg_offset..seg_offset + take];

            let styled_chunk = match seg {
                StyledSegment::Plain(_) => StyledSegment::Plain(chunk.to_string()),
                StyledSegment::Bold(_) => StyledSegment::Bold(chunk.to_string()),
                StyledSegment::Italic(_) => StyledSegment::Italic(chunk.to_string()),
                StyledSegment::Link(_) => StyledSegment::Link(chunk.to_string()),
            };

            line_segments.push(styled_chunk);
            seg_offset += take;
            remaining -= take;

            if seg_offset >= seg_text.len() {
                current_seg = seg_iter.next();
                seg_offset = 0;
            }
        }

        // Skip whitespace at wrap boundary
        if let Some(seg) = current_seg {
            let seg_text = seg.text();
            while seg_offset < seg_text.len() && seg_text.as_bytes().get(seg_offset) == Some(&b' ') {
                seg_offset += 1;
            }
            if seg_offset >= seg_text.len() {
                current_seg = seg_iter.next();
                seg_offset = 0;
            }
        }

        result.push(line_segments);
    }

    result
}
