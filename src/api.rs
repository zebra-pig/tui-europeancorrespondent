use graphql_client::GraphQLQuery;
use reqwest::Client;

const API_URL: &str = "https://api.europeancorrespondent.com/graphql";

type Locale = String;
type Float = f64;
type ID = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.json",
    query_path = "src/graphql/queries.graphql",
    response_derives = "Debug, Clone",
    variables_derives = "Debug"
)]
pub struct FetchLatestEdition;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.json",
    query_path = "src/graphql/queries.graphql",
    response_derives = "Debug, Clone",
    variables_derives = "Debug"
)]
pub struct FetchHomepage;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.json",
    query_path = "src/graphql/queries.graphql",
    response_derives = "Debug, Clone",
    variables_derives = "Debug"
)]
pub struct FetchArticle;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.json",
    query_path = "src/graphql/queries.graphql",
    response_derives = "Debug, Clone",
    variables_derives = "Debug"
)]
pub struct SearchArticles;

// ── Domain models ──

#[derive(Debug, Clone)]
pub struct Edition {
    pub title: String,
    pub date: String,
    pub items: Vec<EditionItem>,
}

/// Hex color from the API (#RRGGBB) → ratatui Color
#[derive(Debug, Clone)]
pub struct DynColor {
    pub light: Option<(u8, u8, u8)>,
    pub dark: Option<(u8, u8, u8)>,
}

impl DynColor {
    /// Returns the light-mode color (used for foreground/accents on any background)
    pub fn rgb(&self) -> Option<(u8, u8, u8)> {
        self.light.or(self.dark)
    }

    /// Returns the dark variant (useful for card backgrounds in dark terminals)
    pub fn dark_rgb(&self) -> Option<(u8, u8, u8)> {
        self.dark.or_else(|| {
            let (r, g, b) = self.light?;
            Some((r / 4, g / 4, b / 4))
        })
    }

    /// Returns a Color suitable as foreground on the terminal's default background.
    /// Uses the raw API color directly - works on both light and dark terminals
    /// since the API colors are designed to be visible.
    pub fn accent_rgb(&self) -> Option<(u8, u8, u8)> {
        self.light.or(self.dark)
    }
}

fn parse_hex(hex: &Option<String>) -> Option<(u8, u8, u8)> {
    let s = hex.as_ref()?;
    if s.len() != 7 || !s.starts_with('#') { return None; }
    let r = u8::from_str_radix(&s[1..3], 16).ok()?;
    let g = u8::from_str_radix(&s[3..5], 16).ok()?;
    let b = u8::from_str_radix(&s[5..7], 16).ok()?;
    Some((r, g, b))
}

#[derive(Debug, Clone)]
pub struct EditionItem {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub date: String,
    pub preview_text: String,
    pub word_count: f64,
    pub read_time_secs: f64,
    pub content: ItemContent,
}

#[derive(Debug, Clone)]
pub enum ItemContent {
    Longform {
        title: String,
        header: Option<String>,
        header_color: DynColor,
        teaser: Option<String>,
        card_color: DynColor,
        title_color: DynColor,
        image_url: Option<String>,
        authors: Vec<String>,
        introduction_comment: Vec<ContentBlock>,
        body: Vec<ContentBlock>,
        comment: Vec<ContentBlock>,
    },
    Feature {
        title: String,
        header: Option<String>,
        header_color: DynColor,
        label: Option<String>,
        label_color: DynColor,
        image_url: Option<String>,
        authors: Vec<String>,
        country_codes: Vec<String>,
        introduction_comment: Vec<ContentBlock>,
        lead: Vec<ContentBlock>,
        comment: Vec<ContentBlock>,
    },
    DataVis {
        title: String,
        header: Option<String>,
        header_color: DynColor,
        label: Option<String>,
        label_color: DynColor,
        image_url: Option<String>,
        image_width: Option<i64>,
        image_height: Option<i64>,
        authors: Vec<String>,
        description: Vec<ContentBlock>,
    },
    CulturalRec {
        title: String,
        header: Option<String>,
        header_color: DynColor,
        image_url: Option<String>,
        authors: Vec<String>,
        description: Vec<ContentBlock>,
    },
    EditorsNote {
        authors: Vec<String>,
        body: Vec<ContentBlock>,
    },
    CommunityNote {
        title: String,
        label: Option<String>,
        signature: Option<String>,
        authors: Vec<String>,
        description_top: Vec<ContentBlock>,
        description_bottom: Vec<ContentBlock>,
    },
    Advert {
        title: String,
    },
}

