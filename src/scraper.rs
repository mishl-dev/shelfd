use anyhow::{Result, anyhow};
use scraper::{Html, Selector};
use std::sync::OnceLock;

fn row_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse("div.border-b").unwrap())
}

fn title_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse("a.js-vim-focus[href^='/md5/']").unwrap())
}

fn author_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse("a[href*='search?q=']").unwrap())
}

fn user_icon_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse("span[class*='mdi--user-edit']").unwrap())
}

fn span_break_all_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse("span.break-all").unwrap())
}

fn link_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse("a[href]").unwrap())
}

fn any_break_sel() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse(".break-all").unwrap())
}

/// Parsed search result before we fetch inline metadata.
pub struct RawEntry {
    pub md5: String,
    pub title: String,
    pub author: String,
}

/// Parse archive search results page.
pub fn parse_search_results(html: &str) -> Vec<RawEntry> {
    let doc = Html::parse_document(html);
    let row_sel = row_sel();
    let title_sel = title_sel();
    let author_sel = author_sel();
    let user_icon_sel = user_icon_sel();

    let entries: Vec<_> = doc
        .select(row_sel)
        .filter_map(|row| {
            let title_node = row.select(title_sel).next()?;
            let href = title_node.value().attr("href")?;
            let md5 = href.strip_prefix("/md5/")?.trim().to_owned();
            if md5.is_empty() {
                return None;
            }
            let title = title_node.text().collect::<String>().trim().to_owned();
            if title.is_empty() {
                return None;
            }
            let author = row
                .select(author_sel)
                .find(|n| n.select(user_icon_sel).next().is_some())
                .or_else(|| row.select(author_sel).next())
                .map(|n| n.text().collect::<String>().trim().to_owned())
                .unwrap_or_default();

            Some(RawEntry { md5, title, author })
        })
        .collect();

    if !entries.is_empty() {
        return entries;
    }

    let title_nodes: Vec<_> = doc.select(title_sel).collect();
    let author_nodes: Vec<_> = doc.select(author_sel).collect();
    let preferred_author_nodes: Vec<_> = author_nodes
        .iter()
        .filter(|n| n.select(user_icon_sel).next().is_some())
        .cloned()
        .collect();

    title_nodes
        .into_iter()
        .enumerate()
        .filter_map(|(i, title_node)| {
            let href = title_node.value().attr("href")?;
            let md5 = href.strip_prefix("/md5/")?.trim().to_owned();
            if md5.is_empty() {
                return None;
            }
            let title = title_node.text().collect::<String>().trim().to_owned();
            if title.is_empty() {
                return None;
            }
            let author = preferred_author_nodes
                .get(i)
                .or_else(|| author_nodes.get(i))
                .map(|n| n.text().collect::<String>().trim().to_owned())
                .unwrap_or_default();

            Some(RawEntry { md5, title, author })
        })
        .collect()
}

/// Detect whether the archive returned a search error (e.g. page limit exceeded).
pub fn has_search_error(html: &str) -> bool {
    html.contains("Error during search.")
}

/// Extract the download URL from the slow_download page.
pub fn parse_download_url(html: &str) -> Result<String> {
    let doc = Html::parse_document(html);
    let span_sel = span_break_all_sel();
    let link_sel = link_sel();
    let any_break_sel = any_break_sel();

    let mut candidates = doc
        .select(span_sel)
        .map(|el| el.text().collect::<String>().trim().to_owned())
        .chain(
            doc.select(link_sel)
                .filter_map(|el| el.value().attr("href").map(str::to_owned)),
        )
        .chain(
            doc.select(any_break_sel)
                .map(|el| el.text().collect::<String>().trim().to_owned()),
        );

    let url = candidates
        .find(|candidate| looks_like_download_url(candidate))
        .or_else(|| first_http_url(html))
        .ok_or_else(|| anyhow!("download URL not found in slow_download page"))?;

    Ok(url)
}

