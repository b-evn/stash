#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Text,
    Link,
    Email,
    Code,
    Path,
    Color,
    Number,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Link => "link",
            Kind::Email => "email",
            Kind::Code => "code",
            Kind::Path => "path",
            Kind::Color => "color",
            Kind::Number => "number",
        }
    }
}

pub fn detect(text: &str) -> Kind {
    let t = text.trim();
    let single_token = !t.is_empty() && !t.chars().any(char::is_whitespace);

    if single_token {
        let lower = t.to_ascii_lowercase();
        if ["http://", "https://", "ftp://"].iter().any(|p| lower.starts_with(p))
            || (lower.starts_with("www.") && lower[4..].contains('.'))
        {
            return Kind::Link;
        }
        if is_email(t) {
            return Kind::Email;
        }
    }
    if is_color(t) {
        return Kind::Color;
    }
    if is_number(t) {
        return Kind::Number;
    }
    if is_path(t) {
        return Kind::Path;
    }
    if code_signals(t) >= 2 {
        return Kind::Code;
    }
    Kind::Text
}

fn is_email(t: &str) -> bool {
    let mut parts = t.splitn(2, '@');
    match (parts.next(), parts.next()) {
        (Some(user), Some(host)) => {
            !user.is_empty()
                && !host.contains('@')
                && host.contains('.')
                && !host.starts_with('.')
                && !host.ends_with('.')
        }
        _ => false,
    }
}

fn is_color(t: &str) -> bool {
    if let Some(hex) = t.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    let lower = t.to_ascii_lowercase();
    ["rgb(", "rgba(", "hsl(", "hsla("].iter().any(|p| lower.starts_with(p)) && lower.ends_with(')')
}

fn is_number(t: &str) -> bool {
    t.len() <= 24
        && t.chars().any(|c| c.is_ascii_digit())
        && t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | '_'))
}

fn is_path(t: &str) -> bool {
    if t.contains('\n') || t.len() > 500 {
        return false;
    }
    let b = t.as_bytes();
    let drive = b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
    let unc = t.starts_with("\\\\") && t.len() > 3;
    let unix = (t.starts_with('/') || t.starts_with("~/")) && t.len() > 2 && !t.contains(' ') && t[1..].contains('/');
    drive || unc || unix
}

fn code_signals(t: &str) -> usize {
    const KEYWORDS: [&str; 14] = [
        "function ", "def ", "class ", "import ", "return ", "const ", "var ", "fn ", "select ", "from ",
        "#include", "public ", "async ", "=> ",
    ];
    let lower = t.to_ascii_lowercase();
    let multiline = t.contains('\n');
    let mut n = 0;
    if t.contains('{') || t.contains('}') {
        n += 1;
    }
    if t.contains("();") || t.contains(");") || t.lines().any(|l| l.trim_end().ends_with(';')) {
        n += 1;
    }
    if t.contains("=>") || t.contains("->") || t.contains("::") || t.contains("</") || t.contains("/>") {
        n += 1;
    }
    if multiline && t.lines().any(|l| l.starts_with("    ") || l.starts_with('\t')) {
        n += 1;
    }
    if KEYWORDS.iter().any(|k| lower.contains(k)) && (multiline || t.contains('(') || t.contains('=')) {
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_basics() {
        assert_eq!(detect("https://example.com/a?b=1"), Kind::Link);
        assert_eq!(detect("www.example.com"), Kind::Link);
        assert_eq!(detect("  me@site.dev "), Kind::Email);
        assert_eq!(detect("#1a2b3c"), Kind::Color);
        assert_eq!(detect("rgb(1, 2, 3)"), Kind::Color);
        assert_eq!(detect("1,234.50"), Kind::Number);
        assert_eq!(detect(r"C:\Users\me\file.txt"), Kind::Path);
        assert_eq!(detect("/usr/local/bin"), Kind::Path);
        assert_eq!(detect("see you at 5, bring the thing"), Kind::Text);
    }

    #[test]
    fn detects_code() {
        assert_eq!(detect("fn main() {\n    println!(\"hi\");\n}"), Kind::Code);
        assert_eq!(detect("const x = (a) => a + 1;"), Kind::Code);
        assert_eq!(detect("let me know if that works"), Kind::Text);
        assert_eq!(detect("Hello, world! How are you?"), Kind::Text);
    }
}
