use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Flex, Layout, Rect, Spacing},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph,
        Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
    },
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

use crate::api::{EditionItem, HomepageSection};
use crate::app::{App, ArticleLine, LoadingState, View};
use crate::markdown;

const DIM: Color = Color::DarkGray;
const DOT_COLORS: &[Color] = &[
    Color::Yellow, Color::Green, Color::Red, Color::Blue, Color::Magenta, Color::Cyan,
];

fn dot_span<'a>(color: Color) -> Span<'a> {
    Span::styled("\u{25CF} ", Style::default().fg(color))
}

fn label_span<'a>(text: &str, color: Color) -> Span<'a> {
    Span::styled(format!(" {} ", text), Style::default().fg(Color::Black).bg(color))
}

// ── Main draw ──

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width == 0 || area.height < 5 { return; }

    let logo_fits = area.width >= 79;
    let header_h = if logo_fits { 4 } else { 2 };

    let [header, content, footer] = Layout::vertical([
        Constraint::Length(header_h), Constraint::Fill(1), Constraint::Length(1),
    ]).areas(area);

    // Header: ASCII logo if wide enough, fallback to text
    if logo_fits {
        let logo = include_str!("../ascii/ascii_logo_final.txt");
        let max_w = logo.lines().filter(|l| !l.is_empty()).map(|l| l.chars().count()).max().unwrap_or(0);
        let block_pad = (header.width as usize).saturating_sub(max_w) / 2;
        let prefix: String = " ".repeat(block_pad);
        let logo_lines: Vec<Line> = logo.lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                // Right-pad each line to max char width, then prepend centering prefix
                let char_w = l.chars().count();
                let right_pad = max_w.saturating_sub(char_w);
                Line::from(format!("{}{}{}", prefix, l, " ".repeat(right_pad)))
            })
            .collect();
        frame.render_widget(Paragraph::new(logo_lines), header);
    } else {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                "The ".dark_gray(),
                "European ".bold(),
                "Correspondent".bold(),
            ])).alignment(Alignment::Center),
            header,
        );
    }

    match &app.view {
        View::Home => draw_home(frame, app, content),
        View::Article => draw_article(frame, app, content),
        View::Search => draw_search(frame, app, content),
        View::EditionsList => draw_editions(frame, app, content),
    }

    let hints = match &app.view {
        View::Home => "\u{2191}\u{2193} Navigate  \u{23CE} Read  / Search  e Editions  q Quit",
        View::Article => "\u{2191}\u{2193} Scroll  Space Page  Esc Back  q Quit",
        View::Search => "\u{2191}\u{2193} Navigate  \u{23CE} Read  / Search  Esc Back",
        View::EditionsList => "\u{2191}\u{2193} Navigate  \u{23CE} Open  Esc Back",
    };
    frame.render_widget(
        Paragraph::new(hints.dark_gray()).alignment(Alignment::Center),
        footer,
    );

    if app.search_active { draw_search_input(frame, app, area); }
}

// ── Home: render into offscreen buffer, then splice visible region ──

