use std::{collections::BTreeSet, io::Write};

use anyhow::Result;
use chrono::Datelike;
use clap::Clap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

mod radio;
pub mod utils;
mod vimeo;

use utils::URL_RADIO;

#[derive(Debug, Clap)]
struct Opts {
    #[clap(subcommand)]
    subcmd: Subcommand,
}

#[derive(Debug, Clap)]
enum Subcommand {
    #[clap(version = "0.1.0", author = "335g <actionstar619@yahoo.co.jp>")]
    Radio,
}

fn get_or_exit<T>(res: Result<T>) -> T {
    match res {
        Ok(x) => x,
        Err(e) => {
            eprintln!("ERROR: {:?}", e);
            std::process::exit(1);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();

    match opts.subcmd {
        Subcommand::Radio => {
            let onairs = radio::get_onair().await;
            let mut onairs = get_or_exit(onairs);
            onairs.sort_by(|a, b| b.date().cmp(a.date()));

            {
                let stdout = std::io::stdout();
                let mut out_handle = stdout.lock();

                for (i, onair) in onairs.iter().enumerate() {
                    out_handle
                        .write(format!("[{}]: {}({})\n", i, onair.date(), onair.times()).as_bytes())?;
                }
                out_handle.write(b"\n")?;
                out_handle.write(
                    b"What do you want to get the contents? (Please input [0-9] or `all`)\n\n",
                )?;
            }
            
            let mut input = String::new();
            {
                let stdin = std::io::stdin();
                stdin.read_line(&mut input)?;
            }
            let inputs = input
                .trim() // 改行削除
                .split_whitespace()
                .map(|s| s.split_terminator(','))
                .flatten()
                .map(|s| s.to_string())
                .collect::<BTreeSet<_>>();

            let wanted_indexes = radio::wanted_onair_indexes(onairs.len(), inputs);
            let wanted_indexes = get_or_exit(wanted_indexes);

            let wanted_onairs = onairs
                .into_iter()
                .enumerate()
                .filter(|(i, _)| wanted_indexes.contains(i))
                .map(|(_, x)| x);

            let bars = MultiProgress::new();
            let style = ProgressStyle::default_bar()
                .template(
                    "{spinner_green} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
                )
                .progress_chars("##-");

            let mut handles = vec![];
            for onair in wanted_onairs {
                let mp3_bar = bars.add(ProgressBar::new(1));
                let mp4_bar = bars.add(ProgressBar::new(1));
                mp3_bar.set_style(style.clone());
                mp3_bar.set_message(&format!("audio: [{}]", onair.times()));
                mp4_bar.set_style(style.clone());
                mp4_bar.set_message(&format!("video:[{}]", onair.times()));
                
                let date = onair.date();
                let times = onair.times();
                let filename = format!(
                    "{:02}{:02}{:02}_{}.m4a",
                    date.year() - 2000,
                    date.month(),
                    date.day(),
                    times
                );
                
                let url = onair.url().clone();
                println!("url: {:?}", &url);
                
                let handle = tokio::spawn(async move {
                    match vimeo::get_content(url, URL_RADIO, filename, mp3_bar, mp4_bar).await {
                        Ok(_) => {
                            println!("OK");
                        }
                        Err(e) => {
                            eprintln!("ERROR: {:?}", e);
                        }
                    }
                    
                });

                handles.push(handle);
            }

            for handle in handles {
                let res = handle.await;
                println!("fin");
            }

            bars.join().unwrap();
        }
    }

    Ok(())
}
