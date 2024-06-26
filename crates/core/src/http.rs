use crate::context::Context;
use crate::helpers::decode_entities;
use crate::settings::Settings;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::cookie::CookieStore;
use reqwest::cookie::Jar;
use reqwest::{Error, Url};
use scraper::Html;
use scraper::Selector;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const AO3: &str = "https://archiveofourown.org";
const AO3_LOGIN: &str = "https://archiveofourown.org/users/login";
const AO3_FAILED_LOGIN: &str = "The password or user name you entered doesn't match our records.";
const AO3_SUCCESS_LOGIN: &str = "Successfully logged in.";
const AO3_ALREADY_LOGIN: &str = "You are already signed in.";

pub struct HttpClient {
    client: Client,
    pub logged_in: bool,
    cookie_set: bool,
    cookies: Arc<Jar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub title: String,
    pub location: String,
}

pub fn list_to_str(list: &Vec<Link>, sep: &str) -> String {
    let mut temp = Vec::new();
    for link in list {
        temp.push(link.title.clone());
    }
    temp.join(sep)
}

pub fn update_session(context: &mut Context) {
    if context.settings.ao3.remember_me {
        let url = AO3.parse::<Url>().unwrap();
        match context.client.cookies.cookies(&url) {
            Some(cookie_str) => {
                context.settings.ao3.login_cookie = Some(cookie_str.to_str().unwrap().to_string())
            }
            None => println!("No cookies available"),
        }
    }
}

pub fn test_login(res: Result<Response, Error>, cookie_set: bool) -> bool {
    let mut logged_in = cookie_set;
    match res {
        Ok(r) => {
            let text = r.text();
            match text {
                Ok(t) => {
                    if t.contains(AO3_FAILED_LOGIN) {
                        logged_in = false;
                    } else if t.contains(AO3_SUCCESS_LOGIN) || t.contains(AO3_ALREADY_LOGIN){
                        logged_in = true;
                    } else {
                        logged_in = false;
                    }
                }
                Err(e) => {
                    format!("There was an error logging in: {}", e);
                    logged_in = false;
                }
            };
        }
        Err(e) => {
            println!("{}", e)
        }
    };
    logged_in
}

pub fn scrape_inner_text(frag: &Html, select: &str) -> String {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => {
            let raw_text = el.text().collect::<Vec<_>>().join("");
            let trimmed = raw_text.trim();
            let text = decode_entities(trimmed).into_owned();
            return text;
        }
        None => {
            return "####".to_string();
        }
    };
}

pub fn scrape_login_csrf(frag: &Html) -> String {
    let token = Selector::parse(r#"form.new_user input[name="authenticity_token"]"#).unwrap();
    let input = frag.select(&token).next().unwrap();
    input.value().attr("value").unwrap().to_string()
}

pub fn scrape_kudos_csrf(frag: &Html) -> Option<&str> {
    let token = Selector::parse(r#"form#new_kudo input[name="authenticity_token"]"#).unwrap();
    let input = frag.select(&token).next();
    if let Some(input) = input {
        input.value().attr("value")
    } else {
        None
    }    
}

pub fn scrape(frag: &Html, select: &str) -> String {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => {
            let raw = el.inner_html();
            let trimmed = raw.trim();
            let clean = decode_entities(trimmed).into_owned();
            return clean;
        }
        None => {
            println!("error trying to scrape {}", select);
            return "#####".to_string();
        }
    };
}

pub fn scrape_link_list(frag: &Html, select: &str) -> Vec<Link> {
    let selector = Selector::parse(select).unwrap();
    let elems = frag.select(&selector);

    let mut results = Vec::new();
    for el in elems {
        let raw_title = el.inner_html();
        let trimmed = raw_title.trim();
        let title = decode_entities(trimmed).into_owned();
        let location = el.value().attr("href").unwrap_or("").to_string();
        results.push(Link { title, location });
    }

    results
}

pub fn scrape_link(frag: &Html, select: &str) -> Link {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => {
            let raw_title = el.inner_html();
            let trimmed = raw_title.trim();
            let title = decode_entities(trimmed).into_owned();
            let location = el.value().attr("href").unwrap_or("").to_string();
            return Link { title, location };
        }
        None => {
            return Link {
                title: "####".to_string(),
                location: "".to_string(),
            };
        }
    };
}

