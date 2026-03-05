mod core;
mod parser;
mod planner;
mod verifier;
mod exec;

use std::process::{self, Command};
use crate::core::DraftStatus;

fn main() {
    // 1. Identity Check (Must be root/sudo to interact with LVM and /var/log/lvq)
    if !is_root() {
        eprintln!("Error: This operation requires administrative privileges (sudo).");
        process::exit(1);
    }

    // 2. Parse CLI arguments into an Action
    let action = match parser::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Parse Error: {e}");
            process::exit(1);
        }
    };

    // 3. Generate the Pending Draft
    let mut draft = match planner::plan(action) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Planning Error: {e}");
            process::exit(1);
        }
    };

    // 4. Verify System State (Transition Pending -> Ready/Done/Dirty/Invalid)
    draft = match verifier::verify(draft) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Verification Failed: {e}");
            // Match exit codes to your requirements: 4 for Dirty, 1 for Invalid
            process::exit(1);
        }
    };

    // Handle high-level verification results
    match draft.status {
        DraftStatus::Done => {
            println!("System is already in the desired state. No changes needed.");
            process::exit(0);
        }
        DraftStatus::Ready => {
            // Proceed to execution
        }
        _ => {
            eprintln!("Architectural Error: Verification returned unexpected status {:?}.", draft.status);
            process::exit(1);
        }
    }

    // 5. Translate Ready Draft to Instruction Set (Unauthorized Exec)
    let mut exec = match exec::provision::exec_provision(draft.clone()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Execution Preparation Error: {e}");
            process::exit(1);
        }
    };

    // 6. User Authorization Gate
    if let Err(e) = exec::confirm_execution(&mut exec) {
        eprintln!("{e}");
        process::exit(0); // Exit gracefully on user abort
    }

    // 7. Execution and Audit Logging
    if let Err(e) = exec::apply_execution(exec) {
        eprintln!("Execution Failed: {e}");
        eprintln!("Check /var/log/lvq for forensic details.");
        process::exit(1);
    }

    // 8. Final State Convergence Check
    println!("Verifying system state post-execution...");
    match verifier::verify(draft) {
        Ok(final_draft) if final_draft.status == DraftStatus::Done => {
            println!("SUCCESS: System state successfully converged to target.");
            process::exit(0);
        }
        _ => {
            eprintln!("CRITICAL: Execution completed, but system state does not match target.");
            eprintln!("Check /var/log/lvq and run 'lvq verify' to debug.");
            process::exit(4); // Exit with 'Dirty' status code
        }
    }
}

/// Simple check for root privileges
fn is_root() -> bool {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .expect("Failed to execute id command");
    
    let uid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    uid_str == "0"
}
