use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};
use std::io::Cursor;

use crate::{
    models::{BookEntry, ExploreEntry},
    state::AppState,
};

const ATOM_NS: &str = "http://www.w3.org/2005/Atom";
const OPDS_NS: &str = "http://opds-spec.org/2010/catalog";
const DC_NS: &str = "http://purl.org/dc/terms/";
const THR_NS: &str = "http://purl.org/syndication/thread/1.0";
const OPENSEARCH_NS: &str = "http://a9.com/-/spec/opensearch/1.1/";
const OPDS_CT_NAV: &str = "application/atom+xml;profile=opds-catalog;kind=navigation";
const OPDS_CT_ACQ: &str = "application/atom+xml;profile=opds-catalog;kind=acquisition";
const OPENSEARCH_CT: &str = "application/opensearchdescription+xml";
const DEFAULT_ACQ_TYPE: &str = "application/octet-stream";

pub struct PaginationPaths {
    pub self_href: String,
    pub next_href: Option<String>,
    pub previous_href: Option<String>,
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
}

/// GET /opds — root navigation feed.
pub async fn root_feed(State(state): State<AppState>) -> Response {
    let xml = build_root_feed(
        state.public_base_url.as_deref(),
        &state.app_name,
        &state.archive_name,
    );
    atom_response(xml)
}

pub async fn open_search(State(state): State<AppState>) -> Response {
    let xml = build_open_search_description(
        state.public_base_url.as_deref(),
        &state.app_name,
        &state.archive_name,
    );
    (
        [(
            header::CONTENT_TYPE,
            "application/opensearchdescription+xml; charset=utf-8",
        )],
        xml,
    )
        .into_response()
}

#[allow(clippy::too_many_arguments)]
pub fn search_feed(
    query: &str,
    books: &[BookEntry],
    page: usize,
    has_more: bool,
    public_base_url: Option<&str>,
    app_name: &str,
    _archive_name: &str,
    archive_base: &str,
) -> String {
    let mut w = writer();

    start_feed(
        &mut w,
        &format!(
            "urn:{app_name}:search:{}:page:{page}",
            urlencoding::encode(query)
        ),
    );
    text_elem(&mut w, "title", &format!("Search: {query}"));
    text_elem(&mut w, "updated", &now_rfc3339());
    feed_author(&mut w, app_name);
    generator(&mut w, app_name);

    let self_path = search_page_path(query, page);
    // Self link
    link(
        &mut w,
        "self",
        &absolute_url(public_base_url, &self_path),
        OPDS_CT_ACQ,
    );
    link(
        &mut w,
        "start",
        &absolute_url(public_base_url, "/opds"),
        OPDS_CT_NAV,
    );
    // Up link
    link(
        &mut w,
        "up",
        &absolute_url(public_base_url, "/opds"),
        OPDS_CT_NAV,
    );
    if page > 1 {
        link(
            &mut w,
            "previous",
            &absolute_url(public_base_url, &search_page_path(query, page - 1)),
            OPDS_CT_ACQ,
        );
    }
    if has_more {
        link(
            &mut w,
            "next",
            &absolute_url(public_base_url, &search_page_path(query, page + 1)),
            OPDS_CT_ACQ,
        );
    }
    let total_results = books.len().to_string();
    simple_elem_with_attrs(&mut w, "opensearch:totalResults", &[], &total_results);
    simple_elem_with_attrs(
        &mut w,
        "opensearch:itemsPerPage",
        &[],
        &books.len().to_string(),
    );
    simple_elem_with_attrs(
        &mut w,
        "opensearch:startIndex",
        &[],
        &page
            .saturating_sub(1)
            .saturating_mul(books.len().max(1))
            .to_string(),
    );

    for book in books {
        entry(&mut w, book, public_base_url, archive_base);
    }

    end_feed(&mut w);
    finish(w)
}

fn search_page_path(query: &str, page: usize) -> String {
    let encoded = urlencoding::encode(query);
    let page = page.max(1);
    format!("/opds/search?q={encoded}&page={page}")
}