fn draw_home(frame: &mut Frame, app: &mut App, area: Rect) {
    let homepage = match &app.homepage {
        LoadingState::Loading => { frame.render_widget(Paragraph::new("Loading...").centered().fg(DIM), area); return; }
        LoadingState::Error(e) => { frame.render_widget(Paragraph::new(format!("Error: {e}")).centered().fg(Color::Red), area); return; }
        LoadingState::Loaded(hp) => hp,
    };

    let w = area.width;
    let selected = app.home_view.selected;

    // Collect all items for index-based lookup (just references, no clone)
    let all_items: Vec<&EditionItem> = homepage.sections.iter().flat_map(|s| match s {
        HomepageSection::Hero { items } => items.iter().collect::<Vec<_>>(),
        HomepageSection::ItemList { items, .. } => items.iter().collect(),
        HomepageSection::Highlight { item } | HomepageSection::Inline { item } => vec![item],
    }).collect();

    // Phase 1: measure total height and track item offsets + heights
    let hero_h = (area.height * 63 / 100).max(20);
    let mut total_h: u16 = 1;
    let mut item_idx: usize = 0;
    let mut item_offsets: Vec<u16> = Vec::new();
    let mut item_heights: Vec<u16> = Vec::new();
    let mut section_layouts: Vec<SectionLayout> = Vec::new();
    let mut color_idx: usize = 0;

    for section in &homepage.sections {
        match section {
            HomepageSection::Hero { items } => {
                let y = total_h;
                let half = hero_h / 2;
                for (i, _) in items.iter().enumerate() {
                    let (offset, height) = if w >= 96 && items.len() >= 5 {
                        match i {
                            0 => (y, hero_h),          // center: full height
                            1 | 2 => (y, half),        // top row
                            3 | 4 => (y + half, hero_h - half), // bottom row
                            _ => (y, hero_h),
                        }
                    } else {
                        let tile_h = hero_h / items.len().max(1) as u16;
                        (y + (i as u16) * tile_h, tile_h)
                    };
                    item_offsets.push(offset);
                    item_heights.push(height);
                    item_idx += 1;
                }
                section_layouts.push(SectionLayout::Hero { y, height: hero_h, count: items.len() });
                total_h += hero_h + 1;
            }
            HomepageSection::ItemList { heading, subheading, header_color, items } => {
                let dc = header_color.accent_rgb()
                    .map(|(r,g,b)| Color::Rgb(r,g,b))
                    .unwrap_or(DOT_COLORS[color_idx % DOT_COLORS.len()]);
                color_idx += 1;

                let mut lines: Vec<Line> = vec![Line::from("")];
                if let Some(h) = heading {
                    lines.push(Line::from(vec![Span::raw("  "), dot_span(dc), h.to_uppercase().bold()]));
                    if let Some(sub) = subheading {
                        lines.push(Line::from(vec![Span::raw("    "), sub.clone().italic().dark_gray()]));
                    }
                    lines.push(Line::from(""));
                }
                for item in items {
                    item_offsets.push(total_h + lines.len() as u16);
                    item_heights.push(3); // compact: title + authors + blank
                    build_compact_item(item, false, &mut lines);
                    item_idx += 1;
                }
                lines.push(Line::from(""));

                let h = lines.len() as u16;
                section_layouts.push(SectionLayout::Text { y: total_h, lines });
                total_h += h;
            }
            HomepageSection::Highlight { item } => {
                let highlight_h = (area.height * 47 / 100).max(14);
                item_offsets.push(total_h);
                item_heights.push(highlight_h);
                section_layouts.push(SectionLayout::Highlight {
                    y: total_h,
                    height: highlight_h,
                    item_index: item_idx,
                    reversed: color_idx % 2 == 1, // alternate image side
                });
                item_idx += 1;
                total_h += highlight_h + 1;
            }
            HomepageSection::Inline { item } => {
                let mut lines = Vec::new();
                item_offsets.push(total_h);
                item_heights.push(2);
                build_compact_item(item, false, &mut lines);
                item_idx += 1;
                let h = lines.len() as u16;
                section_layouts.push(SectionLayout::Text { y: total_h, lines });
                total_h += h;
            }
        }
    }

    // Phase 2: update offsets + scroll
    app.home_view.item_offsets = item_offsets;
    app.home_view.item_heights = item_heights;
    app.home_view.ensure_visible(area.height);
    app.home_view.scroll = if total_h > area.height {
        app.home_view.scroll.min(total_h.saturating_sub(area.height))
    } else { 0 };
    let scroll = app.home_view.scroll;

    // Phase 3: rebuild offscreen buffer only when dirty or size changed
    let needs_rebuild = app.home_dirty || app.home_buffer.is_none()
        || app.home_buffer_size != (w, total_h);

    if needs_rebuild {
        let mut image_cache = app.image_cache.take();

        let buf_area = Rect { x: 0, y: 0, width: w, height: total_h };
        let mut offscreen = Buffer::empty(buf_area);

        let mut hero_item_offset: usize = 0;
        let mut tile_rects: Vec<(usize, Rect)> = Vec::new();
        for sl in &section_layouts {
            match sl {
                SectionLayout::Hero { y, height, count } => {
                    let hero_rect = Rect { x: 0, y: *y, width: w, height: *height };
                    render_hero_grid(
                        &mut offscreen, &mut image_cache, &all_items[hero_item_offset..hero_item_offset + count],
                        hero_rect, 999, hero_item_offset, &mut tile_rects,
                    );
                    hero_item_offset += count;
                }
                SectionLayout::Text { y, lines } => {
                    let rect = Rect { x: 0, y: *y, width: w, height: lines.len() as u16 };
                    Paragraph::new(lines.clone()).render(rect, &mut offscreen);
                }
                SectionLayout::Highlight { y, height, item_index, reversed } => {
                    if let Some(item) = all_items.get(*item_index) {
                        let rect = Rect { x: 0, y: *y, width: w, height: *height };
                        tile_rects.push((*item_index, rect));
                        render_highlight(
                            &mut offscreen, &mut image_cache, item, rect,
                            false, *reversed, // never focused in cached buffer
                        );
                    }
                }
            }
        }

        app.image_cache = image_cache;
        app.home_buffer = Some(offscreen);
        app.home_buffer_size = (w, total_h);
        app.home_tile_rects = tile_rects;
        app.home_dirty = false;
    }

    // Phase 4: splice visible region from cached buffer
    if let Some(offscreen) = &app.home_buffer {
        let frame_buf = frame.buffer_mut();
        for fy in 0..area.height {
            let src_y = fy + scroll;
            if src_y >= total_h { break; }
            for fx in 0..area.width.min(w) {
                frame_buf[(area.x + fx, area.y + fy)] = offscreen[(fx, src_y)].clone();
            }
        }
    }

    // Phase 5: draw selection highlight
    // For tiles (hero/highlight): brighten the existing dim border
    // For compact items: draw ▸ marker + bold
    if let Some(&sel_y) = app.home_view.item_offsets.get(selected) {
        let frame_buf = frame.buffer_mut();

        // Look up actual tile rect from stored layout data
        let tile_rect = app.home_tile_rects.iter()
            .find(|(idx, _)| *idx == selected)
            .map(|(_, r)| *r);

        if let Some(mut tr) = tile_rect {
            // Layout may include 1 extra cell from spacing rounding - clamp to content
            tr.width = tr.width.saturating_sub(1);
            // Draw rounded border characters into the space-border cells
            let border = ratatui::symbols::border::ROUNDED;
            let set_cell = |buf: &mut Buffer, vx: u16, vy: u16, sym: &str| {
                let sy = vy as i32 - scroll as i32;
                if sy >= 0 && (sy as u16) < area.height && vx < area.width {
                    if let Some(cell) = buf.cell_mut((area.x + vx, area.y + sy as u16)) {
                        cell.set_symbol(sym);
                        cell.set_style(Style::reset());
                    }
                }
            };
            // Corners
            set_cell(frame_buf, tr.x, tr.y, border.top_left);
            set_cell(frame_buf, tr.x + tr.width - 1, tr.y, border.top_right);
            set_cell(frame_buf, tr.x, tr.y + tr.height - 1, border.bottom_left);
            set_cell(frame_buf, tr.x + tr.width - 1, tr.y + tr.height - 1, border.bottom_right);
            // Horizontal
            for x in (tr.x + 1)..(tr.x + tr.width - 1) {
                set_cell(frame_buf, x, tr.y, border.horizontal_top);
                set_cell(frame_buf, x, tr.y + tr.height - 1, border.horizontal_bottom);
            }
            // Vertical
            for y in (tr.y + 1)..(tr.y + tr.height - 1) {
                set_cell(frame_buf, tr.x, y, border.vertical_left);
                set_cell(frame_buf, tr.x + tr.width - 1, y, border.vertical_right);
            }
        } else {
            // Compact item: ▸ marker + bold title
            let screen_y = sel_y as i32 - scroll as i32;
            if screen_y >= 0 && (screen_y as u16) < area.height {
                let fy = area.y + screen_y as u16;
                if let Some(cell) = frame_buf.cell_mut((area.x + 2, fy)) {
                    cell.set_symbol("\u{25B8}");
                }
                for fx in (area.x + 4)..(area.x + area.width) {
                    if let Some(cell) = frame_buf.cell_mut((fx, fy)) {
                        cell.set_style(cell.style().add_modifier(Modifier::BOLD));
                    }
                }
            }
        }
    }

    // Scrollbar
    if total_h > area.height {
        let mut sb = ScrollbarState::new(total_h as usize)
            .position(scroll as usize)
            .viewport_content_length(area.height as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::default().fg(DIM)),
            area, &mut sb,
        );
    }
}

