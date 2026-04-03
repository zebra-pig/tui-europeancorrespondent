use crate::api::{ContentBlock, Edition, EditionItem, Homepage, HomepageSection, ItemContent};
use crate::images::ImageCache;
use crate::markdown::{self, StyledSegment};

pub enum View {
    Home,
    Article,
    Search,
    EditionsList,
}

pub enum LoadingState<T> {
    Loading,
    Loaded(T),
    Error(String),
}

/// Tracks a scrollable, selectable view
pub struct ScrollView {
    pub scroll: u16,
    pub selected: usize,
    pub item_count: usize,
    /// Line offset of each selectable item in the content
    pub item_offsets: Vec<u16>,
}

impl ScrollView {
    pub fn new() -> Self {
        Self { scroll: 0, selected: 0, item_count: 0, item_offsets: Vec::new() }
    }

    pub fn next(&mut self) {
        if self.item_count > 0 {
            self.selected = (self.selected + 1).min(self.item_count - 1);
        }
    }

    pub fn prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Ensure selected item is visible given viewport height
    pub fn ensure_visible(&mut self, viewport_h: u16) {
        if let Some(&offset) = self.item_offsets.get(self.selected) {
            if offset < self.scroll {
                self.scroll = offset.saturating_sub(1);
            } else if offset >= self.scroll + viewport_h {
                self.scroll = offset.saturating_sub(viewport_h / 2);
            }
        }
    }
}

pub struct App {
    pub locale: String,
    pub view: View,
    pub should_quit: bool,

    // Home view
    pub homepage: LoadingState<Homepage>,
    pub home_slugs: Vec<Option<String>>,
    pub home_view: ScrollView,
    /// Cached offscreen buffer - rebuilt only when content/size changes
    pub home_buffer: Option<ratatui::buffer::Buffer>,
    pub home_buffer_size: (u16, u16), // (width, total_height) it was built for
    pub home_dirty: bool,             // set true when content changes

    // Article view
    pub article: LoadingState<EditionItem>,
    pub article_scroll: u16,
    pub article_lines: Vec<ArticleLine>,

    // Search
    pub search_query: String,
    pub search_results: LoadingState<Vec<EditionItem>>,
    pub search_view: ScrollView,
    pub search_active: bool,

    // Editions list
    pub editions: LoadingState<Vec<Edition>>,
    pub editions_view: ScrollView,

    // Images
    pub image_cache: Option<ImageCache>,
}

/// Describes where a hero image should be rendered
pub struct ImagePlacement {
    pub url: String,
    pub line_offset: u16,
    pub x_offset: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone)]