pub fn explore_root_feed(
    public_base_url: Option<&str>,
    subjects: &[crate::state::ExploreSubject],
    app_name: &str,
    _archive_name: &str,
) -> String {
    let mut w = writer();

    start_feed(&mut w, &format!("urn:{app_name}:explore"));
    text_elem(&mut w, "title", "Explore Books");
    text_elem(&mut w, "updated", &now_rfc3339());
    feed_author(&mut w, app_name);
    generator(&mut w, app_name);

    link(
        &mut w,
        "self",
        &absolute_url(public_base_url, "/opds/explore"),
        OPDS_CT_NAV,
    );
    link(
        &mut w,
        "start",
        &absolute_url(public_base_url, "/opds"),
        OPDS_CT_NAV,
    );
    link(
        &mut w,
        "up",
        &absolute_url(public_base_url, "/opds"),
        OPDS_CT_NAV,
    );

    nav_entry(
        &mut w,
        "Popular Top 250",
        &format!("urn:{app_name}:explore:top"),
        "Browse a popularity-weighted list.",
        &absolute_url(public_base_url, "/opds/explore/top"),
    );

    for subject in subjects {
        nav_entry(
            &mut w,
            &subject.name,
            &format!("urn:{app_name}:explore:subject:{}", subject.slug),
            &format!("Browse popular books for {}.", subject.name),
            &absolute_url(
                public_base_url,
                &format!("/opds/explore/subject/{}", subject.slug),
            ),
        );
    }

    end_feed(&mut w);
    finish(w)
}

pub fn explore_feed(
    title: &str,
    id: &str,
    entries: &[ExploreEntry],
    pagination: &PaginationPaths,
    public_base_url: Option<&str>,
    app_name: &str,
) -> String {
    let mut w = writer();

    start_feed(&mut w, id);
    text_elem(&mut w, "title", title);
    text_elem(&mut w, "updated", &now_rfc3339());
    feed_author(&mut w, app_name);
    generator(&mut w, app_name);
    link(
        &mut w,
        "self",
        &absolute_url(public_base_url, &pagination.self_href),
        OPDS_CT_NAV,
    );
    link(
        &mut w,
        "start",
        &absolute_url(public_base_url, "/opds"),
        OPDS_CT_NAV,
    );
    link(
        &mut w,
        "up",
        &absolute_url(public_base_url, "/opds/explore"),
        OPDS_CT_NAV,
    );
    if let Some(previous_href) = &pagination.previous_href {
        link(
            &mut w,
            "previous",
            &absolute_url(public_base_url, previous_href),
            OPDS_CT_NAV,
        );
    }
    if let Some(next_href) = &pagination.next_href {
        link(
            &mut w,
            "next",
            &absolute_url(public_base_url, next_href),
            OPDS_CT_NAV,
        );
    }
    simple_elem_with_attrs(
        &mut w,
        "opensearch:totalResults",
        &[],
        &pagination.total_items.to_string(),
    );
    simple_elem_with_attrs(
        &mut w,
        "opensearch:itemsPerPage",
        &[],
        &pagination.page_size.to_string(),
    );
    simple_elem_with_attrs(
        &mut w,
        "opensearch:startIndex",
        &[],
        &pagination
            .page
            .saturating_sub(1)
            .saturating_mul(pagination.page_size)
            .to_string(),
    );

    for entry in entries {
        explore_entry(&mut w, entry, public_base_url);
    }

    end_feed(&mut w);
    finish(w)
}

// ── private helpers ────────────────────────────────────────────────────────

fn build_root_feed(public_base_url: Option<&str>, app_name: &str, archive_name: &str) -> String {
    let mut w = writer();

    start_feed(&mut w, &format!("urn:{app_name}:root"));
    text_elem(
        &mut w,
        "title",
        &format!("{app_name} — All of human knowledge. No late fees."),
    );
    text_elem(&mut w, "updated", &now_rfc3339());
    feed_author(&mut w, app_name);
    generator(&mut w, app_name);

    link(
        &mut w,
        "self",
        &absolute_url(public_base_url, "/opds"),
        OPDS_CT_NAV,
    );
    link(
        &mut w,
        "start",
        &absolute_url(public_base_url, "/opds"),
        OPDS_CT_NAV,
    );
    link(
        &mut w,
        "search",
        &absolute_url(public_base_url, "/opds/opensearch.xml"),
        OPENSEARCH_CT,
    );

    nav_entry(
        &mut w,
        "Search Books",
        &format!("urn:{app_name}:search"),
        &format!("Search {archive_name} through {app_name}."),
        &absolute_url(public_base_url, "/opds/search?q="),
    );

    nav_entry(
        &mut w,
        "Explore Books",
        &format!("urn:{app_name}:explore"),
        "Browse popular books and subjects.",
        &absolute_url(public_base_url, "/opds/explore"),
    );

    end_feed(&mut w);
    finish(w)
}

