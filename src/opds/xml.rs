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
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn epoch_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86_400;

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