pub fn scrape_many(frag: &Html, select: &str) -> Vec<String> {
    let selector = Selector::parse(select).unwrap();
    let elems = frag.select(&selector);

    let mut results = Vec::new();
    for el in elems {
        results.push(el.inner_html());
    }

    results
}

pub fn scrape_many_outer(frag: &Html, select: &str) -> Vec<String> {
    let selector = Selector::parse(select).unwrap();
    let elems = frag.select(&selector);

    let mut results = Vec::new();
    for el in elems {
        results.push(el.html());
    }

    results
}

pub fn scrape_outer(frag: &Html, select: &str) -> String {
    let selector = Selector::parse(select).unwrap();
    match frag.select(&selector).next() {
        Some(el) => return el.inner_html(),
        None => return "#####".to_string(),
    };
}

pub fn scrape_inner(frag: String, select: &str) -> String {
    let html = Html::parse_fragment(&frag);
    let selector = Selector::parse(select).unwrap();
    html.select(&selector).next().unwrap().html()
}

impl HttpClient {
    pub fn new(settings: &mut Settings) -> HttpClient {
        let cookie_jar = Jar::default();
        let mut cookie_set = false;

        if settings.ao3.remember_me {
            let url = AO3.parse::<Url>().unwrap();
            match settings.ao3.clone().login_cookie {
                Some(cookie) => {
                    cookie_jar.add_cookie_str(&cookie, &url);
                    cookie_jar.add_cookie_str("user_credentials=1; path=/;", &url);
                    cookie_set = true;
                }
                _ => {}
            }
        }
        let cookies = Arc::new(cookie_jar);
        let client = Client::builder()
            .cookie_provider(cookies.clone())
            .build()
            .unwrap();

        // Note: having a user cookie set doesn't guarantee we're actually logged in
        // as the cookie may be invalid/expired.
        let res = client.get(AO3).send();
        let logged_in = test_login(res, cookie_set);
        HttpClient {
            client,
            logged_in,
            cookie_set,
            cookies,
        }
    }

    pub fn get_parse(&self, url: &str) -> Html {
        let res = self.client.get(url).send();

        match res {
            Ok(r) => {
                let text = r.text();
                match text {
                    Ok(t) => return Html::parse_document(&t),
                    Err(_e) => return Html::new_fragment(),
                };
            }
            Err(_e) => return Html::new_fragment(),
        };
    }

    pub fn get(&self, url: &str) -> RequestBuilder {
        self.client.get(url)
    }

    pub fn get_html(&self, url: &str) -> String {
        let res = self.client.get(url).send();
        match res {
            Ok(r) => {
                let text = r.text();
                match text {
                    Ok(t) => return t,
                    Err(e) => {
                        return format!(
                            "There was an error in the response body of {}:\n{}",
                            url, e
                        )
                    }
                };
            }
            Err(e) => {
                println!("{}", e);
                return format!("Error fetching {} - {}", url, e);
            }
        }
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        self.client.post(url)
    }

    pub fn test_login(&mut self) -> bool {
        let res = self.get(AO3).send();
        if !self.cookie_set {
            return false;
        } else {
            return test_login(res, self.cookie_set);
        }
    }

    pub fn login(&mut self, user: &str, password: &str) {
        let html = self.get_parse(AO3_LOGIN);
        let token = scrape_login_csrf(&html);
        let params = [
            ("user[login]", user),
            ("user[password]", password),
            ("user[remember_me]", "1"),
            ("authenticity_token", &token),
        ];

        let res = self.client.post(AO3_LOGIN).form(&params).send();
        let logged_in = test_login(res, self.cookie_set);
        self.logged_in = logged_in;
    }
}