fn build_open_search_description(
    public_base_url: Option<&str>,
    app_name: &str,
    archive_name: &str,
) -> String {
    let mut w = writer();
    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .unwrap();

    let mut root = BytesStart::new("OpenSearchDescription");
    root.push_attribute(("xmlns", OPENSEARCH_NS));
    w.write_event(Event::Start(root)).unwrap();

    text_elem(&mut w, "ShortName", app_name);
    text_elem(
        &mut w,
        "Description",
        &format!("Search {archive_name} through {app_name}"),
    );
    empty_elem_with_attrs(
        &mut w,
        "Url",
        &[
            ("type", OPDS_CT_ACQ),
            (
                "template",
                &absolute_url(public_base_url, "/opds/search?q={searchTerms}"),
            ),
        ],
    );

    w.write_event(Event::End(BytesEnd::new("OpenSearchDescription")))
        .unwrap();
    finish(w)
}

fn entry(
    w: &mut Writer<Cursor<Vec<u8>>>,
    book: &BookEntry,
    public_base_url: Option<&str>,
    archive_base: &str,
) {
    w.write_event(Event::Start(BytesStart::new("entry")))
        .unwrap();

    text_elem(w, "title", &book.title);
    text_elem(w, "id", &format!("urn:md5:{}", book.md5));
    text_elem(w, "updated", &now_rfc3339());

    // Author block
    w.write_event(Event::Start(BytesStart::new("author")))
        .unwrap();
    text_elem(w, "name", &book.author);
    w.write_event(Event::End(BytesEnd::new("author"))).unwrap();
    text_elem_with_attrs(
        w,
        "summary",
        &[("type", "text")],
        &format!("Downloads: {}", book.downloads),
    );

    // Dublin Core metadata
    if let Some(year) = book.first_publish_year {
        text_elem_with_attrs(w, "dc:issued", &[], &year.to_string());
    }
    if let Some(lang) = &book.language {
        text_elem_with_attrs(w, "dc:language", &[], lang);
    }

    // Subject categories
    for subject in &book.subjects {
        let mut tag = BytesStart::new("category");
        tag.push_attribute(("term", subject.as_str()));
        tag.push_attribute(("label", subject.as_str()));
        w.write_event(Event::Empty(tag)).unwrap();
    }

    // Cover image — always emit so OPDS clients can discover covers.
    // The /opds/cover/{md5} handler enriches metadata on first request.
    {
        let thumb_url = absolute_url(
            public_base_url,
            &format!("/opds/cover/{}?size=medium", book.md5),
        );
        let mut thumb_tag = BytesStart::new("link");
        thumb_tag.push_attribute(("rel", "http://opds-spec.org/image/thumbnail"));
        thumb_tag.push_attribute(("href", thumb_url.as_str()));
        thumb_tag.push_attribute(("type", "image/jpeg"));
        w.write_event(Event::Empty(thumb_tag)).unwrap();

        let image_url = absolute_url(
            public_base_url,
            &format!("/opds/cover/{}?size=large", book.md5),
        );
        let mut tag = BytesStart::new("link");
        tag.push_attribute(("rel", "http://opds-spec.org/image"));
        tag.push_attribute(("href", image_url.as_str()));
        tag.push_attribute(("type", "image/jpeg"));
        w.write_event(Event::Empty(tag)).unwrap();
    }

    // Acquisition link
    {
        let mut tag = BytesStart::new("link");
        tag.push_attribute(("rel", "http://opds-spec.org/acquisition"));
        tag.push_attribute((
            "href",
            absolute_url(public_base_url, &format!("/opds/download/{}", book.md5)).as_str(),
        ));
        tag.push_attribute((
            "type",
            book.download_media_type
                .as_deref()
                .unwrap_or(DEFAULT_ACQ_TYPE),
        ));
        w.write_event(Event::Empty(tag)).unwrap();
    }

    {
        let mut tag = BytesStart::new("link");
        tag.push_attribute(("rel", "alternate"));
        tag.push_attribute(("href", format!("{archive_base}/md5/{}", book.md5).as_str()));
        tag.push_attribute(("type", "text/html"));
        w.write_event(Event::Empty(tag)).unwrap();
    }

    w.write_event(Event::End(BytesEnd::new("entry"))).unwrap();
}

