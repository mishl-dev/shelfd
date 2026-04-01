use quick_xml::{
    events::{BytesDecl, BytesEnd, BytesStart, Event},
    Writer,
};
use std::io::Cursor;

use crate::{
    models::{BookEntry, ExploreEntry},
    state::ExploreSubject,
};

use super::links::{absolute_url, extract_cover_id, search_page_path};
use super::xml::{
    empty_elem_with_attrs, end_feed, feed_author, finish, generator, link, now_rfc3339,
    simple_elem_with_attrs, start_feed, text_elem, text_elem_with_attrs, writer, DEFAULT_ACQ_TYPE,
    OPDS_CT_ACQ, OPDS_CT_NAV, OPENSEARCH_CT, OPENSEARCH_NS,
};

pub struct PaginationPaths {
    pub self_href: String,
    pub next_href: Option<String>,
    pub previous_href: Option<String>,
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
}

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

pub fn explore_root_feed(
    public_base_url: Option<&str>,
    self_href: &str,
    subjects: &[ExploreSubject],
    app_name: &str,
    archive_name: &str,
) -> String {
    navigation_feed(
        public_base_url,
        self_href,
        subjects,
        app_name,
        "Explore Books",
        Some(&format!("Browse {} through {}.", archive_name, app_name)),
    )
}

pub(crate) fn root_navigation_feed(
    public_base_url: Option<&str>,
    self_href: &str,
    subjects: &[ExploreSubject],
    app_name: &str,
    archive_name: &str,
) -> String {
    navigation_feed(
        public_base_url,
        self_href,
        subjects,
        app_name,
        app_name,
        Some(&format!(
            "Browse and search {} through {}.",
            archive_name, app_name
        )),
    )
}

fn navigation_feed(
    public_base_url: Option<&str>,
    self_href: &str,
    subjects: &[ExploreSubject],
    app_name: &str,
    feed_title: &str,
    subtitle: Option<&str>,
) -> String {
    let mut w = writer();

    start_feed(&mut w, &format!("urn:{app_name}:explore"));
    text_elem(&mut w, "title", feed_title);
    if let Some(subtitle) = subtitle {
        text_elem(&mut w, "subtitle", subtitle);
    }
    text_elem(&mut w, "updated", &now_rfc3339());
    feed_author(&mut w, app_name);
    generator(&mut w, app_name);

    link(
        &mut w,
        "self",
        &absolute_url(public_base_url, self_href),
        OPDS_CT_NAV,
    );
    link(
        &mut w,
        "search",
        &absolute_url(public_base_url, "/opds/opensearch.xml"),
        OPENSEARCH_CT,
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

pub(crate) fn build_open_search_description(
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

    if let Some(year) = book.first_publish_year {
        text_elem_with_attrs(w, "dc:issued", &[], &year.to_string());
    }
    if let Some(lang) = &book.language {
        text_elem_with_attrs(w, "dc:language", &[], lang);
    }

    for subject in &book.subjects {
        let mut tag = BytesStart::new("category");
        tag.push_attribute(("term", subject.as_str()));
        tag.push_attribute(("label", subject.as_str()));
        w.write_event(Event::Empty(tag)).unwrap();
    }

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

    let mut tag = BytesStart::new("link");
    tag.push_attribute(("rel", "alternate"));
    tag.push_attribute(("href", format!("{archive_base}/md5/{}", book.md5).as_str()));
    tag.push_attribute(("type", "text/html"));
    w.write_event(Event::Empty(tag)).unwrap();

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

    if let Some(year) = entry.first_publish_year {
        text_elem_with_attrs(w, "dc:issued", &[], &year.to_string());
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explore_root_feed_is_default_navigation() {
        let xml = explore_root_feed(
            None,
            "/opds/explore",
            &[crate::state::ExploreSubject {
                slug: "science_fiction".to_owned(),
                name: "Science Fiction".to_owned(),
            }],
            "shelfd",
            "Archive",
        );
        assert!(xml.contains("Explore Books"));
        assert!(xml.contains("Browse Archive through shelfd."));
        assert!(xml.contains("rel=\"search\""));
        assert!(xml.contains("/opds/opensearch.xml"));
        assert!(xml.contains("xmlns:opensearch=\"http://a9.com/-/spec/opensearch/1.1/\""));
        assert!(xml.contains("xmlns:dc=\"http://purl.org/dc/terms/\""));
        assert!(xml.contains("/opds/explore/top"));
        assert!(xml.contains("/opds/explore/subject/science_fiction"));
        assert!(!xml.contains("Search Books"));
    }

    #[test]
    fn root_feed_uses_app_branding() {
        let xml = root_navigation_feed(
            None,
            "/opds",
            &[crate::state::ExploreSubject {
                slug: "science_fiction".to_owned(),
                name: "Science Fiction".to_owned(),
            }],
            "shelfd",
            "Archive",
        );

        assert!(xml.contains("<title>shelfd</title>"));
        assert!(xml.contains("Browse and search Archive through shelfd."));
        assert!(xml.contains("/opds/explore/top"));
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
            "shelfd",
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
            "shelfd",
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
        let xml = build_open_search_description(Some("http://localhost:7451"), "shelfd", "Archive");

        assert!(xml.contains("OpenSearchDescription"));
        assert!(xml.contains("template=\"http://localhost:7451/opds/search?q={searchTerms}\""));
    }

    #[test]
    fn explore_root_feed_contains_top_and_subject_links() {
        let xml = explore_root_feed(
            Some("http://localhost:7451"),
            "/opds/explore",
            &[crate::state::ExploreSubject {
                slug: "science_fiction".to_owned(),
                name: "Science Fiction".to_owned(),
            }],
            "shelfd",
            "Archive",
        );

        assert!(xml.contains("Popular Top 250"));
        assert!(xml.contains("/opds/explore/top"));
        assert!(xml.contains("/opds/explore/subject/science_fiction"));
    }
}