pub enum ArticleLine {
    Title(String),
    Header(String),
    Author(String),
    Meta(String),
    Heading(String),
    RichText(Vec<StyledSegment>),
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
            article: LoadingState::Loading,
            article_scroll: 0,
            article_lines: Vec::new(),
            search_query: String::new(),
            search_results: LoadingState::Loaded(Vec::new()),
            search_view: ScrollView::new(),
            search_active: false,
            editions: LoadingState::Loading,
            editions_view: ScrollView::new(),
            image_cache: None,
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
                    for item in items {
                        self.home_slugs.push(item.slug.clone());
                    }
                }
                HomepageSection::ItemList { items, .. } => {
                    for item in items {
                        self.home_slugs.push(item.slug.clone());
                    }
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

        let wrap_width = (width as usize).saturating_sub(6).max(20);
        let mut lines = Vec::new();

        if let Some(title) = &item.title {
            for line in textwrap::wrap(title, wrap_width) {
                lines.push(ArticleLine::Title(line.to_string()));
            }
            lines.push(ArticleLine::Blank);
        }

        let mut meta_parts = Vec::new();
        meta_parts.push(item.date.clone());
        meta_parts.push(format!("{} words", item.word_count as u32));
        let mins = item.read_time_secs as u32 / 60;
        if mins > 0 {
            meta_parts.push(format!("{} min read", mins));
        }
        lines.push(ArticleLine::Meta(meta_parts.join("  \u{2502}  ")));
        lines.push(ArticleLine::Blank);

        let authors = item.content.authors();
        if !authors.is_empty() {
            lines.push(ArticleLine::Author(format!("By {}", authors.join(", "))));
            lines.push(ArticleLine::Blank);
        }

        match &item.content {
            ItemContent::Longform { header, teaser, body, .. } => {
                if let Some(h) = header {
                    for line in textwrap::wrap(h, wrap_width) {
                        lines.push(ArticleLine::Header(line.to_string()));
                    }
                    lines.push(ArticleLine::Blank);
                }
                if let Some(t) = teaser {
                    for segs in markdown::wrap_md(t, wrap_width) {
                        lines.push(ArticleLine::RichText(segs));
                    }
                    lines.push(ArticleLine::Blank);
                }
                render_content_blocks(body, wrap_width, &mut lines);
            }
            ItemContent::Feature { header, label, country_codes, lead, comment, .. } => {
                if let Some(h) = header {
                    for line in textwrap::wrap(h, wrap_width) {
                        lines.push(ArticleLine::Header(line.to_string()));
                    }
                    lines.push(ArticleLine::Blank);
                }
                if let Some(l) = label {
                    lines.push(ArticleLine::Meta(l.clone()));
                    lines.push(ArticleLine::Blank);
                }
                if !country_codes.is_empty() {
                    lines.push(ArticleLine::Meta(format!("Countries: {}", country_codes.join(", "))));
                    lines.push(ArticleLine::Blank);
                }
                render_content_blocks(lead, wrap_width, &mut lines);
                if !comment.is_empty() {
                    lines.push(ArticleLine::Blank);
                    lines.push(ArticleLine::Heading("Comment".to_string()));
                    lines.push(ArticleLine::Blank);
                    render_content_blocks(comment, wrap_width, &mut lines);
                }
            }
            ItemContent::DataVis { header, description, .. } => {
                if let Some(h) = header {
                    lines.push(ArticleLine::Header(h.clone()));
                    lines.push(ArticleLine::Blank);
                }
                render_content_blocks(description, wrap_width, &mut lines);
            }
            ItemContent::CulturalRec { header, description, .. } => {
                if let Some(h) = header {
                    lines.push(ArticleLine::Header(h.clone()));
                    lines.push(ArticleLine::Blank);
                }
                render_content_blocks(description, wrap_width, &mut lines);
            }
            ItemContent::EditorsNote { body, .. } => {
                render_content_blocks(body, wrap_width, &mut lines);
            }
            ItemContent::CommunityNote { title, label, signature, description_top, description_bottom, .. } => {
                for line in textwrap::wrap(title, wrap_width) {
                    lines.push(ArticleLine::Header(line.to_string()));
                }
                lines.push(ArticleLine::Blank);
                if let Some(l) = label {
                    lines.push(ArticleLine::Meta(l.clone()));
                    lines.push(ArticleLine::Blank);
                }
                render_content_blocks(description_top, wrap_width, &mut lines);
                render_content_blocks(description_bottom, wrap_width, &mut lines);
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

fn render_content_blocks(blocks: &[ContentBlock], wrap_width: usize, lines: &mut Vec<ArticleLine>) {
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
            ContentBlock::Image { caption, alt } => {
                if let Some(cap) = caption {
                    let cleaned = strip_html(cap);
                    lines.push(ArticleLine::ImageCaption(format!("[Image: {}]", cleaned)));
                } else if let Some(a) = alt {
                    lines.push(ArticleLine::ImageCaption(format!("[Image: {}]", a)));
                } else {
                    lines.push(ArticleLine::ImageCaption("[Image]".to_string()));
                }
                lines.push(ArticleLine::Blank);
            }
        }
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
