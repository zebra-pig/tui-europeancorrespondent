use chrono::NaiveDate;
use crate::api::{ContentBlock, Edition, EditionItem, Homepage, HomepageSection, ItemContent};
use crate::images::ImageCache;
use crate::markdown::{self, StyledSegment};

pub enum View {
    Home,
    Article,
    Search,
    DatePicker,
    EditionView,
}

pub enum LoadingState<T> {
    Loading,
    Loaded(T),
    Error(String),
}

pub struct ScrollView {
    pub scroll: u16,
    pub selected: usize,
    pub item_count: usize,
    pub item_offsets: Vec<u16>,
    pub item_heights: Vec<u16>,
}

impl ScrollView {
    pub fn new() -> Self {
        Self { scroll: 0, selected: 0, item_count: 0, item_offsets: Vec::new(), item_heights: Vec::new() }
    }

    pub fn next(&mut self) {
        if self.item_count > 0 {
            self.selected = (self.selected + 1).min(self.item_count - 1);
        }
    }

    pub fn prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn ensure_visible(&mut self, viewport_h: u16) {
        if let Some(&top) = self.item_offsets.get(self.selected) {
            let height = self.item_heights.get(self.selected).copied().unwrap_or(2);
            let bottom = top + height;
            if top < self.scroll {
                self.scroll = top.saturating_sub(1);
            } else if bottom > self.scroll + viewport_h {
                if height >= viewport_h {
                    self.scroll = top.saturating_sub(1);
                } else {
                    self.scroll = bottom.saturating_sub(viewport_h);
                }
            }
        }
    }
}

pub struct App {
    pub locale: String,
    pub view: View,
    pub should_quit: bool,

    pub homepage: LoadingState<Homepage>,
    pub home_slugs: Vec<Option<String>>,
    pub home_view: ScrollView,
    pub home_buffer: Option<ratatui::buffer::Buffer>,
    pub home_buffer_size: (u16, u16),
    pub home_dirty: bool,
    /// Tile rects in virtual coordinates (for selection overlay)
    pub home_tile_rects: Vec<(usize, ratatui::layout::Rect)>, // (item_index, rect)

    pub article: LoadingState<EditionItem>,
    pub article_scroll: u16,
    pub article_lines: Vec<ArticleLine>,
    pub article_built_width: u16,
    pub article_buffer: Option<ratatui::buffer::Buffer>,
    pub article_dirty: bool,

    pub search_query: String,
    pub search_results: LoadingState<Vec<EditionItem>>,
    pub search_view: ScrollView,
    pub search_active: bool,

    pub editions: LoadingState<Vec<Edition>>,
    pub editions_view: ScrollView,

    // Date picker
    pub picker_date: NaiveDate,
    // Edition viewer
    pub edition: LoadingState<Option<Edition>>,
    pub edition_view: ScrollView,

    pub image_cache: Option<ImageCache>,
    pub cell_aspect: f64,
}

#[derive(Clone)]
pub enum ArticleLine {
    Title(String),
    Header(String, Option<(u8, u8, u8)>),
    Author(String),
    Meta(String),
    Heading(String),
    RichText(Vec<StyledSegment>),
    InlineImage { url: String, height: u16 },
    ImageCaption(String),
    Blank,
}

impl App {
    pub fn new() -> Self {
        Self {
            locale: "en-GB".to_string(),
            view: View::Home,
            should_quit: false,
            homepage: LoadingState::Loading,
            home_slugs: Vec::new(),
            home_view: ScrollView::new(),
            home_buffer: None,
            home_buffer_size: (0, 0),
            home_dirty: true,
            home_tile_rects: Vec::new(),
            article: LoadingState::Loading,
            article_scroll: 0,
            article_lines: Vec::new(),
            article_built_width: 0,
            article_buffer: None,
            article_dirty: true,
            search_query: String::new(),
            search_results: LoadingState::Loaded(Vec::new()),
            search_view: ScrollView::new(),
            search_active: false,
            editions: LoadingState::Loading,
            editions_view: ScrollView::new(),
            picker_date: chrono::Local::now().date_naive(),
            edition: LoadingState::Loading,
            edition_view: ScrollView::new(),
            image_cache: None,
            cell_aspect: 0.5,
        }
    }