enum SectionLayout {
    Hero { y: u16, height: u16, count: usize },
    Text { y: u16, lines: Vec<Line<'static>> },
    Highlight { y: u16, height: u16, item_index: usize, reversed: bool },
}

// ── Hero grid: rendered into a buffer using Layout ──

fn render_hero_grid(
    buf: &mut Buffer,
    image_cache: &mut Option<crate::images::ImageCache>,
    items: &[&EditionItem],
    area: Rect,
    selected: usize,
    base_idx: usize,
    tile_rects: &mut Vec<(usize, Rect)>,
) {
    if items.is_empty() || area.height < 4 { return; }

    let w = area.width;
    let gap = 2u16;

    if w >= 96 && items.len() >= 5 {
        // 3-col with Flex + Spacing
        let [left, center, right] = Layout::horizontal([
            Constraint::Ratio(2, 7), Constraint::Ratio(3, 7), Constraint::Ratio(2, 7),
        ])
        .flex(Flex::SpaceBetween)
        .spacing(Spacing::Space(gap))
        .areas(area);

        let [lt, lb] = Layout::vertical([Constraint::Fill(1); 2])
            .spacing(Spacing::Space(1))
            .areas(left);
        let [rt, rb] = Layout::vertical([Constraint::Fill(1); 2])
            .spacing(Spacing::Space(1))
            .areas(right);

        tile_rects.push((base_idx, center));
        tile_rects.push((base_idx + 1, lt));
        tile_rects.push((base_idx + 2, rt));
        if items.len() > 3 { tile_rects.push((base_idx + 3, lb)); }
        if items.len() > 4 { tile_rects.push((base_idx + 4, rb)); }

        render_tile_buf(buf, image_cache, items[0], center, base_idx == selected, true);
        render_tile_buf(buf, image_cache, items[1], lt, base_idx + 1 == selected, false);
        render_tile_buf(buf, image_cache, items[2], rt, base_idx + 2 == selected, false);
        if items.len() > 3 { render_tile_buf(buf, image_cache, items[3], lb, base_idx + 3 == selected, false); }
        if items.len() > 4 { render_tile_buf(buf, image_cache, items[4], rb, base_idx + 4 == selected, false); }

    } else if w >= 50 && items.len() >= 2 {
        let [left, right] = Layout::horizontal([Constraint::Ratio(4, 7), Constraint::Ratio(3, 7)])
            .spacing(Spacing::Space(gap))
            .areas(area);

        tile_rects.push((base_idx, left));
        render_tile_buf(buf, image_cache, items[0], left, base_idx == selected, true);

        let n = (items.len() - 1).min(4);
        let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Fill(1)).collect();
        let rows = Layout::vertical(constraints).spacing(Spacing::Space(1)).split(right);
        for (i, r) in rows.iter().enumerate() {
            tile_rects.push((base_idx + i + 1, *r));
            render_tile_buf(buf, image_cache, items[i + 1], *r, base_idx + i + 1 == selected, false);
        }
    } else {
        let constraints: Vec<Constraint> = items.iter().map(|_| Constraint::Fill(1)).collect();
        let rows = Layout::vertical(constraints).spacing(Spacing::Space(1)).split(area);
        for (i, r) in rows.iter().enumerate() {
            tile_rects.push((base_idx + i, *r));
            render_tile_buf(buf, image_cache, items[i], *r, base_idx + i == selected, i == 0);
        }
    }
}

