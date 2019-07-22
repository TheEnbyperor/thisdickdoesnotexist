extern crate reqwest;
extern crate xml;
extern crate html5ever;
extern crate clap;
extern crate image;

use clap::{Arg, App};
use html5ever::tendril::TendrilSink;
use std::fmt::Debug;
use std::io::Read;
use crate::image::Pixel;

#[derive(Debug, PartialEq)]
enum State {
    Start,
    Feed,
    Entry,
    Title,
    Content,
    ID,
}

#[derive(Debug)]
struct Feed {
    entries: Vec<FeedEntry>,
}

impl Feed {
    fn new() -> Self {
        Self {
            entries: Vec::new()
        }
    }
}

struct FeedEntry {
    title: Option<String>,
    content: Option<html5ever::rcdom::RcDom>,
    id: Option<String>,
}

impl Debug for FeedEntry {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        let document = match &self.content {
            Some(c) => Some(&c.document),
            None => None
        };
        fmt.debug_struct("FeedEntry")
            .field("title", &self.title)
            .field("id", &self.id)
            .field("content.document", &document)
            .finish()
    }
}

impl FeedEntry {
    fn new() -> Self {
        Self {
            content: None,
            title: None,
            id: None,
        }
    }
}

fn get_entries(client: &reqwest::Client, subreddit: &str, after: Option<String>) -> Feed {
    let url = match after {
        None => format!("https://www.reddit.com/r/{}/.rss", subreddit),
        Some(s) => format!("https://www.reddit.com/r/{}/.rss?after={}", subreddit, s)
    };

    let feed = client.get(&url).send().unwrap();
    let parser = xml::EventReader::new(feed);

    let mut feed = Option::<Feed>::None;
    let mut entry = Option::<FeedEntry>::None;
    let mut state = State::Start;

    for e in parser {
        match e {
            Ok(xml::reader::XmlEvent::StartElement { name, .. }) => {
                if state == State::Start && name.local_name == "feed" {
                    state = State::Feed;
                    feed = Some(Feed::new());
                } else if state == State::Feed && name.local_name == "entry" {
                    state = State::Entry;
                    entry = Some(FeedEntry::new());
                } else if state == State::Entry && name.local_name == "title" {
                    state = State::Title;
                } else if state == State::Entry && name.local_name == "content" {
                    state = State::Content;
                } else if state == State::Entry && name.local_name == "id" {
                    state = State::ID;
                }
            }
            Ok(xml::reader::XmlEvent::EndElement { name }) => {
                if state == State::Feed && name.local_name == "feed" {
                    state = State::Start;
                } else if state == State::Entry && name.local_name == "entry" {
                    feed.as_mut().unwrap().entries.push(entry.unwrap());
                    entry = None;
                    state = State::Feed;
                } else if state == State::Title && name.local_name == "title" {
                    state = State::Entry;
                } else if state == State::Content && name.local_name == "content" {
                    state = State::Entry;
                } else if state == State::ID && name.local_name == "id" {
                    state = State::Entry;
                }
            }
            Ok(xml::reader::XmlEvent::Characters(string)) => {
                if state == State::Title {
                    entry.as_mut().unwrap().title = Some(string);
                } else if state == State::Content {
                    let doc = html5ever::parse_document(
                        html5ever::rcdom::RcDom::default(),
                        Default::default())
                        .from_utf8()
                        .read_from(&mut string.as_bytes())
                        .unwrap();
                    entry.as_mut().unwrap().content = Some(doc);
                } else if state == State::ID {
                    entry.as_mut().unwrap().id = Some(string);
                }
            }
            Ok(_) => {}
            Err(e) => {
                panic!(e);
            }
        }
    }

    feed.unwrap()
}


fn walk_for_img(node: &html5ever::rcdom::Handle) -> Option<String> {
    match &node.data {
        html5ever::rcdom::NodeData::Element {
            ref name,
            ref attrs,
            ..
        } => {
            if name.local.to_string() == "a" {
                for attr in attrs.borrow().iter() {
                    if attr.name.local.to_string() == "href" {
                        let mut url = attr.value.to_string();
                        if url.starts_with("https://i.redd.it/") || url.starts_with("https://i.imgur.com") {
                            return Some(url);
                        } else if url.starts_with("https://imgur.com") {
                            url = url.split("https://imgur.com").last().unwrap().to_string();
                            url = format!("https://i.imgur.com{}.jpg", url);
                            return Some(url);
                        }
                    }
                }
            }
        }
        _ => {}
    }

    for child in node.children.borrow().iter() {
        if let Some(s) = walk_for_img(child) {
            return Some(s);
        }
    }

    None
}