    pub fn rebuild_home_items(&mut self) {
        let hp = match &self.homepage {
            LoadingState::Loaded(hp) => hp,
            _ => return,
        };
        self.home_slugs.clear();
        for section in &hp.sections {
            match section {
                HomepageSection::Hero { items } => {
                    for item in items { self.home_slugs.push(item.slug.clone()); }
                }
                HomepageSection::ItemList { items, .. } => {
                    for item in items { self.home_slugs.push(item.slug.clone()); }
                }
                HomepageSection::Highlight { item } | HomepageSection::Inline { item } => {
                    self.home_slugs.push(item.slug.clone());
                }
            }
        }
        self.home_view.item_count = self.home_slugs.len();
        self.home_view.selected = 0;
        self.home_view.scroll = 0;
    }

    pub fn selected_slug(&self) -> Option<String> {
        self.home_slugs.get(self.home_view.selected)?.clone()
    }

    pub fn build_article_lines(&mut self, width: u16) {
        let item = match &self.article {
            LoadingState::Loaded(item) => item,
            _ => return,
        };

        let content_width = (width as usize).min(90);
        let wrap_width = content_width.saturating_sub(4).max(20);
        self.article_built_width = width;
        let cell_aspect = self.cell_aspect;
        let mut lines = Vec::new();

        let hdr_rgb = item.content.header_color().and_then(|c| c.accent_rgb());

        // Dot badge + header FIRST (like website's DotBadge above the title)
        if let Some(header) = item.content.header() {
            lines.push(ArticleLine::Header(header.to_string(), hdr_rgb));
            lines.push(ArticleLine::Blank);
        }

        // Title (skip for DataVis - they show only the dot badge header)
        let is_datavis = matches!(&item.content, ItemContent::DataVis { .. });
        if !is_datavis {
            if let Some(title) = &item.title {
                for line in textwrap::wrap(title, wrap_width) {
                    lines.push(ArticleLine::Title(line.to_string()));
                }
                lines.push(ArticleLine::Blank);
            }
        }

        // Formatted date (e.g. "25 March 2026")
        let formatted_date = format_date(&item.date);
        lines.push(ArticleLine::Meta(formatted_date));
        lines.push(ArticleLine::Blank);

        // Authors
        let authors = item.content.authors();
        if !authors.is_empty() {
            lines.push(ArticleLine::Author(format!("By {}", authors.join(", "))));
            lines.push(ArticleLine::Blank);
        }

        match &item.content {
            ItemContent::Longform { teaser, introduction_comment, body, comment, .. } => {
                // Header already rendered above title
                if !introduction_comment.is_empty() {
                    render_content_blocks(introduction_comment, wrap_width, cell_aspect, &mut lines);
                }
                if let Some(t) = teaser {
                    for segs in markdown::wrap_md(t, wrap_width) {
                        lines.push(ArticleLine::RichText(segs));
                    }
                    lines.push(ArticleLine::Blank);
                }
                render_content_blocks(body, wrap_width, cell_aspect, &mut lines);
                if !comment.is_empty() {
                    lines.push(ArticleLine::Blank);
                    lines.push(ArticleLine::Meta("\u{2500}\u{2500}\u{2500}".to_string()));
                    lines.push(ArticleLine::Blank);
                    render_content_blocks(comment, wrap_width, cell_aspect, &mut lines);
                }
            }
            ItemContent::Feature { label, introduction_comment, lead, comment, .. } => {
                // Header already rendered above title
                if let Some(l) = label {
                    lines.push(ArticleLine::Meta(l.clone()));
                    lines.push(ArticleLine::Blank);
                }
                if !introduction_comment.is_empty() {
                    render_content_blocks(introduction_comment, wrap_width, cell_aspect, &mut lines);
                }
                render_content_blocks(lead, wrap_width, cell_aspect, &mut lines);
                if !comment.is_empty() {
                    lines.push(ArticleLine::Blank);
                    lines.push(ArticleLine::Meta("\u{2500}\u{2500}\u{2500}".to_string()));
                    lines.push(ArticleLine::Blank);
                    render_content_blocks(comment, wrap_width, cell_aspect, &mut lines);
                }
            }
            ItemContent::DataVis { description, image_url, image_width, image_height, .. } => {
                // Header already rendered above title
                render_content_blocks(description, wrap_width, cell_aspect, &mut lines);
                if let Some(url) = image_url {
                    let mut img_h = compute_image_height(wrap_width, *image_width, *image_height);
                    if img_h == 0 {
                        // Unknown dimensions - use a generous default, will be corrected at render
                        img_h = 25;
                    }
                    lines.push(ArticleLine::InlineImage { url: url.clone(), height: img_h });
                    lines.push(ArticleLine::Blank);
                }
            }
            ItemContent::CulturalRec { description, .. } => {
                // Header already rendered above title
                render_content_blocks(description, wrap_width, cell_aspect, &mut lines);
            }
            ItemContent::EditorsNote { body, .. } => {
                render_content_blocks(body, wrap_width, cell_aspect, &mut lines);
            }
            ItemContent::CommunityNote { title, label, signature, description_top, description_bottom, .. } => {
                for line in textwrap::wrap(title, wrap_width) {
                    lines.push(ArticleLine::Header(line.to_string(), hdr_rgb));
                }
                lines.push(ArticleLine::Blank);
                if let Some(l) = label {
                    lines.push(ArticleLine::Meta(l.clone()));
                    lines.push(ArticleLine::Blank);
                }
                render_content_blocks(description_top, wrap_width, cell_aspect, &mut lines);
                render_content_blocks(description_bottom, wrap_width, cell_aspect, &mut lines);
                if let Some(sig) = signature {
                    lines.push(ArticleLine::Blank);
                    lines.push(ArticleLine::Author(sig.clone()));
                }
            }
            ItemContent::Advert { .. } => {
                lines.push(ArticleLine::Meta("[Advertisement]".to_string()));
            }
        }

        self.article_lines = lines;
    }
}

