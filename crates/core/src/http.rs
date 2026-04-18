use crate::ao3_metadata::Ao3Info;
use crate::context::Context;

use crate::html::{self, scrape_login_csrf};
use crate::settings::Settings;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::cookie::CookieStore;
use reqwest::cookie::Jar;
use reqwest::{Error, Url};
use serde::{Serialize, Deserialize};
use scraper::Html;
use std::sync::Arc;
use std::fs::File;

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
    credentials: Credentials
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Credentials {
    username: Option<String>,
    password: Option<String>
}

impl Default for Credentials {
    fn default() -> Self {
        Credentials {
            username: Some(String::new()),
            password: Some(String::new())
        }
    }
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
            .cookie_store(true)
            .build()
            .unwrap();

        HttpClient {
            client,
            logged_in: false,
            cookie_set,
            cookies,
            credentials: Credentials {
                username: settings.ao3.username.clone(),
                password: settings.ao3.password.clone()
            }
        }
    }

    pub fn get_parse(&self, url: &str) -> Html {
        let res = self.client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Referer", "https://archiveofourown.org/")
            .send();

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

    pub fn download_work(&self, work: Ao3Info) {

        let work_url = format!("https://archiveofourown.org/works/{}?view_adult=true", work.id);
        let work_html = self.get_parse(&work_url);
        let download_links = html::scrape_link_list(&work_html, "li.download ul li");
        for link in download_links {
            if link.title == "EPUB" {
                let mut file = File::create(work.download_name()).unwrap();
                let mut res = self.get(&link.location).send().unwrap();
                let _result = res.copy_to(&mut file);
            }
        }
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        self.client.post(url)
    }

    pub fn are_login_cookies_stale(&self) -> bool {
        if !self.cookie_set { return true; }

        let res = self.get(AO3).send();
        !self.is_logged_in(res)
    }

    pub fn is_logged_in(&self, res: Result<Response, Error>) -> bool {
        let mut logged_in = self.cookie_set;
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
                        println!("There was an error logging in: {}", e);
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
        self.logged_in = self.is_logged_in(res);
    }

    pub fn renew_login(&mut self) {
        if self.are_login_cookies_stale() {
            if let (Some(username), Some(password)) =
                (self.credentials.username.clone(), self.credentials.password.clone()) {
                self.login(&username, &password);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use reqwest::Url;
    use crate::helpers::{load_toml};

    const TEST_SETTINGS_PATH: &str = "TestSettings.toml";

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(default, rename_all = "kebab-case")]
    struct TestSettings {
        pub ao3_credentials: Credentials
    }

    impl TestSettings {
        pub fn load() -> TestSettings {
            let path = Path::new(TEST_SETTINGS_PATH);
            load_toml::<TestSettings, _>(path)
                    .map_err(|e| eprintln!("Can't open TestSettings.toml: {:#}.", e))
                    .unwrap()
        }
    }

    impl Default for TestSettings {
        fn default() -> Self {
            TestSettings {
                ao3_credentials: Default::default()
            }
        }
    }

    #[test]
    #[allow(non_snake_case)]
    fn WHEN_httpClientIsCreated_THEN_itWillStoreCookies() {
        // WHEN HttpClient is created
        let mut settings: Settings = Default::default();
        let client = HttpClient::new(&mut settings);

        // THEN it will store cookies
        let url = AO3.parse::<Url>().unwrap();

        // Note: when adding custom cookies, they must include a path
        client.cookies.add_cookie_str("fakeTestCookie=unittest; path=/;", &url);
        assert!(client.cookies.cookies(&url).unwrap().to_str().expect("test cookie").to_string().contains("fakeTestCookie=unittest"));
    }

    #[test]
    #[allow(non_snake_case)]
    fn GIVEN_settingsSetToRememberLogin_WHEN_httpClientIsCreated_THEN_itWillHaveAo3Cookies() {
        // GIVEN Settings set to remember login
        let mut settings: Settings = Default::default();
        settings.ao3.remember_me = true;
        settings.ao3.login_cookie = Some("fakeTestCookie=unittest".to_string());

        // WHEN HttpClient is created
        let client = HttpClient::new(&mut settings);

        // THEN it will have Ao3 cookies
        let url = AO3.parse::<Url>().unwrap();
        assert!(client.cookies.cookies(&url).unwrap().to_str().expect("test cookie").to_string().contains("fakeTestCookie=unittest"));
    }

    #[test]
    #[allow(non_snake_case)]
    fn WHEN_loginIsCalledWithValidLogin_THEN_clientWillBeLoggedIn() {
        // NOTE: this test is flaky due to having to hit AO3 servers
        // GIVEN remember_me is not set
        let mut settings: Settings = Default::default();
        let mut client = HttpClient::new(&mut settings);
        let test_settings: TestSettings = TestSettings::load();

        client.login(
            test_settings.ao3_credentials.username.unwrap().as_str(),
            test_settings.ao3_credentials.password.unwrap().as_str());

        // THEN client will be logged in
        assert!(client.logged_in);
    }

    #[test]
    #[allow(non_snake_case)]
    fn GIVEN_rememberMeIsNotSet_WHEN_areLoginCookiesStaleIsCalled_THEN_cookiesWillBeStale() {
        // GIVEN remember_me is not set
        let mut settings: Settings = Default::default();
        let client = HttpClient::new(&mut settings);

        // WHEN are_login_cookies_stale is called
        let stale = client.are_login_cookies_stale();

        // THEN cookies will be stale
        assert!(stale);
    }

    #[test]
    #[allow(non_snake_case)]
    fn GIVEN_firstCookieCreation_WHEN_areLoginCookiesStaleIsCalled_THEN_cookiesWillBeStale() {
        // GIVEN first cookie creation
        let mut settings: Settings = Default::default();
        settings.ao3.remember_me = true;
        let client = HttpClient::new(&mut settings);

        // WHEN are_login_cookies_stale is called
        let stale = client.are_login_cookies_stale();

        // THEN cookies will not be stale
        assert!(stale);
    }

    #[test]
    #[allow(non_snake_case)]
    fn GIVEN_recentLogin_WHEN_areLoginCookiesStaleIsCalled_THEN_cookiesWillNotBeStale() {
        // NOTE: this test is flaky due to having to hit AO3 servers
        // GIVEN recent login
        let mut settings: Settings = Default::default();
        settings.ao3.remember_me = true;
        let mut cookieCollector = HttpClient::new(&mut settings);
        let test_settings: TestSettings = TestSettings::load();
        cookieCollector.login(
            test_settings.ao3_credentials.username.unwrap().as_str(),
            test_settings.ao3_credentials.password.unwrap().as_str());

        let url = AO3.parse::<Url>().unwrap();
        let login_cookie_header = cookieCollector.cookies.cookies(&url).unwrap();
        let login_cookie = login_cookie_header.to_str().expect("login cookie");

        settings.ao3.login_cookie = Some(login_cookie.to_string());
        let client = HttpClient::new(&mut settings);

        // WHEN are_login_cookies_stale is called
        let stale = client.are_login_cookies_stale();

        // THEN cookies will not be stale
        assert!(stale);
    }

    #[test]
    #[allow(non_snake_case)]
    fn GIVEN_noUsername_WHEN_renewLoginIsCalled_THEN_clientWillNotBeLoggedIn() {
        // GIVEN no username
        let mut settings: Settings = Default::default();
        let mut client = HttpClient::new(&mut settings);

        // WHEN renew_login is called
        client.renew_login();

        // THEN client will not be logged in
        assert!(!client.logged_in);
    }

    #[test]
    #[allow(non_snake_case)]
    fn GIVEN_validUsernameAndPassword_WHEN_renewLoginIsCalled_THEN_clientWillBeLoggedIn() {
        // NOTE: this test is flaky due to having to hit AO3 servers
        // GIVEN no username
        let mut settings: Settings = Default::default();
        let test_settings: TestSettings = TestSettings::load();
        settings.ao3.username = test_settings.ao3_credentials.username;
        settings.ao3.password = test_settings.ao3_credentials.password;
        let mut client = HttpClient::new(&mut settings);

        // WHEN renew_login is called
        client.renew_login();

        // THEN client will be logged in
        assert!(client.logged_in);
    }
}