/// Render a hero tile into a Buffer.
/// Always bordered (border style changes on focus, not presence → image doesn't resize).
/// Primary items show teaser text below the title.
/// Images use Crop to fill the space (like CSS object-cover).
fn render_tile_buf(buf: &mut Buffer, image_cache: &mut Option<crate::images::ImageCache>, item: &EditionItem, area: Rect, focused: bool, primary: bool) {
    if area.width < 4 || area.height < 3 { return; }

    let title = item.title.as_deref().unwrap_or("Untitled");

    // Always reserve border space for consistent sizing.
    // Space-border when not focused (invisible), overlay will draw real border when selected.
    let block = Block::bordered()
        .border_set(ratatui::symbols::border::Set {
            top_left: " ", top_right: " ", bottom_left: " ", bottom_right: " ",
            horizontal_top: " ", horizontal_bottom: " ",
            vertical_left: " ", vertical_right: " ",
        });

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.width < 2 || inner.height < 2 { return; }

    // Layout inside the block: image on top, text below
    let title_lines = textwrap::wrap(title, inner.width.saturating_sub(2) as usize);
    let title_h = (title_lines.len() as u16).min(2);

    // Primary: title + teaser; secondary: just title
    let teaser = if primary {
        item.content.teaser().map(|t| {
            markdown::segments_to_plain(&markdown::parse_md(t))
        })
    } else { None };
    let teaser_h = if let Some(ref t) = teaser {
        let wrapped = textwrap::wrap(t, inner.width.saturating_sub(2) as usize);
        (wrapped.len() as u16).min(3)
    } else { 0 };

    let text_h = title_h + teaser_h;
    let [img_area, text_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(text_h),
    ]).areas(inner);

    // Image: object-fit cover (pre-resized to fill, then rendered to fit exactly)
    let rendered = item.content.image_url().map_or(false, |url| {
        image_cache.as_mut().map_or(false, |cache| {
            cache.get_cover(url, img_area.width, img_area.height).map_or(false, |proto| {
                StatefulImage::new()
                    .resize(Resize::Fit(None))
                    .render(img_area, buf, proto);
                true
            })
        })
    });

    // Fallback: colored placeholder
    if !rendered {
        let color = item.content.placeholder_color()
            .map(|(r,g,b)| Color::Rgb(r,g,b))
            .unwrap_or(DIM);
        let row: String = "\u{2584}".repeat(img_area.width as usize);
        let lines: Vec<Line> = (0..img_area.height)
            .map(|_| Line::from(row.clone().fg(color)))
            .collect();
        Paragraph::new(lines).render(img_area, buf);
    }

    // Title text
    let mut text_lines: Vec<Line> = Vec::new();
    for l in &title_lines {
        text_lines.push(Line::from(format!(" {l}").bold()));
    }

    // Teaser (primary only, like the web component)
    if let Some(ref t) = teaser {
        let wrapped = textwrap::wrap(t, inner.width.saturating_sub(2) as usize);
        for l in wrapped.iter().take(3) {
            text_lines.push(Line::from(format!(" {l}").dark_gray()));
        }
    }

    Paragraph::new(text_lines).render(text_area, buf);
}

