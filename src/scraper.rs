use anyhow::{anyhow, Result};
// Use the external `scraper` crate via its full path to avoid name clash with this module.
use scraper::{Html, Selector};

/// Parsed search result before we fetch inline metadata.
pub struct RawEntry {
    pub md5: String,
    pub title: String,
    pub author: String,
}

/// Parse archive search results page.
pub fn parse_search_results(html: &str) -> Vec<RawEntry> {
    let doc = Html::parse_document(html);
    let row_sel = sel("div.border-b");
    let title_sel = sel("a.js-vim-focus[href^='/md5/']");
    let author_sel = sel("a[href*='search?q=']");
    let user_icon_sel = sel("span[class*='mdi--user-edit']");

    let entries: Vec<_> = doc
        .select(&row_sel)
        .filter_map(|row| {
            let title_node = row.select(&title_sel).next()?;
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
                .select(&author_sel)
                .find(|n| n.select(&user_icon_sel).next().is_some())
                .or_else(|| row.select(&author_sel).next())
                .map(|n| n.text().collect::<String>().trim().to_owned())
                .unwrap_or_default();

            Some(RawEntry { md5, title, author })
        })
        .collect();

    if !entries.is_empty() {
        return entries;
    }

    let title_nodes: Vec<_> = doc.select(&title_sel).collect();
    let author_nodes: Vec<_> = doc.select(&author_sel).collect();
    let preferred_author_nodes: Vec<_> = author_nodes
        .iter()
        .filter(|n| n.select(&user_icon_sel).next().is_some())
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
    let span_sel = sel("span.break-all");
    let link_sel = sel("a[href]");
    let any_break_sel = sel(".break-all");

    let mut candidates = doc
        .select(&span_sel)
        .map(|el| el.text().collect::<String>().trim().to_owned())
        .chain(
            doc.select(&link_sel)
                .filter_map(|el| el.value().attr("href").map(str::to_owned)),
        )
        .chain(
            doc.select(&any_break_sel)
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

/// Panic-free selector construction; panics only in debug on bad literals.
fn sel(css: &'static str) -> Selector {
    Selector::parse(css).unwrap_or_else(|e| panic!("bad selector `{css}`: {e:?}"))
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
}
