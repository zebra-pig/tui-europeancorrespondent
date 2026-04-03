mod api;
mod app;
mod images;
mod markdown;
mod ui;

use app::{App, LoadingState, View};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use image::DynamicImage;
use ratatui_image::picker::Picker;
use std::io::stdout;
use std::sync::Arc;
use tokio::sync::mpsc;

enum AsyncMsg {
    HomepageLoaded(Result<api::Homepage, String>),
    ArticleLoaded(Result<api::EditionItem, String>),
    SearchResults(Result<Vec<api::EditionItem>, String>),
    EditionsLoaded(Result<Vec<api::Edition>, String>),
    ImageLoaded(String, DynamicImage),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Detect image protocol before raw mode
    let picker = Picker::from_query_stdio().ok();

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    let (tx, mut rx) = mpsc::unbounded_channel::<AsyncMsg>();
    let client = Arc::new(api::ApiClient::new());

    // Image cache
    if let Some(ref p) = picker {
        let (cache, mut img_rx) = images::ImageCache::new(p);
        let tx2 = tx.clone();
        tokio::spawn(async move {
            while let Some((url, proto)) = img_rx.recv().await {
                let _ = tx2.send(AsyncMsg::ImageLoaded(url, proto));
            }
        });
        app.image_cache = Some(cache);
    }