fn looks_like_download_url(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

fn first_http_url(html: &str) -> Option<String> {
    let start = html.find("https://").or_else(|| html.find("http://"))?;
    let tail = &html[start..];
    let end = tail
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '<')
        .unwrap_or(tail.len());
    let candidate = tail[..end].trim();
    looks_like_download_url(candidate).then(|| candidate.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_search_results_extracts_md5_title_and_author() {
        let html = r#"
        <html><body>
          <a class="js-vim-focus" href="/md5/abc123">Book One</a>
          <a href="/search?q=author1">Author One</a>
          <a class="js-vim-focus" href="/md5/def456">Book Two</a>
          <a href="/search?q=author2">Author Two</a>
        </body></html>
        "#;

        let entries = parse_search_results(html);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].md5, "abc123");
        assert_eq!(entries[0].title, "Book One");
        assert_eq!(entries[0].author, "Author One");
        assert_eq!(entries[1].md5, "def456");
        assert_eq!(entries[1].title, "Book Two");
        assert_eq!(entries[1].author, "Author Two");
    }

    #[test]
    fn parse_search_results_empty_html() {
        let entries = parse_search_results("");
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_search_results_filters_empty_md5() {
        let html = r#"
        <html><body>
          <a class="js-vim-focus" href="/md5/">Empty MD5</a>
          <a href="/search?q=author">Author</a>
        </body></html>
        "#;
        let entries = parse_search_results(html);
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_search_results_filters_empty_title() {
        let html = r#"
        <html><body>
          <a class="js-vim-focus" href="/md5/abc123">   </a>
          <a href="/search?q=author">Author</a>
        </body></html>
        "#;
        let entries = parse_search_results(html);
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_search_results_no_author_defaults_to_empty() {
        let html = r#"
        <html><body>
          <a class="js-vim-focus" href="/md5/abc123">Book</a>
        </body></html>
        "#;
        let entries = parse_search_results(html);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].author, "");
    }

    #[test]
    fn parse_search_results_fallback_path() {
        let html = r#"
        <html><body>
          <a class="js-vim-focus" href="/md5/aaa111">Book A</a>
          <a href="/search?q=authorA">Author A</a>
          <a class="js-vim-focus" href="/md5/bbb222">Book B</a>
          <a href="/search?q=authorB">Author B</a>
        </body></html>
        "#;

        let entries = parse_search_results(html);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].md5, "aaa111");
        assert_eq!(entries[0].title, "Book A");
        assert_eq!(entries[1].md5, "bbb222");
        assert_eq!(entries[1].title, "Book B");
    }

    #[test]
    fn parse_search_results_prefers_author_with_user_icon() {
        let html = r#"
        <html><body>
          <div class="border-b">
            <a class="js-vim-focus" href="/md5/abc123">Book</a>
            <a href="/search?q=editor">Editor</a>
            <a href="/search?q=real_author">
              <span class="mdi--user-edit-icon"></span>
              Real Author
            </a>
          </div>
        </body></html>
        "#;
        let entries = parse_search_results(html);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].author, "Real Author");
    }

    #[test]
    fn parse_download_url_extracts_first_break_all_span() {
        let html = r#"
        <html><body>
          <span class="break-all">https://example.com/download.epub</span>
        </body></html>
        "#;

        let url = parse_download_url(html).unwrap();
        assert_eq!(url, "https://example.com/download.epub");
    }

    #[test]
    fn parse_download_url_falls_back_to_anchor_href() {
        let html = r#"
        <html><body>
          <a href="https://cdn.example.com/file.pdf">download</a>
        </body></html>
        "#;

        let url = parse_download_url(html).unwrap();
        assert_eq!(url, "https://cdn.example.com/file.pdf");
    }

    #[test]
    fn parse_download_url_falls_back_to_raw_html_url() {
        let html = r#"<html><body>https://raw.example.com/file.txt</body></html>"#;
        let url = parse_download_url(html).unwrap();
        assert_eq!(url, "https://raw.example.com/file.txt");
    }

    #[test]
    fn parse_download_url_no_url_returns_error() {
        let html = r#"<html><body><p>No URL here</p></body></html>"#;
        assert!(parse_download_url(html).is_err());
    }

    #[test]
    fn parse_download_url_empty_html_returns_error() {
        assert!(parse_download_url("").is_err());
    }

    #[test]
    fn has_search_error_detects_error() {
        let html = "<html><body>Error during search. Please try again.</body></html>";
        assert!(has_search_error(html));
    }

    #[test]
    fn has_search_error_normal_html() {
        let html = "<html><body>Results found</body></html>";
        assert!(!has_search_error(html));
    }

    #[test]
    fn has_search_error_empty_html() {
        assert!(!has_search_error(""));
    }

    #[test]
    fn looks_like_download_url_accepts_http() {
        assert!(looks_like_download_url("http://example.com"));
        assert!(looks_like_download_url("https://example.com"));
    }

    #[test]
    fn looks_like_download_url_rejects_non_http() {
        assert!(!looks_like_download_url("ftp://example.com"));
        assert!(!looks_like_download_url("data:text/plain,hello"));
        assert!(!looks_like_download_url("javascript:alert(1)"));
        assert!(!looks_like_download_url(""));
    }

    #[test]
    fn first_http_url_extracts_terminated_by_whitespace() {
        let html = "prefix https://example.com/file.epub suffix";
        assert_eq!(
            first_http_url(html),
            Some("https://example.com/file.epub".to_owned())
        );
    }

    #[test]
    fn first_http_url_extracts_terminated_by_quote() {
        let html = r#"prefix "https://example.com/file.pdf" suffix"#;
        assert_eq!(
            first_http_url(html),
            Some("https://example.com/file.pdf".to_owned())
        );
    }

    #[test]
    fn first_http_url_extracts_terminated_by_angle_bracket() {
        let html = "prefix https://example.com/file.txt<rest>";
        assert_eq!(
            first_http_url(html),
            Some("https://example.com/file.txt".to_owned())
        );
    }

    #[test]
    fn first_http_url_no_url_returns_none() {
        assert!(first_http_url("no url here").is_none());
    }

    #[test]
    fn first_http_url_prefers_https_over_http() {
        let html = "http://first.com https://second.com";
        assert_eq!(first_http_url(html), Some("https://second.com".to_owned()));
    }
}
