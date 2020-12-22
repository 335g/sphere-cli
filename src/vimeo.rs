
use std::path::PathBuf;
use chrono::{Datelike, Local, Timelike};
use indicatif::ProgressBar;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{IntoUrl, Url};
use tempfile::TempDir;
use which::which;
use easy_scraper::Pattern;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::utils::USER_AGENT;
use crate::error::Error;

#[derive(Debug, Deserialize)]
struct Content {
    audio: Vec<Audio>,
    video: Vec<Video>,
}

impl Content {
    fn choose_the_best_content(self) -> Result<(Audio, Video), Error> {
        let audio = self.audio
            .into_iter()
            .next()
            .ok_or(Error::NoAudio)?;
        let video = self.video
            .into_iter()
            .fold(None, |acc, x| {
                match acc {
                    None => Some(x),
                    Some(v) => {
                        if v.height < x.height {
                            Some(x)
                        } else {
                            Some(v)
                        }
                    }
                }
            })
            .ok_or(Error::NoVideo)?;

        Ok((audio, video))
    }
}

#[derive(Debug, Deserialize)]
pub struct Audio {
    base_url: String,
    init_segment: String,
    segments: Vec<Segment>
}

impl Media for Audio {
    fn init_segment(&self) -> &String {
        &self.init_segment
    }

    fn segments(&self) -> &Vec<Segment> {
        &self.segments
    }
}

impl Audio {
    async fn get_contents<W>(&self, url: Url, writer: W, pb: ProgressBar) -> Result<(), Error>
    where
        W: AsyncWriteExt + Unpin
    {
        let (_, base_url) = self.base_url.split_at(3);
        let base_url = url.join(base_url)?;
        write_segments(self, base_url, writer, pb).await?;

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Video {
    height: f64,
    base_url: String,
    init_segment: String,
    segments: Vec<Segment>,
}

impl Media for Video {
    fn init_segment(&self) -> &String {
        &self.init_segment
    }

    fn segments(&self) -> &Vec<Segment> {
        &self.segments
    }
}

impl Video {
    async fn get_contents<W>(&self, url: Url, writer: W, pb: ProgressBar) -> Result<(), Error>
    where
        W: AsyncWriteExt + Unpin
    {
        let base_url = url.join(&format!("video/{}", self.base_url))?;
        write_segments(self, base_url, writer, pb).await?;

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Segment {
    end: f64,
    start: f64,
    url: String,
}

trait Media {
    fn init_segment(&self) -> &String;
    fn segments(&self) -> &Vec<Segment>;
}

async fn write_segments<W, M>(media: &M, base_url: Url, mut writer: W, pb: ProgressBar) -> Result<(), Error>
where
    M: Media,
    W: tokio::io::AsyncWrite + Unpin
{
    let init_segment = base64::decode(media.init_segment())?;
    writer.write_all(&init_segment).await?;
    
    for seg in media.segments() {
        let url = base_url.join(&seg.url)?;
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()?;
        let resp = client.get(url)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(Error::Network)
        }

        let bytes = resp.bytes().await?;
        writer.write_all(bytes.as_ref()).await?;
        pb.inc(1);
    }

    pb.finish_with_message("Finished");

    Ok(())
}

pub async fn get_content<U, I>(url: U, from_url: I, filename: String, mp3_bar: ProgressBar, mp4_bar: ProgressBar) -> Result<PathBuf, Error>
where
    U: IntoUrl,
    I: Into<&'static str>,
{
    // check `ffmpeg`
    let _  = which("ffmpeg")?;

    // access the url
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;
    let req = client.get(url)
        .header("referer", from_url.into())
        .build()?;
    let resp = client.execute(req)
        .await?
        .text()
        .await?;
    
    let (info_url, base_url) = {
        let pat = Pattern::new(r#"
            <body><script>{{content}}</script></body>
        "#).unwrap();
        let master_regex = Regex::new(r#""(https://[^"]+)(video)([^"]+master.json[?][^",]+)""#).unwrap();
        let map = pat.matches(&resp)
            .into_iter()
            .filter(|m| master_regex.is_match(&m["content"]))
            .next()
            .ok_or(Error::NoMasterJson)?;
        let cap = master_regex.captures(&map["content"]).unwrap();
        
        let info_url = Url::parse(&format!("{}{}{}", &cap[1], &cap[2], &cap[3]))?;
        let base_url = Url::parse(&cap[1])?;

        (info_url, base_url)
    };
    
    // next access

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;
    let content = client.get(info_url)
        .send()
        .await?
        .json::<Content>()
        .await?;
    let (audio, video) = content.choose_the_best_content()?;

    // audio & video

    let dir = TempDir::new()?;
    let now = Local::now();
    let prefix = format!("{}{}{}_{}{}{}",
        now.year(),
        now.month0(),
        now.day0(),
        now.hour(),
        now.minute(),
        now.second());

    let mp3_url = base_url.clone();
    let mp3_filepath = dir.path().join(&format!("{}.mp3", &prefix));
    mp3_bar.set_length(audio.segments.len() as u64);
    let mp3_file = tokio::fs::File::create(&mp3_filepath).await?;
    let mp3_writer = tokio::io::BufWriter::new(mp3_file);
    let mp3_handle = tokio::spawn(async move {
        audio.get_contents(mp3_url, mp3_writer, mp3_bar).await
    });

    let mp4_url = base_url;
    let mp4_filepath = dir.path().join(&format!("{}.mp4", &prefix));
    mp4_bar.set_length(video.segments.len() as u64);
    let mp4_file = tokio::fs::File::create(&mp4_filepath).await?;
    let mp4_writer = tokio::io::BufWriter::new(mp4_file);
    let mp4_handle = tokio::spawn(async move {
        video.get_contents(mp4_url, mp4_writer, mp4_bar).await
    });

    let _ = mp3_handle.await?;
    let _ = mp4_handle.await?;

    // merge

    let current_dir = std::env::current_dir()?;
    let filepath = current_dir.join(filename);
    std::process::Command::new("ffmpeg")
        .args(&[
            "-i",
            mp3_filepath.to_str().unwrap(),
            "-i",
            mp4_filepath.to_str().unwrap(),
            "-acodec",
            "copy",
            "-vcodec",
            "copy",
            filepath.to_str().unwrap()
        ])
        .output()?;
    dir.close()?;

    Ok(filepath)
}