// ── Highlight section: colored bg, image left, text right ──

fn render_highlight(
    buf: &mut Buffer,
    image_cache: &mut Option<crate::images::ImageCache>,
    item: &EditionItem,
    area: Rect,
    focused: bool,
    reversed: bool,
) {
    if area.width < 10 || area.height < 4 { return; }

    // Background: cardColor.light (the warm pastel colors like the website)
    // Text: titleColor.light (dark text on light bg, like the website's light mode)
    let bg_color = item.content.card_color()
        .and_then(|c| c.light)  // use light variant - the warm pastel
        .map(|(r,g,b)| Color::Rgb(r,g,b));
    let fg_color = match &item.content {
        crate::api::ItemContent::Longform { title_color, .. } => {
            title_color.light.map(|(r,g,b)| Color::Rgb(r,g,b))
        }
        _ => item.content.header_color().and_then(|c| c.light).map(|(r,g,b)| Color::Rgb(r,g,b)),
    };

    // Fill background
    if let Some(bg) = bg_color {
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg);
                }
            }
        }
    }

    // Outer block for focus border
    let block = if focused {
        Block::bordered().border_type(BorderType::Rounded)
    } else {
        Block::bordered().border_set(ratatui::symbols::border::Set {
            top_left: " ", top_right: " ", bottom_left: " ", bottom_right: " ",
            horizontal_top: " ", horizontal_bottom: " ",
            vertical_left: " ", vertical_right: " ",
        })
    };
    if let Some(bg) = bg_color {
        block.style(Style::default().bg(bg)).render(area, buf);
    } else {
        block.render(area, buf);
    }
    let inner = Rect {
        x: area.x + 1, y: area.y + 1,
        width: area.width.saturating_sub(2), height: area.height.saturating_sub(2),
    };

    // 2-column layout: image | text (or reversed)
    let [col_a, col_b] = Layout::horizontal([
        Constraint::Ratio(1, 2), Constraint::Ratio(1, 2),
    ])
    .spacing(Spacing::Space(2))
    .areas(inner);

    let (img_col, text_col) = if reversed { (col_b, col_a) } else { (col_a, col_b) };

    // Image
    let rendered = item.content.image_url().map_or(false, |url| {
        image_cache.as_mut().map_or(false, |cache| {
            cache.get_cover(url, img_col.width, img_col.height).map_or(false, |proto| {
                StatefulImage::new().resize(Resize::Fit(None)).render(img_col, buf, proto);
                true
            })
        })
    });

    if !rendered {
        let color = item.content.placeholder_color()
            .map(|(r,g,b)| Color::Rgb(r,g,b))
            .unwrap_or(DIM);
        let row: String = "\u{2584}".repeat(img_col.width as usize);
        let lines: Vec<Line> = (0..img_col.height)
            .map(|_| Line::from(row.clone().fg(color)))
            .collect();
        Paragraph::new(lines).render(img_col, buf);
    }

    // Text: title + teaser, vertically centered
    let title = item.title.as_deref().unwrap_or("Untitled");
    let wrap_w = text_col.width.saturating_sub(4) as usize;
    let title_wrapped = textwrap::wrap(title, wrap_w);

    let teaser = item.content.teaser().map(|t| {
        markdown::segments_to_plain(&markdown::parse_md(t))
    });
    let teaser_wrapped: Vec<String> = teaser.as_ref().map(|t| {
        textwrap::wrap(&t.chars().take(200).collect::<String>(), wrap_w)
            .iter().take(3).map(|l| l.to_string()).collect()
    }).unwrap_or_default();

    let text_h = title_wrapped.len() + 1 + teaser_wrapped.len();
    let pad_top = text_col.height.saturating_sub(text_h as u16) / 2;

    let text_area = Rect {
        x: text_col.x + 2,
        y: text_col.y + pad_top,
        width: text_col.width.saturating_sub(4),
        height: text_col.height.saturating_sub(pad_top),
    };

    let mut lines: Vec<Line> = Vec::new();
    let title_style = Style::default().add_modifier(Modifier::BOLD)
        .fg(fg_color.unwrap_or(Color::Reset));
    for l in &title_wrapped {
        lines.push(Line::from(Span::styled(l.to_string(), title_style)).centered());
    }
    lines.push(Line::from(""));
    let teaser_style = Style::default().fg(fg_color.unwrap_or(DIM));
    for l in &teaser_wrapped {
        lines.push(Line::from(Span::styled(l.to_string(), teaser_style)).centered());
    }

    Paragraph::new(lines).render(text_area, buf);
}

