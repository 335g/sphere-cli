
use clap::Clap;

mod radio;
pub mod utils;

#[derive(Debug, Clap)]
struct Opts {
    #[clap(subcommand)]
    subcmd: Subcommand,
}

#[derive(Debug, Clap)]
enum Subcommand {
    #[clap(version = "0.1.0", author = "335g <actionstar619@yahoo.co.jp>")]
    Radio
}

#[tokio::main]
async fn main() {
    let opts = Opts::parse();
    
    match opts.subcmd {
        Subcommand::Radio => {
            let mut onairs = match radio::get_onair().await {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("ERROR: {:?}", e);
                    std::process::exit(1);
                }
            };
            onairs.sort_by(|a, b| b.date().cmp(a.date()));
            
            println!("{:?}", onairs);
        }
    }
}