fn save_file(client: &reqwest::Client, url: &str, prefix: &str) -> Result<u64, String> {
    let name = match url.split('/').last() {
        Some(n) => n,
        None => return Err("No name".to_string())
    };
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(format!("./{}/{}", prefix, name)) {
        Ok(f) => f,
        Err(e) => return Err(e.to_string())
    };
    let mut resp = match client.get(url).send() {
        Ok(r) => r,
        Err(e) => return Err(e.to_string())
    };

    let mut in_buffer = Vec::new();
    let mut out_buffer = Vec::new();
    match resp.read_to_end(&mut in_buffer) {
        Ok(_) => {},
        Err(e) => return Err(e.to_string())
    }
    let in_img: image::RgbImage = match image::load_from_memory(&in_buffer) {
        Ok(i) => i,
        Err(e) => return Err(e.to_string())
    }.to_rgb();
    let mut out_img: image::RgbImage = image::ImageBuffer::from_pixel(512, 512, image::Rgb::<u8>([0, 0, 0]));

    let dim = in_img.dimensions();
    let w_gt_h = dim.0 > dim.1;
    let new_w = if w_gt_h { 512 } else { ((512 as f64 / dim.1 as f64) * dim.0 as f64) as u32 };
    let new_h = if !w_gt_h { 512 } else { ((512 as f64 /dim.0 as f64) * dim.1 as f64) as u32 };
    let new_x = (512 - new_w) / 2;
    let new_y = (512 - new_h) / 2;

    let mid_img = image::imageops::resize(&in_img, new_w, new_h, image::FilterType::CatmullRom);
    image::imageops::replace(&mut out_img, &mid_img, new_x, new_y);

    match image::jpeg::JPEGEncoder::new(&mut out_buffer)
        .encode(&out_img.into_vec(), 512, 512, image::Rgb::<u8>::COLOR_TYPE) {
        Ok(_) => {},
        Err(e) => return Err(e.to_string())
    }

    match std::io::copy(&mut out_buffer.as_slice(), &mut file) {
        Ok(c) => Ok(c),
        Err(e) => Err(e.to_string())
    }
}

fn main() {
    let matches = App::new("Reddit image downloader")
        .version("1.0")
        .author("Q Misell <q@misell.cymru>")
        .about("Downloads images from a subreddit")
        .arg(Arg::with_name("SUBREDDIT")
            .index(1)
            .help("SubReddit to fetch from")
            .required(true)
            .takes_value(true)
        )
        .arg(Arg::with_name("after")
            .help("Read posts from after the given post ID")
            .required(false)
            .short("a")
            .takes_value(true)
        )
        .arg(Arg::with_name("loc")
            .help("Location to save images")
            .required(false)
            .default_value(".")
            .index(2)
            .takes_value(true)
        )
        .get_matches();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64; rv:69.0) Gecko/20100101 Firefox/69.0"));
    let client = reqwest::Client::builder().default_headers(headers).build().unwrap();

    get_loop(match matches.value_of("after") {
        None => None,
        Some(s) => Some(s.to_string())
    }, &matches, &client)
}

fn get_loop(after: Option<String>, matches: &clap::ArgMatches, client: &reqwest::Client) {
    let mut last_id = after;

    loop {
        let feed = get_entries(&client, matches.value_of("SUBREDDIT").unwrap(), last_id);

        for e in &feed.entries {
            match walk_for_img(&e.content.as_ref().unwrap().document) {
                None => {}
                Some(i) => {
                    println!("{}, {}, {:?}", e.id.as_ref().unwrap(), i, save_file(&client, &i, matches.value_of("loc").unwrap()));
                }
            }
        }

        last_id = match feed.entries.last().as_ref() {
            Some(l) => Some(l.id.as_ref().unwrap().clone()),
            None => return
        }
    }
}