// ── Compact/hero item builders ──

fn build_hero_item(item: &EditionItem, width: usize, selected: bool, lines: &mut Vec<Line<'static>>) {
    let wrap = width.saturating_sub(8).min(90);
    let hdr_color = item.content.header_color()
        .and_then(|c| c.accent_rgb())
        .map(|(r,g,b)| Color::Rgb(r,g,b))
        .unwrap_or(DIM);
    let header_text = item.content.header().unwrap_or(item.content.type_label());
    let title = item.title.as_deref().unwrap_or("Untitled");
    let teaser = markdown::segments_to_plain(&markdown::parse_md(
        item.content.teaser().unwrap_or(&item.preview_text),
    ));
    let sel = if selected { "\u{25B8} " } else { "  " };

    lines.push(Line::from(vec![
        Span::raw("  "), Span::raw(sel), dot_span(hdr_color),
        Span::styled(header_text.to_string(), Style::default().fg(hdr_color)),
    ]));
    for l in textwrap::wrap(title, wrap) {
        lines.push(Line::from(vec![Span::raw("    "), l.to_string().bold()]));
    }
    if let Some((label, lc)) = item.content.label_info() {
        let c = lc.light.map(|(r,g,b)| Color::Rgb(r,g,b)).unwrap_or(Color::Red);
        lines.push(Line::from(vec![Span::raw("    "), label_span(label, c)]));
    }
    let authors = item.content.authors();
    if !authors.is_empty() {
        lines.push(Line::from(vec![Span::raw("    "), authors.join(", ").italic()]));
    }
    if !teaser.is_empty() {
        for l in textwrap::wrap(&teaser.chars().take(250).collect::<String>(), wrap) {
            lines.push(Line::from(vec![Span::raw("    "), l.to_string().dark_gray()]));
        }
    }
}

fn build_compact_item(item: &EditionItem, selected: bool, lines: &mut Vec<Line<'static>>) {
    let sel = if selected { "\u{25B8} " } else { "  " };
    let bold = if selected { Modifier::BOLD } else { Modifier::empty() };

    let mut spans: Vec<Span> = vec![Span::raw("  "), Span::raw(sel)];
    if let Some((label, lc)) = item.content.label_info() {
        let c = lc.light.map(|(r,g,b)| Color::Rgb(r,g,b)).unwrap_or(Color::Red);
        spans.push(label_span(label, c));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(
        item.title.as_deref().unwrap_or("Untitled").to_string(),
        Style::default().add_modifier(bold),
    ));
    lines.push(Line::from(spans));

    let authors = item.content.authors();
    if !authors.is_empty() {
        lines.push(Line::from(vec![Span::raw("      "), authors.join(", ").italic().dark_gray()]));
    }
    lines.push(Line::from(""));
}

