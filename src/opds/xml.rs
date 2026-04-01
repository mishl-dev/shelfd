use chrono::Utc;
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};
use std::io::Cursor;

pub const ATOM_NS: &str = "http://www.w3.org/2005/Atom";
pub const OPDS_NS: &str = "http://opds-spec.org/2010/catalog";
pub const DC_NS: &str = "http://purl.org/dc/terms/";
pub const THR_NS: &str = "http://purl.org/syndication/thread/1.0";
pub const OPENSEARCH_NS: &str = "http://a9.com/-/spec/opensearch/1.1/";
pub const OPDS_CT_NAV: &str = "application/atom+xml;profile=opds-catalog;kind=navigation";
pub const OPDS_CT_ACQ: &str = "application/atom+xml;profile=opds-catalog;kind=acquisition";
pub const OPENSEARCH_CT: &str = "application/opensearchdescription+xml";
pub const DEFAULT_ACQ_TYPE: &str = "application/octet-stream";

pub fn start_feed(w: &mut Writer<Cursor<Vec<u8>>>, id: &str) {
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

pub fn end_feed(w: &mut Writer<Cursor<Vec<u8>>>) {
    w.write_event(Event::End(BytesEnd::new("feed"))).unwrap();
}

pub fn link(w: &mut Writer<Cursor<Vec<u8>>>, rel: &str, href: &str, mime: &str) {
    let mut tag = BytesStart::new("link");
    tag.push_attribute(("rel", rel));
    tag.push_attribute(("href", href));
    tag.push_attribute(("type", mime));
    w.write_event(Event::Empty(tag)).unwrap();
}

pub fn text_elem(w: &mut Writer<Cursor<Vec<u8>>>, tag: &str, text: &str) {
    w.write_event(Event::Start(BytesStart::new(tag))).unwrap();
    w.write_event(Event::Text(BytesText::new(text))).unwrap();
    w.write_event(Event::End(BytesEnd::new(tag))).unwrap();
}

pub fn text_elem_with_attrs(
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

pub fn feed_author(w: &mut Writer<Cursor<Vec<u8>>>, app_name: &str) {
    w.write_event(Event::Start(BytesStart::new("author")))
        .unwrap();
    text_elem(w, "name", app_name);
    w.write_event(Event::End(BytesEnd::new("author"))).unwrap();
}

pub fn generator(w: &mut Writer<Cursor<Vec<u8>>>, app_name: &str) {
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

pub fn empty_elem_with_attrs(w: &mut Writer<Cursor<Vec<u8>>>, tag: &str, attrs: &[(&str, &str)]) {
    let mut start = BytesStart::new(tag);
    for (key, value) in attrs {
        start.push_attribute((*key, *value));
    }
    w.write_event(Event::Empty(start)).unwrap();
}

pub fn simple_elem_with_attrs(
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

pub fn writer() -> Writer<Cursor<Vec<u8>>> {
    Writer::new(Cursor::new(Vec::with_capacity(16384)))
}

pub fn finish(w: Writer<Cursor<Vec<u8>>>) -> String {
    let bytes = w.into_inner().into_inner();
    String::from_utf8(bytes).expect("quick-xml produced non-UTF-8")
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