fn explore_entry(
    w: &mut Writer<Cursor<Vec<u8>>>,
    entry: &ExploreEntry,
    public_base_url: Option<&str>,
) {
    w.write_event(Event::Start(BytesStart::new("entry")))
        .unwrap();

    text_elem(w, "title", &entry.title);
    text_elem(w, "id", &entry.id);
    text_elem(w, "updated", &now_rfc3339());

    w.write_event(Event::Start(BytesStart::new("author")))
        .unwrap();
    text_elem(w, "name", &entry.author);
    w.write_event(Event::End(BytesEnd::new("author"))).unwrap();
    text_elem_with_attrs(w, "summary", &[("type", "text")], &entry.summary);

    // Dublin Core metadata
    if let Some(year) = entry.first_publish_year {
        text_elem_with_attrs(w, "dc:issued", &[], &year.to_string());
    }

    // Subject categories
    for subject in &entry.subjects {
        let mut tag = BytesStart::new("category");
        tag.push_attribute(("term", subject.as_str()));
        tag.push_attribute(("label", subject.as_str()));
        w.write_event(Event::Empty(tag)).unwrap();
    }

    if let Some(cover_id) = entry.cover_url.as_deref().and_then(extract_cover_id) {
        let thumb_url = absolute_url(
            public_base_url,
            &format!("/opds/cover/{cover_id}?size=medium"),
        );
        let mut thumb_tag = BytesStart::new("link");
        thumb_tag.push_attribute(("rel", "http://opds-spec.org/image/thumbnail"));
        thumb_tag.push_attribute(("href", thumb_url.as_str()));
        thumb_tag.push_attribute(("type", "image/jpeg"));
        w.write_event(Event::Empty(thumb_tag)).unwrap();

        let image_url = absolute_url(
            public_base_url,
            &format!("/opds/cover/{cover_id}?size=large"),
        );
        let mut img_tag = BytesStart::new("link");
        img_tag.push_attribute(("rel", "http://opds-spec.org/image"));
        img_tag.push_attribute(("href", image_url.as_str()));
        img_tag.push_attribute(("type", "image/jpeg"));
        w.write_event(Event::Empty(img_tag)).unwrap();
    }

    let mut tag = BytesStart::new("link");
    tag.push_attribute(("rel", "subsection"));
    tag.push_attribute((
        "href",
        absolute_url(
            public_base_url,
            &format!(
                "/opds/search?q={}",
                urlencoding::encode(&entry.search_query)
            ),
        )
        .as_str(),
    ));
    tag.push_attribute(("type", OPDS_CT_ACQ));
    w.write_event(Event::Empty(tag)).unwrap();

    let mut alternate = BytesStart::new("link");
    alternate.push_attribute(("rel", "alternate"));
    alternate.push_attribute(("href", entry.alternate_url.as_str()));
    alternate.push_attribute(("type", "text/html"));
    w.write_event(Event::Empty(alternate)).unwrap();

    w.write_event(Event::End(BytesEnd::new("entry"))).unwrap();
}

fn nav_entry(w: &mut Writer<Cursor<Vec<u8>>>, title: &str, id: &str, summary: &str, href: &str) {
    let entry_tag = BytesStart::new("entry");
    w.write_event(Event::Start(entry_tag)).unwrap();

    text_elem(w, "title", title);
    text_elem(w, "id", id);
    text_elem(w, "updated", &now_rfc3339());
    text_elem_with_attrs(w, "summary", &[("type", "text")], summary);
    link(w, "subsection", href, OPDS_CT_NAV);

    w.write_event(Event::End(BytesEnd::new("entry"))).unwrap();
}

