//! The Windows half of running a Core with the privileges a TUN device needs.
//!
//! Only the pure parts are covered here — the command lines and how Windows
//! answers them — because they are the parts that can be checked on any
//! machine. The UAC prompt itself is exercised by nothing.

use magies_core_runtime::elevated::{
    ElevatedCoreError, parse_tasklist_output, runas_error, taskkill_arguments, tasklist_arguments,
    windows_elevation_script,
};

#[test]
fn the_elevated_script_starts_the_core_and_records_what_it_started() {
    let script = windows_elevation_script(
        std::path::Path::new(r"C:\Cores\sing-box.exe"),
        std::path::Path::new(r"C:\Run\session.json"),
        std::path::Path::new(r"C:\Run\core.pid"),
        std::path::Path::new(r"C:\Run\core.log"),
    );

    // The Core's own log goes to stderr, so that is the stream the panel
    // follows; stdout is kept separately rather than merged, because
    // Start-Process refuses to point both at one file.
    assert!(
        script.contains(r"-FilePath 'C:\Cores\sing-box.exe'"),
        "{script}"
    );
    assert!(
        script.contains(r"'run','-c','C:\Run\session.json'"),
        "{script}"
    );
    assert!(
        script.contains(r"-RedirectStandardError 'C:\Run\core.log'"),
        "{script}"
    );
    assert!(
        script.contains(r"-RedirectStandardOutput 'C:\Run\core.log.out'"),
        "{script}"
    );
    assert!(script.contains(r"'C:\Run\core.pid'"), "{script}");
    assert!(script.contains("-WindowStyle Hidden"), "{script}");
}

#[test]
fn a_path_with_a_quote_cannot_break_out_of_the_powershell_script() {
    let script = windows_elevation_script(
        std::path::Path::new(r"C:\Cores\sing'box.exe"),
        std::path::Path::new(r"C:\Run\session.json"),
        std::path::Path::new(r"C:\Run\core.pid"),
        std::path::Path::new(r"C:\Run\core.log"),
    );

    // PowerShell escapes a single quote by doubling it.
    assert!(
        script.contains(r"-FilePath 'C:\Cores\sing''box.exe'"),
        "{script}"
    );
}

#[test]
fn stopping_asks_politely_before_it_forces() {
    // Without a graceful stop first, sing-box never runs its own cleanup, and
    // on Windows that cleanup is what takes the TUN routes back out.
    assert_eq!(taskkill_arguments(4321, false), vec!["/PID", "4321", "/T"]);
    assert_eq!(
        taskkill_arguments(4321, true),
        vec!["/PID", "4321", "/T", "/F"]
    );
}

#[test]
fn liveness_reads_the_pid_back_out_of_tasklist() {
    assert_eq!(
        tasklist_arguments(4321),
        vec!["/FI", "PID eq 4321", "/NH", "/FO", "CSV"]
    );

    // tasklist answers a filter that matches nothing with a sentence, not an
    // empty result, so a substring check on the PID would be a false positive
    // waiting to happen.
    assert!(parse_tasklist_output(
        "\"sing-box.exe\",\"4321\",\"Console\",\"1\",\"52,000 K\"\r\n",
        4321
    ));
    assert!(!parse_tasklist_output(
        "INFO: No tasks are running which match the specified criteria.\r\n",
        4321
    ));
    assert!(!parse_tasklist_output("", 4321));
    // A different process whose memory figure happens to contain the digits.
    assert!(!parse_tasklist_output(
        "\"other.exe\",\"77\",\"Console\",\"1\",\"4,321 K\"\r\n",
        4321
    ));
}

#[test]
fn a_cancelled_uac_prompt_is_the_same_refusal_the_other_platforms_report() {
    // PowerShell surfaces a declined UAC prompt as this exact message.
    assert_eq!(
        runas_error("The operation was canceled by the user.").code(),
        "tun_authorization_declined"
    );
    assert!(matches!(
        runas_error("something else went wrong"),
        ElevatedCoreError::LaunchRejected { .. }
    ));
}
