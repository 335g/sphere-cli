
use chrono::{Date, Local, TimeZone};
use reqwest::{Url, Client};
use crate::utils::{URL_RADIO, USER_AGENT};
use easy_scraper::Pattern;
use regex::Regex;

#[derive(Debug)]
pub struct OnAir {
    times: u32,
    date: Date<Local>,
    url: Url
}

impl OnAir {
    pub fn date(&self) -> &Date<Local> {
        &self.date
    }
}

pub async fn get_onair() -> anyhow::Result<Vec<OnAir>> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()?;
    let doc = client
        .get(URL_RADIO)
        .send()
        .await?
        .text()
        .await?;
    let re1 = Regex::new(r"(?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2})").unwrap();
    let re2 = Regex::new(r"\d{3,}").unwrap();
    let pat = r#"
        <li class="col-sm-6">
            <div class="title">{{title}}</div>
            <time datetime="{{date}}"></time>
            <div class="movie-player">
                <iframe src="{{url}}"></iframe>
            </div>
        </li>
    "#;
    let pat = Pattern::new(pat).unwrap();
    let onairs = pat.matches(&doc)
        .into_iter()
        .filter_map(|x| {
            let title = x.get("title")?;
            let date = x.get("date")?;
            let url = x.get("url")?;

            let cap = re1.captures(date)?;
            let year = cap["y"].parse::<i32>().ok()?;
            let month = cap["m"].parse::<u32>().ok()?;
            let day = cap["d"].parse::<u32>().ok()?;
            let date = Local.ymd(year, month, day);

            let times = re2.captures(title)
                .and_then(|cap| 
                    cap.get(0).and_then(|x| 
                        x.as_str().parse::<u32>().ok()
                    )
                )?;
            
            let url = Url::parse(&format!("https:{}", url)).ok()?;

            let onair = OnAir { times, date, url };

            Some(onair)
        })
        .collect::<Vec<_>>();
    
    Ok(onairs)
}