// ── Article ──

fn draw_article(frame: &mut Frame, app: &mut App, area: Rect) {
    match &app.article {
        LoadingState::Loading => { frame.render_widget(Paragraph::new("Loading...").centered().fg(DIM), area); return; }
        LoadingState::Error(e) => { frame.render_widget(Paragraph::new(format!("Error: {e}")).centered().fg(Color::Red), area); return; }
        LoadingState::Loaded(_) => {}
    }

    // Rebuild lines if width changed
    if app.article_built_width != area.width {
        app.build_article_lines(area.width);
        app.article_dirty = true;
    }

    let [_, content, _] = Layout::horizontal([
        Constraint::Fill(1), Constraint::Max(90), Constraint::Fill(1),
    ]).areas(area);

    // Rebuild offscreen buffer when dirty (same pattern as home page)
    if app.article_dirty {
        let mut lines: Vec<Line> = Vec::new();
        let mut image_placements: Vec<(u16, String, u16)> = Vec::new();
        let img_w = content.width.saturating_sub(4);

        for al in &app.article_lines {
            match al {
                ArticleLine::Title(t) => lines.push(Line::from(format!("  {t}").bold())),
                ArticleLine::Header(t, color) => {
                    let c = color.map(|(r,g,b)| Color::Rgb(r,g,b)).unwrap_or(DIM);
                    lines.push(Line::from(vec![Span::raw("  "), dot_span(c), t.to_string().bold()]));
                }
                ArticleLine::Author(t) => lines.push(Line::from(format!("  {t}").italic())),
                ArticleLine::Meta(t) => lines.push(Line::from(format!("  {t}").dark_gray())),
                ArticleLine::Heading(t) => lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(t.to_string(), Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
                ])),
                ArticleLine::RichText(segs) => {
                    let mut spans = vec![Span::raw("  ")];
                    spans.extend(markdown::segments_to_spans(segs));
                    lines.push(Line::from(spans));
                }
                ArticleLine::InlineImage { url, height } => {
                    let mut h = *height;
                    // Resolve from actual image if height unknown
                    if h == 0 {
                        if let Some(cache) = &app.image_cache {
                            if let Some(cached) = cache.images.get(url.as_str()) {
                                let (sw, sh) = (cached.source.width(), cached.source.height());
                                if sw > 0 {
                                    h = ((img_w as f64 * sh as f64 / sw as f64) / 2.0).round() as u16;
                                    h = h.max(3).min(50);
                                }
                            }
                        }
                        if h == 0 { h = 25; }
                    }
                    image_placements.push((lines.len() as u16, url.clone(), h));
                    for _ in 0..h { lines.push(Line::from("")); }
                }
                ArticleLine::ImageCaption(t) => lines.push(Line::from(format!("  {t}").italic().dark_gray())),
                ArticleLine::Blank => lines.push(Line::from("")),
            }
        }

        let total_h = lines.len() as u16;
        let buf_area = Rect { x: 0, y: 0, width: content.width, height: total_h.max(1) };
        let mut offscreen = Buffer::empty(buf_area);

        // Render text into offscreen buffer
        Paragraph::new(lines).render(buf_area, &mut offscreen);

        // Render images into offscreen buffer (same as home page tiles)
        let mut image_cache = app.image_cache.take();
        for (line_y, url, img_h) in &image_placements {
            let img_rect = Rect { x: 2, y: *line_y, width: img_w, height: *img_h };
            if let Some(cache) = &mut image_cache {
                if !cache.images.contains_key(url.as_str()) {
                    cache.fetch(url);
                }
                // Use get_cover with halfblocks font (1,2) for correct aspect ratio
                if let Some(proto) = cache.get_cover(url, img_w, *img_h) {
                    StatefulImage::new().resize(Resize::Fit(None)).render(img_rect, &mut offscreen, proto);
                }
            }
        }
        app.image_cache = image_cache;

        app.article_buffer = Some(offscreen);
        app.article_dirty = false;
    }

    // Splice visible region from cached buffer (same as home page)
    let scroll = app.article_scroll;
    let total;
    if let Some(offscreen) = &app.article_buffer {
        total = offscreen.area.height as usize;
        let frame_buf = frame.buffer_mut();
        for fy in 0..content.height {
            let src_y = fy + scroll;
            if src_y >= offscreen.area.height { break; }
            for fx in 0..content.width.min(offscreen.area.width) {
                frame_buf[(content.x + fx, content.y + fy)] = offscreen[(fx, src_y)].clone();
            }
        }
    } else {
        total = 0;
    }

    if total > area.height as usize {
        let mut sb = ScrollbarState::new(total)
            .position(scroll as usize)
            .viewport_content_length(area.height as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::default().fg(DIM)),
            area, &mut sb,
        );
    }
}

