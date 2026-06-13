mod cli;

use anyhow::Result;
use clap::Parser;
use cli::{Args, Command};
use simple_parser::{BuildTarget, build_entity, parse_action, parse_event};

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Action { input, json, build } => {
            let draft = parse_action(&input)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&draft)?);
            } else {
                println!("{draft:#?}");
            }

            if build {
                let entity = build_entity(&draft, BuildTarget::Action)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&entity)?);
                } else {
                    println!("{entity:#?}");
                }
            }
        }

        Command::Event { input, json, build } => {
            let draft = parse_event(&input)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&draft)?);
            } else {
                println!("{draft:#?}");
            }

            if build {
                let entity = build_entity(&draft, BuildTarget::Event)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&entity)?);
                } else {
                    println!("{entity:#?}");
                }
            }
        }
    }

    Ok(())
}
