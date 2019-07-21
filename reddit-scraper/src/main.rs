extern crate reqwest;
extern crate xml;

const SUBREDDIT: &str = "cableporn";

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

#[derive(Debug)]
struct FeedEntry {
    title: Option<String>,
    content: Option<String>,
    id: Option<String>
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

fn get_entries(client: &reqwest::Client, subreddit: &str, after: Option<&str>) -> Feed {
    let url = match after {
        None => {
            format!("https://www.reddit.com/r/{}/.rss", subreddit)
        }
        Some(s) => {
            format!("https://www.reddit.com/r/{}/.rss?after={}", subreddit, s)
        }
    };

    let feed = client.get(&url).send().unwrap();
    let parser = xml::EventReader::new(feed);

    let mut feed = Option::<Feed>::None;
    let mut entry = Option::<FeedEntry>::None;
    let mut state = State::Start;

    for e in parser {
        match e {
            Ok(xml::reader::XmlEvent::StartElement { name, ..}) => {
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
                    entry.as_mut().unwrap().content = Some(string);
                } else if state == State::ID {
                    entry.as_mut().unwrap().id = Some(string);
                }
            }
            Ok(_e) => {}
            Err(e) => {
                panic!(e);
            }
        }
    }

    feed.unwrap()
}

fn main() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64; rv:69.0) Gecko/20100101 Firefox/69.0"));
    let client = reqwest::Client::builder().default_headers(headers).build().unwrap();

    let feed1 = get_entries(&client, SUBREDDIT, None);
    let feed2 = get_entries(&client, SUBREDDIT, Some(&feed1.entries.last().unwrap().id.clone().unwrap()));

    println!("{:#?}", feed1);
    println!("{:#?}", feed2);
}
