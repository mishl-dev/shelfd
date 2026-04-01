pub fn absolute_url(public_base_url: Option<&str>, path: &str) -> String {
    match public_base_url {
        Some(base) if !base.is_empty() => format!("{}{}", base.trim_end_matches('/'), path),
        _ => path.to_owned(),
    }
}

pub fn extract_cover_id(url: &str) -> Option<&str> {
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let url = url.strip_prefix("covers.openlibrary.org/b/id/")?;
    let id = url.split('-').next()?;
    if id.is_empty() { None } else { Some(id) }
}

pub fn search_page_path(query: &str, page: usize) -> String {
    let encoded = urlencoding::encode(query);
    let page = page.max(1);
    format!("/opds/search?q={encoded}&page={page}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_url_with_base() {
        assert_eq!(
            absolute_url(Some("http://localhost:7451"), "/opds/search?q=test"),
            "http://localhost:7451/opds/search?q=test"
        );
    }

    #[test]
    fn absolute_url_without_base() {
        assert_eq!(
            absolute_url(None, "/opds/search?q=test"),
            "/opds/search?q=test"
        );
    }

    #[test]
    fn absolute_url_base_with_trailing_slash() {
        assert_eq!(
            absolute_url(Some("http://localhost:7451/"), "/opds/search"),
            "http://localhost:7451/opds/search"
        );
    }

    #[test]
    fn absolute_url_empty_base() {
        assert_eq!(absolute_url(Some(""), "/opds/search"), "/opds/search");
    }

    #[test]
    fn extract_cover_id_numeric() {
        assert_eq!(
            extract_cover_id("https://covers.openlibrary.org/b/id/12345-M.jpg"),
            Some("12345")
        );
    }

    #[test]
    fn extract_cover_id_non_matching() {
        assert!(extract_cover_id("https://example.com/cover.jpg").is_none());
    }

    #[test]
    fn extract_cover_id_empty() {
        assert!(extract_cover_id("").is_none());
    }

    #[test]
    fn extract_cover_id_http_prefix() {
        assert_eq!(
            extract_cover_id("http://covers.openlibrary.org/b/id/999-L.jpg"),
            Some("999")
        );
    }

    #[test]
    fn search_page_path_page_1() {
        assert_eq!(search_page_path("dune", 1), "/opds/search?q=dune&page=1");
    }

    #[test]
    fn search_page_path_page_clamped_to_1() {
        assert_eq!(search_page_path("dune", 0), "/opds/search?q=dune&page=1");
    }

    #[test]
    fn search_page_path_special_chars_encoded() {
        let path = search_page_path("hello world", 1);
        assert!(path.contains("hello+world") || path.contains("hello%20world"));
    }
}
