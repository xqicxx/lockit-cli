//! Shell escaping utilities for safe output formatting.
//!
//! Provides the [`shell_quote`] function to safely escape strings for use in
//! POSIX shell commands. This module focuses on preventing shell injection
//! vulnerabilities by properly quoting and escaping special characters.

/// Quote a string for safe use in a POSIX shell command.
///
/// Uses single-quote style, which prevents all shell interpretation except
/// for the single quote character itself, which is escaped as `'\''`.
///
/// # Examples
///
/// ```ignore
/// // lockit-cli is a binary crate; run `cargo test --package lockit-cli` for tests.
/// // No quoting needed for safe strings:
/// //   shell_quote("simple")           → "simple"
/// //   shell_quote("hello world")      → "'hello world'"
/// //   shell_quote("$(rm -rf /)")      → "'$(rm -rf /)'"
/// //   shell_quote("it's")             → "'it'\\''s'"
/// //   shell_quote("")                 → "''"
/// ```
pub fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }

    // Check if quoting is needed
    let needs_quote = s.chars().any(|c| {
        matches!(
            c,
            ' ' | '\t'
                | '"'
                | '\''
                | '\\'
                | '$'
                | '`'
                | '!'
                | '\n'
                | ';'
                | '|'
                | '&'
                | '<'
                | '>'
                | '('
                | ')'
                | '*'
                | '?'
                | '['
                | ']'
                | '#'
                | '~'
        )
    });

    if needs_quote {
        // Single-quote the string, escaping internal single quotes as '\''
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that shell injection attempts are properly neutralized
    #[test]
    fn test_command_injection_prevention() {
        // Command substitution
        assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
        assert_eq!(shell_quote("$(cat /etc/passwd)"), "'$(cat /etc/passwd)'");

        // Backtick command substitution
        assert_eq!(shell_quote("`rm -rf /`"), "'`rm -rf /`'");
        assert_eq!(shell_quote("`cat /etc/passwd`"), "'`cat /etc/passwd`'");

        // Semicolon chaining
        assert_eq!(shell_quote("; rm -rf /"), "'; rm -rf /'");
        assert_eq!(shell_quote("ls; cat /etc/passwd"), "'ls; cat /etc/passwd'");

        // Pipe redirection
        assert_eq!(shell_quote("| cat /etc/passwd"), "'| cat /etc/passwd'");
        assert_eq!(shell_quote("foo | bar"), "'foo | bar'");

        // AND/OR chaining
        assert_eq!(shell_quote("&& whoami"), "'&& whoami'");
        assert_eq!(shell_quote("|| cat /etc/shadow"), "'|| cat /etc/shadow'");
    }

    /// Test that environment variable injection is prevented
    #[test]
    fn test_env_injection_prevention() {
        assert_eq!(shell_quote("$PATH"), "'$PATH'");
        assert_eq!(shell_quote("${PATH}"), "'${PATH}'");
        assert_eq!(shell_quote("$HOME/.ssh"), "'$HOME/.ssh'");
        assert_eq!(shell_quote("${IFS}x${IFS}"), "'${IFS}x${IFS}'");
    }

    /// Test single quote escaping edge cases
    #[test]
    fn test_single_quote_escaping() {
        // Simple case - "it's" becomes 'it'\''s'
        assert_eq!(shell_quote("it's"), "'it'\\''s'");

        // Single quote at end - "foo'" becomes 'foo'\'''
        // This is POSIX-correct: close quote, add escaped quote, empty quote
        assert_eq!(shell_quote("foo'"), "'foo'\\'''");

        // Single quote at start - "'foo" becomes ''\''foo'
        assert_eq!(shell_quote("'foo"), "''\\''foo'");

        // Multiple single quotes - each ' becomes '\'' inside single quotes
        assert_eq!(shell_quote("'''"), "''\\'''\\'''\\'''");

        // Mixed content
        assert_eq!(
            shell_quote("it's a test's test"),
            "'it'\\''s a test'\\''s test'"
        );
    }

    /// Test boundary cases
    #[test]
    fn test_boundary_cases() {
        // Empty string
        assert_eq!(shell_quote(""), "''");

        // Safe string - no quoting needed
        assert_eq!(shell_quote("simple"), "simple");
        assert_eq!(shell_quote("hello"), "hello");
        assert_eq!(shell_quote("foo_bar-baz"), "foo_bar-baz");
        assert_eq!(shell_quote("path/to/file"), "path/to/file");

        // Spaces trigger quoting
        assert_eq!(shell_quote("hello world"), "'hello world'");

        // Tabs trigger quoting
        assert_eq!(shell_quote("hello\tworld"), "'hello\tworld'");

        // Newlines trigger quoting
        assert_eq!(shell_quote("hello\nworld"), "'hello\nworld'");
    }

    /// Test special shell characters
    #[test]
    fn test_special_characters() {
        // Glob characters
        assert_eq!(shell_quote("*"), "'*'");
        assert_eq!(shell_quote("?"), "'?'");
        assert_eq!(shell_quote("[abc]"), "'[abc]'");

        // Redirection
        assert_eq!(shell_quote(">"), "'>'");
        assert_eq!(shell_quote("<"), "'<'");
        assert_eq!(shell_quote(">>"), "'>>'");

        // Background
        assert_eq!(shell_quote("&"), "'&'");

        // Home expansion
        assert_eq!(shell_quote("~"), "'~'");
        assert_eq!(shell_quote("~/path"), "'~/path'");

        // Comments
        assert_eq!(shell_quote("#comment"), "'#comment'");

        // Parentheses
        assert_eq!(shell_quote("(cmd)"), "'(cmd)'");
    }

    /// Test that quoted strings don't have special meaning
    #[test]
    fn test_no_special_meaning_in_quotes() {
        // These should all be safely quoted, not interpreted
        assert_eq!(shell_quote("foo bar baz"), "'foo bar baz'");
        assert_eq!(shell_quote("path with spaces"), "'path with spaces'");
        assert_eq!(
            shell_quote("value with $pecial char$"),
            "'value with $pecial char$'"
        );
    }

    /// Test realistic use cases
    #[test]
    fn test_realistic_use_cases() {
        // API keys with special characters
        assert_eq!(
            shell_quote("sk-proj-abc123xyz"),
            "sk-proj-abc123xyz" // Safe, no quoting needed
        );

        // Database URLs - `:` is not a shell special char, so no quoting needed
        assert_eq!(
            shell_quote("postgres://user:pass@host:5432/db"),
            "postgres://user:pass@host:5432/db"
        );

        // JWT tokens (base64-like, usually safe)
        assert_eq!(
            shell_quote("eyJhbGciOiJIUzI1NiJ9.eyJs"),
            "eyJhbGciOiJIUzI1NiJ9.eyJs" // Safe, no special chars
        );

        // Password with special chars
        assert_eq!(
            shell_quote("P@ssw0rd!#$"),
            "'P@ssw0rd!#$'" // ! triggers quoting
        );
    }
}
