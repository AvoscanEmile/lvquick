use std::fs::OpenOptions;
use std::io::{self, Write};
use std::process::Command;
use crate::core::Exec;
pub mod provision;

pub fn confirm_execution(exec: &mut Exec) -> Result<(), String> {
    if exec.auto_confirm {
        exec.is_allowed = true;
        return Ok(());
    }

    println!("\n--- PENDING SYSTEM CHANGES ---");
    for (i, cmd) in exec.list.iter().enumerate() {
        println!("{:2}. {}", i + 1, cmd);
    }
    println!("------------------------------");
    print!("\nExecute these commands? [Y/n]: ");
    io::stdout().flush().map_err(|e| format!("Terminal error: {e}"))?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| format!("Input error: {e}"))?;
    
    if input.trim() == "Y" {
        exec.is_allowed = true;
        Ok(())
    } else {
        exec.is_allowed = false;
        Err("Execution aborted by user.".to_string())
    }
}

pub fn apply_execution(exec: Exec) -> Result<(), String> {

    if !exec.is_allowed {
        return Err("Security Error: Attempted to apply an unauthorized execution plan.".into());
    }

    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/var/log/lvq")
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    writeln!(log, "\nFull execution plan:").ok();
    for cmd in &exec.list {
        writeln!(log, "  {cmd}").ok();
    }
    writeln!(log, "------------------------------").ok();
    writeln!(log, "\nExecution Log:").ok();

    for cmd_str in exec.list {
        writeln!(log, "INTENT: {}", cmd_str).ok();

        let status = Command::new("sh")
            .arg("-c")
            .arg(&cmd_str)
            .status()
            .map_err(|e| format!("Process error for [{}]: {}", cmd_str, e))?;

        if status.success() {
            writeln!(log, "SUCCESS: {}", cmd_str).ok();
        } else {
            writeln!(log, "FAILED: {}", cmd_str).ok();
            return Err(format!("Command failed with exit code: {:?}", status.code()));
        }
    }

    Ok(())
}