fn start_feed(w: &mut Writer<Cursor<Vec<u8>>>, id: &str) {
    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .unwrap();

    let mut feed = BytesStart::new("feed");
    feed.push_attribute(("xmlns", ATOM_NS));
    feed.push_attribute(("xmlns:opds", OPDS_NS));
    feed.push_attribute(("xmlns:dc", DC_NS));
    feed.push_attribute(("xmlns:thr", THR_NS));
    feed.push_attribute(("xmlns:opensearch", OPENSEARCH_NS));
    w.write_event(Event::Start(feed)).unwrap();

    text_elem(w, "id", id);
}

fn end_feed(w: &mut Writer<Cursor<Vec<u8>>>) {
    w.write_event(Event::End(BytesEnd::new("feed"))).unwrap();
}

fn link(w: &mut Writer<Cursor<Vec<u8>>>, rel: &str, href: &str, mime: &str) {
    let mut tag = BytesStart::new("link");
    tag.push_attribute(("rel", rel));
    tag.push_attribute(("href", href));
    tag.push_attribute(("type", mime));
    w.write_event(Event::Empty(tag)).unwrap();
}

fn text_elem(w: &mut Writer<Cursor<Vec<u8>>>, tag: &str, text: &str) {
    w.write_event(Event::Start(BytesStart::new(tag))).unwrap();
    w.write_event(Event::Text(BytesText::new(text))).unwrap();
    w.write_event(Event::End(BytesEnd::new(tag))).unwrap();
}

fn text_elem_with_attrs(
    w: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    attrs: &[(&str, &str)],
    text: &str,
) {
    let mut start = BytesStart::new(tag);
    for (key, value) in attrs {
        start.push_attribute((*key, *value));
    }
    w.write_event(Event::Start(start)).unwrap();
    w.write_event(Event::Text(BytesText::new(text))).unwrap();
    w.write_event(Event::End(BytesEnd::new(tag))).unwrap();
}

fn feed_author(w: &mut Writer<Cursor<Vec<u8>>>, app_name: &str) {
    w.write_event(Event::Start(BytesStart::new("author")))
        .unwrap();
    text_elem(w, "name", app_name);
    w.write_event(Event::End(BytesEnd::new("author"))).unwrap();
}

fn generator(w: &mut Writer<Cursor<Vec<u8>>>, app_name: &str) {
    text_elem_with_attrs(
        w,
        "generator",
        &[
            ("uri", "https://github.com/"),
            ("version", env!("CARGO_PKG_VERSION")),
        ],
        app_name,
    );
}

fn empty_elem_with_attrs(w: &mut Writer<Cursor<Vec<u8>>>, tag: &str, attrs: &[(&str, &str)]) {
    let mut start = BytesStart::new(tag);
    for (key, value) in attrs {
        start.push_attribute((*key, *value));
    }
    w.write_event(Event::Empty(start)).unwrap();
}

fn simple_elem_with_attrs(
    w: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    attrs: &[(&str, &str)],
    text: &str,
) {
    let mut start = BytesStart::new(tag);
    for (key, value) in attrs {
        start.push_attribute((*key, *value));
    }
    w.write_event(Event::Start(start)).unwrap();
    w.write_event(Event::Text(BytesText::new(text))).unwrap();
    w.write_event(Event::End(BytesEnd::new(tag))).unwrap();
}

fn writer() -> Writer<Cursor<Vec<u8>>> {
    Writer::new(Cursor::new(Vec::with_capacity(16384)))
}

fn finish(w: Writer<Cursor<Vec<u8>>>) -> String {
    let bytes = w.into_inner().into_inner();
    // quick-xml only writes valid UTF-8 from our inputs; unwrap is fine.
    String::from_utf8(bytes).expect("quick-xml produced non-UTF-8")
}

fn atom_response(xml: String) -> Response {
    (
        [(header::CONTENT_TYPE, "application/atom+xml; charset=utf-8")],
        xml,
    )
        .into_response()
}