// ── Search ──

fn draw_search(frame: &mut Frame, app: &mut App, area: Rect) {
    let [header, list] = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

    frame.render_widget(Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![Span::raw("  "), dot_span(Color::Blue), "SEARCH  ".bold(), format!("\"{}\"", app.search_query).dark_gray()]),
    ]), header);

    match &app.search_results {
        LoadingState::Loading => { frame.render_widget(Paragraph::new("  Searching...").fg(DIM), list); }
        LoadingState::Error(e) => { frame.render_widget(Paragraph::new(format!("  Error: {e}")).fg(Color::Red), list); }
        LoadingState::Loaded(results) => {
            if results.is_empty() { frame.render_widget(Paragraph::new("  No results found").fg(DIM), list); return; }
            let sel = app.search_view.selected;
            let mut lines: Vec<Line> = Vec::new();
            app.search_view.item_offsets.clear();
            for (i, item) in results.iter().enumerate() {
                app.search_view.item_offsets.push(lines.len() as u16);
                build_compact_item(item, i == sel, &mut lines);
            }
            app.search_view.item_count = results.len();
            app.search_view.ensure_visible(list.height);
            let total = lines.len();
            frame.render_widget(Paragraph::new(lines).scroll((app.search_view.scroll, 0)), list);
            if total > list.height as usize {
                let mut sb = ScrollbarState::new(total).position(app.search_view.scroll as usize).viewport_content_length(list.height as usize);
                frame.render_stateful_widget(Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::default().fg(DIM)), list, &mut sb);
            }
        }
    }
}

// ── Editions ──

fn draw_editions(frame: &mut Frame, app: &mut App, area: Rect) {
    match &app.editions {
        LoadingState::Loading => { frame.render_widget(Paragraph::new("Loading...").centered().fg(DIM), area); return; }
        LoadingState::Error(e) => { frame.render_widget(Paragraph::new(format!("Error: {e}")).centered().fg(Color::Red), area); return; }
        LoadingState::Loaded(_) => {}
    }

    let [header, list] = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

    frame.render_widget(Paragraph::new(vec![
        Line::from(""), Line::from(vec![Span::raw("  "), dot_span(Color::Yellow), "EDITIONS".bold()]),
    ]), header);

    let editions = match &app.editions { LoadingState::Loaded(e) => e, _ => return };
    let sel = app.editions_view.selected;
    let mut lines: Vec<Line> = Vec::new();
    app.editions_view.item_offsets.clear();
    for (i, ed) in editions.iter().enumerate() {
        let is_sel = i == sel;
        app.editions_view.item_offsets.push(lines.len() as u16);
        let marker = if is_sel { "\u{25B8} " } else { "  " };
        let bold = if is_sel { Modifier::BOLD } else { Modifier::empty() };
        lines.push(Line::from(vec![Span::raw("  "), Span::raw(marker), Span::styled(&ed.title, Style::default().add_modifier(bold))]));
        lines.push(Line::from(vec![Span::raw("      "), ed.date.clone().dark_gray(), format!("  \u{00B7}  {} articles", ed.items.len()).dark_gray()]));
    }
    app.editions_view.item_count = editions.len();
    app.editions_view.ensure_visible(list.height);
    let total = lines.len();
    frame.render_widget(Paragraph::new(lines).scroll((app.editions_view.scroll, 0)), list);
    if total > list.height as usize {
        let mut sb = ScrollbarState::new(total).position(app.editions_view.scroll as usize).viewport_content_length(list.height as usize);
        frame.render_stateful_widget(Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::default().fg(DIM)), list, &mut sb);
    }
}

// ── Search popup ──

fn draw_search_input(frame: &mut Frame, app: &App, area: Rect) {
    let w = 50.min(area.width.saturating_sub(4));
    let popup = Rect { x: (area.width - w) / 2, y: area.height / 3, width: w, height: 3 };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(&app.search_query), "\u{2588}".dark_gray()]))
            .block(Block::bordered().title(" Search ").border_style(Style::default().fg(DIM))),
        popup,
    );
}