/// Compute display height in terminal lines.
/// For halfblocks: each cell is 1px wide and 2px tall,
/// so lines = width_cells * (img_h / img_w) / 2.
/// If dimensions unknown, returns 0 (caller should compute from actual image later).
fn compute_image_height(display_width_chars: usize, img_w: Option<i64>, img_h: Option<i64>) -> u16 {
    match (img_w, img_h) {
        (Some(w), Some(h)) if w > 0 => {
            let lines = (display_width_chars as f64 * (h as f64 / w as f64) / 2.0).round() as u16;
            lines.max(3).min(50)
        }
        _ => 0, // unknown - will be resolved from actual image data
    }
}

fn render_content_blocks(blocks: &[ContentBlock], wrap_width: usize, _cell_aspect: f64, lines: &mut Vec<ArticleLine>) {
    for block in blocks {
        match block {
            ContentBlock::Heading(text) => {
                let cleaned = strip_html(text);
                lines.push(ArticleLine::Blank);
                for line in textwrap::wrap(&cleaned, wrap_width) {
                    lines.push(ArticleLine::Heading(line.to_string()));
                }
                lines.push(ArticleLine::Blank);
            }
            ContentBlock::Paragraph(text) => {
                let wrapped_lines = markdown::wrap_md(text, wrap_width);
                for segs in wrapped_lines {
                    lines.push(ArticleLine::RichText(segs));
                }
                lines.push(ArticleLine::Blank);
            }
            ContentBlock::Image { url, width: img_w, height: img_h, caption, alt } => {
                if let Some(u) = url {
                    let mut display_h = compute_image_height(wrap_width, *img_w, *img_h);
                    if display_h == 0 { display_h = 25; }
                    lines.push(ArticleLine::InlineImage { url: u.clone(), height: display_h });
                }
                if let Some(cap) = caption {
                    let cleaned = strip_html(cap);
                    if !cleaned.is_empty() {
                        lines.push(ArticleLine::ImageCaption(cleaned));
                    }
                } else if let Some(a) = alt {
                    if !a.is_empty() {
                        lines.push(ArticleLine::ImageCaption(a.clone()));
                    }
                }
                lines.push(ArticleLine::Blank);
            }
        }
    }
}

/// Format ISO date "2026-03-25" as "25 March 2026"
fn format_date(date_str: &str) -> String {
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        date.format("%d %B %Y").to_string()
    } else {
        date_str.to_string()
    }
}

fn strip_html(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}