impl ItemContent {
    pub fn type_label(&self) -> &'static str {
        match self {
            Self::Longform { .. } => "LONGFORM",
            Self::Feature { .. } => "FEATURE",
            Self::DataVis { .. } => "DATA",
            Self::CulturalRec { .. } => "CULTURE",
            Self::EditorsNote { .. } => "NOTE",
            Self::CommunityNote { .. } => "COMMUNITY",
            Self::Advert { .. } => "AD",
        }
    }

    pub fn authors(&self) -> &[String] {
        match self {
            Self::Longform { authors, .. }
            | Self::Feature { authors, .. }
            | Self::DataVis { authors, .. }
            | Self::CulturalRec { authors, .. }
            | Self::EditorsNote { authors, .. }
            | Self::CommunityNote { authors, .. } => authors,
            Self::Advert { .. } => &[],
        }
    }

    pub fn header(&self) -> Option<&str> {
        match self {
            Self::Longform { header, .. }
            | Self::Feature { header, .. }
            | Self::DataVis { header, .. }
            | Self::CulturalRec { header, .. } => header.as_deref(),
            _ => None,
        }
    }

    pub fn header_color(&self) -> Option<&DynColor> {
        match self {
            Self::Longform { header_color, .. }
            | Self::Feature { header_color, .. }
            | Self::DataVis { header_color, .. }
            | Self::CulturalRec { header_color, .. } => Some(header_color),
            _ => None,
        }
    }

    pub fn card_color(&self) -> Option<&DynColor> {
        match self {
            Self::Longform { card_color, .. } => Some(card_color),
            _ => None,
        }
    }

    pub fn label_info(&self) -> Option<(&str, &DynColor)> {
        match self {
            Self::Feature { label: Some(l), label_color, .. }
            | Self::DataVis { label: Some(l), label_color, .. } => Some((l, label_color)),
            _ => None,
        }
    }

    pub fn teaser(&self) -> Option<&str> {
        match self {
            Self::Longform { teaser, .. } => teaser.as_deref(),
            _ => None,
        }
    }

    pub fn image_url(&self) -> Option<&str> {
        match self {
            Self::Longform { image_url, .. }
            | Self::Feature { image_url, .. }
            | Self::DataVis { image_url, .. }
            | Self::CulturalRec { image_url, .. } => image_url.as_deref(),
            _ => None,
        }
    }

    /// Color to use for image placeholder (card_color for longforms, header_color otherwise)
    pub fn placeholder_color(&self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Longform { card_color, .. } => card_color.light.or(card_color.dark),
            _ => self.header_color().and_then(|c| c.light.or(c.dark)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Paragraph(String),
    Heading(String),
    Image { url: Option<String>, width: Option<i64>, height: Option<i64>, caption: Option<String>, alt: Option<String> },
}

// Homepage sections
#[derive(Debug, Clone)]
pub enum HomepageSection {
    Hero {
        items: Vec<EditionItem>,
    },
    ItemList {
        heading: Option<String>,
        subheading: Option<String>,
        header_color: DynColor,
        items: Vec<EditionItem>,
    },
    Highlight {
        item: EditionItem,
    },
    Inline {
        item: EditionItem,
    },
}

#[derive(Debug, Clone)]
pub struct Homepage {
    pub sections: Vec<HomepageSection>,
}

// ── Conversions ──

trait HasColor {
    fn light(&self) -> &Option<String>;
    fn dark(&self) -> &Option<String>;
    fn to_dyn(&self) -> DynColor {
        DynColor {
            light: parse_hex(self.light()),
            dark: parse_hex(self.dark()),
        }
    }
}

macro_rules! impl_has_color {
    ($ty:ty) => {
        impl HasColor for $ty {
            fn light(&self) -> &Option<String> { &self.light }
            fn dark(&self) -> &Option<String> { &self.dark }
        }
    };
}

fn opt_color<T: HasColor>(c: &Option<T>) -> DynColor {
    c.as_ref().map(|c| c.to_dyn()).unwrap_or(DynColor { light: None, dark: None })
}

fn req_color<T: HasColor>(c: &T) -> DynColor {
    c.to_dyn()
}

trait HasName {
    fn first_name(&self) -> &str;
    fn last_name(&self) -> &str;
    fn full_name(&self) -> String {
        format!("{} {}", self.first_name(), self.last_name()).trim().to_string()
    }
}

fn author_names<T: HasName>(authors: &[T]) -> Vec<String> {
    authors.iter().map(|a| a.full_name()).collect()
}

macro_rules! impl_has_name {
    ($ty:ty) => {
        impl HasName for $ty {
            fn first_name(&self) -> &str { &self.first_name }
            fn last_name(&self) -> &str { &self.last_name }
        }
    };
}

impl_has_name!(fetch_latest_edition::EditionItemSummaryContentOnLongformAuthors);
impl_has_name!(fetch_latest_edition::EditionItemSummaryContentOnFeatureAuthors);
impl_has_name!(fetch_latest_edition::EditionItemSummaryContentOnDataVisualisationAuthors);
impl_has_name!(fetch_latest_edition::EditionItemSummaryContentOnCulturalRecommendationAuthors);
impl_has_name!(fetch_latest_edition::EditionItemSummaryContentOnEditorsNoteAuthors);
impl_has_name!(fetch_latest_edition::EditionItemSummaryContentOnCommunityNoteAuthors);

impl_has_name!(fetch_article::EditionItemFullContentOnLongformAuthors);
impl_has_name!(fetch_article::EditionItemFullContentOnFeatureAuthors);
impl_has_name!(fetch_article::EditionItemFullContentOnDataVisualisationAuthors);
impl_has_name!(fetch_article::EditionItemFullContentOnCulturalRecommendationAuthors);
impl_has_name!(fetch_article::EditionItemFullContentOnEditorsNoteAuthors);
impl_has_name!(fetch_article::EditionItemFullContentOnCommunityNoteAuthors);

impl_has_name!(search_articles::EditionItemSummaryContentOnLongformAuthors);
impl_has_name!(search_articles::EditionItemSummaryContentOnFeatureAuthors);
impl_has_name!(search_articles::EditionItemSummaryContentOnDataVisualisationAuthors);
impl_has_name!(search_articles::EditionItemSummaryContentOnCulturalRecommendationAuthors);
impl_has_name!(search_articles::EditionItemSummaryContentOnEditorsNoteAuthors);
impl_has_name!(search_articles::EditionItemSummaryContentOnCommunityNoteAuthors);

impl_has_name!(fetch_homepage::EditionItemSummaryContentOnLongformAuthors);
impl_has_name!(fetch_homepage::EditionItemSummaryContentOnFeatureAuthors);
impl_has_name!(fetch_homepage::EditionItemSummaryContentOnDataVisualisationAuthors);
impl_has_name!(fetch_homepage::EditionItemSummaryContentOnCulturalRecommendationAuthors);
impl_has_name!(fetch_homepage::EditionItemSummaryContentOnEditorsNoteAuthors);
impl_has_name!(fetch_homepage::EditionItemSummaryContentOnCommunityNoteAuthors);

// Color type impls per query module
macro_rules! impl_summary_colors {
    ($mod:ident) => {
        impl_has_color!($mod::EditionItemSummaryContentOnLongformHeaderColor);
        impl_has_color!($mod::EditionItemSummaryContentOnLongformCardColor);
        impl_has_color!($mod::EditionItemSummaryContentOnLongformTitleColor);
        impl_has_color!($mod::EditionItemSummaryContentOnFeatureHeaderColor);
        impl_has_color!($mod::EditionItemSummaryContentOnFeatureLabelColor);
        impl_has_color!($mod::EditionItemSummaryContentOnDataVisualisationHeaderColor);
        impl_has_color!($mod::EditionItemSummaryContentOnDataVisualisationLabelColor);
        impl_has_color!($mod::EditionItemSummaryContentOnCulturalRecommendationHeaderColor);
    };
}
impl_summary_colors!(fetch_latest_edition);
impl_summary_colors!(search_articles);
impl_summary_colors!(fetch_homepage);

impl_has_color!(fetch_article::EditionItemFullContentOnLongformHeaderColor);
impl_has_color!(fetch_article::EditionItemFullContentOnLongformCardColor);
impl_has_color!(fetch_article::EditionItemFullContentOnLongformTitleColor);
impl_has_color!(fetch_article::EditionItemFullContentOnFeatureHeaderColor);
impl_has_color!(fetch_article::EditionItemFullContentOnFeatureLabelColor);
impl_has_color!(fetch_article::EditionItemFullContentOnDataVisualisationHeaderColor);
impl_has_color!(fetch_article::EditionItemFullContentOnDataVisualisationLabelColor);
impl_has_color!(fetch_article::EditionItemFullContentOnCulturalRecommendationHeaderColor);

impl_has_color!(fetch_homepage::FetchHomepagePageSectionsOnEditionItemListSectionHeaderColor);

fn convert_blocks(blocks: &[fetch_article::ContentBlockFields]) -> Vec<ContentBlock> {
    blocks
        .iter()
        .map(|b| match b {
            fetch_article::ContentBlockFields::ParagraphContentBlock(p) => {
                ContentBlock::Paragraph(p.text.clone())
            }
            fetch_article::ContentBlockFields::HeadingContentBlock(h) => {
                ContentBlock::Heading(h.text.clone())
            }
            fetch_article::ContentBlockFields::ImageContentBlock(i) => {
                ContentBlock::Image {
                    url: Some(i.image.url.clone()),
                    width: i.image.width,
                    height: i.image.height,
                    caption: i.image.caption.clone(),
                    alt: i.image.alt.clone(),
                }
            }
        })
        .collect()
}

// Use a macro since all 3 summary queries generate identical EditionItemSummaryContent shapes
macro_rules! convert_summary_content_impl {
    ($mod:ident, $fn_name:ident) => {
        fn $fn_name(c: $mod::EditionItemSummaryContent) -> ItemContent {
            use $mod::EditionItemSummaryContent::*;
            match c {
                Longform(l) => ItemContent::Longform {
                    title: l.longform_title, header: l.header, teaser: l.teaser,
                    header_color: opt_color(&l.header_color), card_color: req_color(&l.card_color),
                    title_color: opt_color(&l.title_color),
                    image_url: Some(l.top_image.url),
                    authors: author_names(&l.authors),
                    introduction_comment: vec![], body: vec![], comment: vec![],
                },
                Feature(f) => ItemContent::Feature {
                    title: f.feature_title, header: f.header, label: f.label,
                    header_color: opt_color(&f.header_color), label_color: opt_color(&f.label_color),
                    image_url: f.image.map(|i| i.url),
                    authors: author_names(&f.authors), country_codes: f.country_codes,
                    introduction_comment: vec![], lead: vec![], comment: vec![],
                },
                DataVisualisation(d) => ItemContent::DataVis {
                    title: d.dv_title, header: d.header, label: d.label,
                    header_color: opt_color(&d.header_color), label_color: opt_color(&d.label_color),
                    image_url: d.image.map(|i| i.url),
                    image_width: None, image_height: None,
                    authors: author_names(&d.authors), description: vec![],
                },
                CulturalRecommendation(c) => ItemContent::CulturalRec {
                    title: c.cr_title, header: c.header, header_color: opt_color(&c.header_color),
                    image_url: c.cover_image.map(|i| i.url),
                    authors: author_names(&c.authors), description: vec![],
                },
                EditorsNote(e) => ItemContent::EditorsNote { authors: author_names(&e.authors), body: vec![] },
                CommunityNote(c) => ItemContent::CommunityNote {
                    title: c.cn_title, label: c.label, signature: c.signature,
                    authors: author_names(&c.authors), description_top: vec![], description_bottom: vec![],
                },
                Advert(a) => ItemContent::Advert { title: a.ad_title },
            }
        }
    };
}

convert_summary_content_impl!(fetch_latest_edition, convert_edition_content);
convert_summary_content_impl!(fetch_homepage, convert_homepage_content);
convert_summary_content_impl!(search_articles, convert_search_content);

fn convert_full_content(c: fetch_article::EditionItemFullContent) -> ItemContent {
    use fetch_article::EditionItemFullContent::*;
    match c {
        Longform(l) => ItemContent::Longform {
            title: l.longform_title, header: l.header, teaser: l.teaser,
            header_color: opt_color(&l.header_color), card_color: req_color(&l.card_color),
            title_color: opt_color(&l.title_color), image_url: None,
            authors: author_names(&l.authors),
            introduction_comment: l.introduction_comment.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
            body: convert_blocks(&l.content),
            comment: l.comment.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
        },
        Feature(f) => ItemContent::Feature {
            title: f.feature_title, header: f.header, label: f.label,
            header_color: opt_color(&f.header_color), label_color: opt_color(&f.label_color),
            image_url: None,
            authors: author_names(&f.authors), country_codes: f.country_codes,
            introduction_comment: f.introduction_comment.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
            lead: f.lead.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
            comment: f.comment.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
        },
        DataVisualisation(d) => ItemContent::DataVis {
            title: d.dv_title, header: d.header, label: d.label,
            header_color: opt_color(&d.header_color), label_color: opt_color(&d.label_color),
            image_url: d.image.as_ref().map(|i| i.url.clone()),
            image_width: d.image.as_ref().and_then(|i| i.width),
            image_height: d.image.as_ref().and_then(|i| i.height),
            authors: author_names(&d.authors),
            description: d.description.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
        },
        CulturalRecommendation(c) => ItemContent::CulturalRec {
            title: c.cr_title, header: c.header, header_color: opt_color(&c.header_color),
            image_url: None,
            authors: author_names(&c.authors),
            description: c.description.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
        },
        EditorsNote(e) => ItemContent::EditorsNote {
            authors: author_names(&e.authors),
            body: convert_blocks(&e.content),
        },
        CommunityNote(c) => ItemContent::CommunityNote {
            title: c.cn_title, label: c.label, signature: c.signature,
            authors: author_names(&c.authors),
            description_top: c.description_top.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
            description_bottom: c.description_bottom.as_ref().map(|b| convert_blocks(b)).unwrap_or_default(),
        },
        Advert(a) => ItemContent::Advert { title: a.ad_title },
    }
}

fn convert_edition_item(item: fetch_latest_edition::EditionItemSummary) -> EditionItem {
    EditionItem {
        title: item.title, slug: item.slug, date: item.date,
        preview_text: item.preview_text, word_count: item.word_count,
        read_time_secs: item.expected_read_time_seconds,
        content: convert_edition_content(item.content),
    }
}

fn convert_homepage_item(item: fetch_homepage::EditionItemSummary) -> EditionItem {
    EditionItem {
        title: item.title, slug: item.slug, date: item.date,
        preview_text: item.preview_text, word_count: item.word_count,
        read_time_secs: item.expected_read_time_seconds,
        content: convert_homepage_content(item.content),
    }
}

fn convert_search_item(item: search_articles::EditionItemSummary) -> EditionItem {
    EditionItem {
        title: item.title, slug: item.slug, date: item.date,
        preview_text: item.preview_text, word_count: item.word_count,
        read_time_secs: item.expected_read_time_seconds,
        content: convert_search_content(item.content),
    }
}

// ── API Client ──

pub struct ApiClient {
    client: Client,
    api_key: Option<String>,
}

impl ApiClient {
    pub fn new(api_key: Option<String>) -> Self {
        Self { client: Client::new(), api_key }
    }

    async fn gql<Q: GraphQLQuery>(&self, vars: Q::Variables) -> Result<Q::ResponseData, String>
    where Q::Variables: serde::Serialize, Q::ResponseData: serde::de::DeserializeOwned {
        let body = Q::build_query(vars);
        let mut req = self.client.post(API_URL).json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        let resp: graphql_client::Response<Q::ResponseData> = req.send().await
            .map_err(|e| format!("Request failed: {}", e))?.json().await
            .map_err(|e| format!("Parse failed: {}", e))?;
        if let Some(errors) = &resp.errors {
            if !errors.is_empty() {
                return Err(errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join(", "));
            }
        }
        resp.data.ok_or_else(|| "No data returned".to_string())
    }

    pub async fn fetch_homepage(&self, locale: &str) -> Result<Homepage, String> {
        let data = self.gql::<FetchHomepage>(fetch_homepage::Variables {
            locale: Some(locale.to_string()),
        }).await?;

        let page = data.page.ok_or("Homepage not found")?;
        let sections = page.sections.into_iter().filter_map(|s| {
            use fetch_homepage::FetchHomepagePageSections::*;
            match s {
                HeroSection(h) => {
                    let items = vec![
                        convert_homepage_item(h.edition_item1),
                        convert_homepage_item(h.edition_item2),
                        convert_homepage_item(h.edition_item3),
                        convert_homepage_item(h.edition_item4),
                        convert_homepage_item(h.edition_item5),
                    ];
                    Some(HomepageSection::Hero { items })
                }
                EditionItemListSection(l) => {
                    let heading = if l.heading.is_empty() { None } else { Some(l.heading) };
                    let subheading = l.subheading.filter(|s| !s.is_empty());
                    Some(HomepageSection::ItemList {
                        heading,
                        subheading,
                        header_color: req_color(&l.header_color),
                        items: l.edition_items.into_iter().map(convert_homepage_item).collect(),
                    })
                }
                EditionItemHighlightSection(h) => {
                    Some(HomepageSection::Highlight { item: convert_homepage_item(h.edition_item) })
                }
                EditionItemInlineSection(i) => {
                    Some(HomepageSection::Inline { item: convert_homepage_item(i.edition_item) })
                }
                _ => None,
            }
        }).collect();

        Ok(Homepage { sections })
    }

    pub async fn fetch_latest_edition(&self, locale: &str) -> Result<Edition, String> {
        let data = self.gql::<FetchLatestEdition>(fetch_latest_edition::Variables {
            locale: Some(locale.to_string()),
        }).await?;
        let ed = data.editions.nodes.into_iter().next().ok_or("No editions found")?;
        Ok(Edition { title: ed.title, date: ed.date, items: ed.items.into_iter().map(convert_edition_item).collect() })
    }

    pub async fn fetch_article(&self, slug: &str, locale: &str) -> Result<EditionItem, String> {
        let data = self.gql::<FetchArticle>(fetch_article::Variables {
            slug: Some(slug.to_string()), locale: Some(locale.to_string()),
        }).await?;
        let item = data.edition_item.ok_or("Article not found")?;
        Ok(EditionItem {
            title: item.title, slug: item.slug, date: item.date,
            preview_text: item.preview_text, word_count: item.word_count,
            read_time_secs: item.expected_read_time_seconds,
            content: convert_full_content(item.content),
        })
    }

    pub async fn fetch_editions_list(&self, locale: &str) -> Result<Vec<Edition>, String> {
        let data = self.gql::<FetchLatestEdition>(fetch_latest_edition::Variables {
            locale: Some(locale.to_string()),
        }).await?;
        Ok(data.editions.nodes.into_iter().map(|ed| Edition {
            title: ed.title, date: ed.date, items: ed.items.into_iter().map(convert_edition_item).collect(),
        }).collect())
    }

    pub async fn search_articles(&self, query_text: &str, locale: &str) -> Result<Vec<EditionItem>, String> {
        let filter = search_articles::EditionItemFilter {
            plain_text: Some(search_articles::StringFilterField {
                contains: Some(query_text.to_string()),
                eq: None, neq: None, in_: None, nin: None, null: None,
                gt: None, gte: None, lt: None, lte: None,
            }),
            id: None, slug: None, published_from: None,
            about_european_institution: None, with_data_visualisation: None,
            country_codes: None, author_ids: None, and: None, or: None,
        };
        let data = self.gql::<SearchArticles>(search_articles::Variables {
            filter: Some(filter), locale: Some(locale.to_string()),
        }).await?;
        Ok(data.edition_items.nodes.into_iter().map(convert_search_item).collect())
    }
}