fn absolute_url(public_base_url: Option<&str>, path: &str) -> String {
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

fn now_rfc3339() -> String {
    // chrono is heavy for one call; roll our own from SystemTime.
    // Format: 2024-01-01T00:00:00Z
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Minimal epoch → calendar conversion (no leap seconds, no timezone).
fn epoch_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86_400;

    // Rata Die algorithm
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };

    (y, mo, d, h, m, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_feed_contains_search_link() {
        let xml = build_root_feed(None, "shelfie", "Archive");
        assert!(xml.contains("shelfie — All of human knowledge. No late fees."));
        assert!(xml.contains("rel=\"search\""));
        assert!(xml.contains("/opds/opensearch.xml"));
        assert!(xml.contains("xmlns:opensearch=\"http://a9.com/-/spec/opensearch/1.1/\""));
        assert!(xml.contains("xmlns:dc=\"http://purl.org/dc/terms/\""));
        assert!(xml.contains("/opds/explore"));
    }

    #[test]
    fn search_feed_contains_entries_and_download_links() {
        let books = vec![BookEntry {
            md5: "abc123".to_owned(),
            title: "Example Book".to_owned(),
            author: "Example Author".to_owned(),
            downloads: 42,
            cover_url: Some("https://covers.openlibrary.org/b/id/12345-M.jpg".to_owned()),
            download_media_type: Some("application/epub+zip".to_owned()),
            cover_checked_at: None,
            first_publish_year: Some(1954),
            language: Some("eng".to_owned()),
            subjects: vec!["Science Fiction".to_owned(), "Fantasy".to_owned()],
            description: None,
        }];

        let xml = search_feed(
            "example query",
            &books,
            1,
            false,
            None,
            "shelfie",
            "Archive",
            "https://example.com",
        );

        assert!(xml.contains("Search: example query"));
        assert!(xml.contains("urn:md5:abc123"));
        assert!(xml.contains("/opds/download/abc123"));
        assert!(xml.contains("/opds/cover/abc123"));
        assert!(xml.contains("<opensearch:totalResults>1</opensearch:totalResults>"));
        assert!(xml.contains("<summary type=\"text\">Downloads: 42</summary>"));
        assert!(xml.contains("type=\"application/epub+zip\""));
        assert!(xml.contains("<dc:issued>1954</dc:issued>"));
        assert!(xml.contains("<dc:language>eng</dc:language>"));
        assert!(xml.contains("<category term=\"Science Fiction\""));
        assert!(xml.contains("<category term=\"Fantasy\""));
        assert!(!xml.contains("rel=\"next\""));
        assert!(!xml.contains("rel=\"previous\""));
    }

    #[test]
    fn search_feed_pagination_links() {
        let books = vec![BookEntry {
            md5: "abc123".to_owned(),
            title: "Example Book".to_owned(),
            author: "Example Author".to_owned(),
            downloads: 42,
            cover_url: None,
            download_media_type: None,
            cover_checked_at: None,
            first_publish_year: None,
            language: None,
            subjects: vec![],
            description: None,
        }];

        let xml = search_feed(
            "dune",
            &books,
            2,
            true,
            None,
            "shelfie",
            "Archive",
            "https://example.com",
        );

        assert!(xml.contains("rel=\"next\""));
        assert!(xml.contains("/opds/search?q=dune&amp;page=3"));
        assert!(xml.contains("rel=\"previous\""));
        assert!(xml.contains("/opds/search?q=dune&amp;page=1"));
    }

    #[test]
    fn open_search_description_points_to_search_template() {
        let xml =
            build_open_search_description(Some("http://localhost:7451"), "shelfie", "Archive");

        assert!(xml.contains("OpenSearchDescription"));
        assert!(xml.contains("template=\"http://localhost:7451/opds/search?q={searchTerms}\""));
    }

    #[test]
    fn explore_root_feed_contains_top_and_subject_links() {
        let xml = explore_root_feed(
            Some("http://localhost:7451"),
            &[crate::state::ExploreSubject {
                slug: "science_fiction".to_owned(),
                name: "Science Fiction".to_owned(),
            }],
            "shelfie",
            "Archive",
        );

        assert!(xml.contains("Popular Top 250"));
        assert!(xml.contains("/opds/explore/top"));
        assert!(xml.contains("/opds/explore/subject/science_fiction"));
    }
}
