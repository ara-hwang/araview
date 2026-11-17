//! Allowlisted metadata for clipboard diagnostics. Never inspect or copy a log message.
//! Local logs remain detailed, but an arbitrary path, filename or panic payload must not be
//! copied merely because it appeared in a warning. Unknown targets collapse to `external`.

pub(crate) fn operation(value: &str) -> &'static str {
    match value {
        "open" => "open",
        "decode" => "decode",
        "cache" => "cache",
        "update" => "update",
        "settings" => "settings",
        "platform" => "platform",
        "operation" => "operation",
        "app" => "app",
        _ => "external",
    }
}

pub(crate) fn summary(level: &str, target: &str) -> String {
    let level = match level {
        "ERROR" => "ERROR",
        _ => "WARN",
    };
    let mut parts = target.split("::");
    if parts.next() != Some("araview") {
        return format!("{level} external (details retained only in local log)");
    }
    let scope = operation(parts.next().unwrap_or(""));
    let code = match parts.next().unwrap_or("") {
        "not_found" => "not_found",
        "permission" => "permission",
        "unsupported" => "unsupported",
        "too_large" => "too_large",
        "corrupt" => "corrupt",
        "invalid_input" => "invalid_input",
        "already_exists" => "already_exists",
        "unknown" => "unknown",
        _ => "not_recorded",
    };
    format!("{level} {scope} code={code} (details retained only in local log)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_targets_cannot_copy_paths_filenames_or_payloads() {
        for secret in [
            r"C:\Users\Alice\private.jpg",
            r"D:\client\photo.png",
            r"\\server\private\scan.tif",
            "/home/alice/secret.jpg",
            "panic payload private.png",
        ] {
            assert!(!summary("ERROR", secret).contains(secret));
            assert_eq!(
                summary("ERROR", &format!("araview::{secret}::{secret}")),
                "ERROR external code=not_recorded (details retained only in local log)"
            );
        }
    }

    #[test]
    fn operation_and_code_are_preserved_without_message_text() {
        assert_eq!(
            summary("WARN", "araview::decode::corrupt"),
            "WARN decode code=corrupt (details retained only in local log)"
        );
        assert_eq!(
            summary("ERROR", "araview::update"),
            "ERROR update code=not_recorded (details retained only in local log)"
        );
        assert!(!summary("user-controlled level", "araview::app").contains("user-controlled"));
    }
}
