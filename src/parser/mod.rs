use std::env;
use crate::core::Action;
mod provision;
use provision::parse_provision;

pub fn parse() -> Result<Action, String> {
    let args: Vec<String> = env::args().collect();
    let mut subcommand: &str = "";
    let mut auto_confirm: bool = false; 

    if args.len() < 2 {  
        return Err("Usage: lvq <command> [options]".to_string());  
    }  

    for i in 1..args.len() {
        let current = &args[i];
        let previous = &args[i-1];

        if subcommand.is_empty() && !current.starts_with('-') && (!previous.starts_with('-') || previous == "-y" || previous == "--auto-confirm")  {
            subcommand = current;
        }

        if current == "-y" || current == "--auto-confirm" {
            auto_confirm = true;
        }
    }
    
    let command = match subcommand {
        "provision" => parse_provision(&args)?,
        _ => return Err(format!("Unknown command: {}", subcommand)),
    };

    Ok(Action { command, auto_confirm })
}