    // Load homepage
    {
        let tx = tx.clone();
        let client = client.clone();
        let locale = app.locale.clone();
        tokio::spawn(async move {
            let result = client.fetch_homepage(&locale).await;
            let _ = tx.send(AsyncMsg::HomepageLoaded(result));
        });
    }

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        while let Ok(msg) = rx.try_recv() {
            match msg {
                AsyncMsg::HomepageLoaded(result) => {
                    app.homepage = match result {
                        Ok(hp) => LoadingState::Loaded(hp),
                        Err(e) => LoadingState::Error(e),
                    };
                    app.rebuild_home_items();
                    app.home_dirty = true;
                    // Fetch images for hero and highlight sections
                    if let (Some(cache), LoadingState::Loaded(hp)) = (&mut app.image_cache, &app.homepage) {
                        for section in &hp.sections {
                            match section {
                                api::HomepageSection::Hero { items } => {
                                    for item in items {
                                        if let Some(url) = item.content.image_url() {
                                            cache.fetch(url);
                                        }
                                    }
                                }
                                api::HomepageSection::Highlight { item } => {
                                    if let Some(url) = item.content.image_url() {
                                        cache.fetch(url);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                AsyncMsg::ArticleLoaded(result) => {
                    app.article = match result {
                        Ok(item) => LoadingState::Loaded(item),
                        Err(e) => LoadingState::Error(e),
                    };
                    let size = terminal.size()?;
                    app.build_article_lines(size.width);
                }
                AsyncMsg::SearchResults(result) => {
                    app.search_results = match result {
                        Ok(items) => LoadingState::Loaded(items),
                        Err(e) => LoadingState::Error(e),
                    };
                    app.search_view.selected = 0;
                    app.search_view.scroll = 0;
                }
                AsyncMsg::EditionsLoaded(result) => {
                    app.editions = match result {
                        Ok(eds) => LoadingState::Loaded(eds),
                        Err(e) => LoadingState::Error(e),
                    };
                    app.editions_view.selected = 0;
                    app.editions_view.scroll = 0;
                }
                AsyncMsg::ImageLoaded(url, img) => {
                    if let Some(cache) = &mut app.image_cache {
                        cache.insert(url, img);
                    }
                    app.home_dirty = true;
                }
            }
        }

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }

                if app.search_active {
                    match key.code {
                        KeyCode::Esc => app.search_active = false,
                        KeyCode::Enter => {
                            app.search_active = false;
                            if !app.search_query.is_empty() {
                                app.view = View::Search;
                                app.search_results = LoadingState::Loading;
                                let tx = tx.clone();
                                let client = client.clone();
                                let query = app.search_query.clone();
                                let locale = app.locale.clone();
                                tokio::spawn(async move {
                                    let result = client.search_articles(&query, &locale).await;
                                    let _ = tx.send(AsyncMsg::SearchResults(result));
                                });
                            }
                        }
                        KeyCode::Backspace => { app.search_query.pop(); }
                        KeyCode::Char(c) => app.search_query.push(c),
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => { app.should_quit = true; }
                    KeyCode::Char('/') => { app.search_active = true; app.search_query.clear(); }
                    _ => {}
                }

                match &app.view {
                    View::Home => match key.code {
                        KeyCode::Char('j') | KeyCode::Down => app.home_view.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.home_view.prev(),
                        KeyCode::Enter => {
                            if let Some(slug) = app.selected_slug() {
                                app.view = View::Article;
                                app.article = LoadingState::Loading;
                                app.article_scroll = 0;
                                let tx = tx.clone();
                                let client = client.clone();
                                let locale = app.locale.clone();
                                tokio::spawn(async move {
                                    let result = client.fetch_article(&slug, &locale).await;
                                    let _ = tx.send(AsyncMsg::ArticleLoaded(result));
                                });
                            }
                        }
                        KeyCode::Char('e') => {
                            app.view = View::EditionsList;
                            app.editions = LoadingState::Loading;
                            let tx = tx.clone();
                            let client = client.clone();
                            let locale = app.locale.clone();
                            tokio::spawn(async move {
                                let result = client.fetch_editions_list(&locale).await;
                                let _ = tx.send(AsyncMsg::EditionsLoaded(result));
                            });
                        }
                        _ => {}
                    },
                    View::Article => match key.code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.article_scroll = app.article_scroll.saturating_add(3);
                            let max = (app.article_lines.len() as u16).saturating_sub(5);
                            app.article_scroll = app.article_scroll.min(max);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.article_scroll = app.article_scroll.saturating_sub(3);
                        }
                        KeyCode::Char(' ') | KeyCode::PageDown => {
                            let page = terminal.size()?.height.saturating_sub(6);
                            app.article_scroll = app.article_scroll.saturating_add(page);
                            let max = (app.article_lines.len() as u16).saturating_sub(5);
                            app.article_scroll = app.article_scroll.min(max);
                        }
                        KeyCode::PageUp => {
                            let page = terminal.size()?.height.saturating_sub(6);
                            app.article_scroll = app.article_scroll.saturating_sub(page);
                        }
                        KeyCode::Esc | KeyCode::Backspace => { app.view = View::Home; }
                        _ => {}
                    },
                    View::Search => match key.code {
                        KeyCode::Char('j') | KeyCode::Down => app.search_view.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.search_view.prev(),
                        KeyCode::Enter => {
                            if let LoadingState::Loaded(results) = &app.search_results {
                                let idx = app.search_view.selected;
                                if let Some(slug) = results.get(idx).and_then(|i| i.slug.clone()) {
                                    app.view = View::Article;
                                    app.article = LoadingState::Loading;
                                    app.article_scroll = 0;
                                    let tx = tx.clone();
                                    let client = client.clone();
                                    let locale = app.locale.clone();
                                    tokio::spawn(async move {
                                        let result = client.fetch_article(&slug, &locale).await;
                                        let _ = tx.send(AsyncMsg::ArticleLoaded(result));
                                    });
                                }
                            }
                        }
                        KeyCode::Esc | KeyCode::Backspace => { app.view = View::Home; }
                        _ => {}
                    },
                    View::EditionsList => match key.code {
                        KeyCode::Char('j') | KeyCode::Down => app.editions_view.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.editions_view.prev(),
                        KeyCode::Enter => { app.view = View::Home; }
                        KeyCode::Esc | KeyCode::Backspace => { app.view = View::Home; }
                        _ => {}
                    },
                }

                if app.should_quit { break; }
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
