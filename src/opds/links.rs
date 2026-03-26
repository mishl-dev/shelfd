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
