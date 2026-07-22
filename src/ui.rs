use std::io::{self, IsTerminal, Write};
use std::sync::OnceLock;

use console::{Color, Style, style};
use dialoguer::theme::ColorfulTheme;
use serde_json::json;

/// Primary on dark terminal backgrounds (`#A9FBC0`).
pub const PRIMARY_DARK_BG: (u8, u8, u8) = (0xa9, 0xfb, 0xc0);
/// Primary on light terminal backgrounds (`#016853`).
pub const PRIMARY_LIGHT_BG: (u8, u8, u8) = (0x01, 0x68, 0x53);

const MUTED_DARK: (u8, u8, u8) = (0x9a, 0xa0, 0xb4);
const MUTED_LIGHT: (u8, u8, u8) = (0x5c, 0x63, 0x70);
const TEXT_DARK: (u8, u8, u8) = (0xe8, 0xea, 0xf0);
const TEXT_LIGHT: (u8, u8, u8) = (0x1a, 0x1d, 0x23);
const WARN: (u8, u8, u8) = (0xd9, 0x77, 0x06);
const ERROR: (u8, u8, u8) = (0xdc, 0x26, 0x26);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBg {
    Dark,
    Light,
}

impl TerminalBg {
    pub fn detect() -> Self {
        static CACHED: OnceLock<TerminalBg> = OnceLock::new();
        *CACHED.get_or_init(detect_terminal_bg)
    }

    pub fn primary(self) -> (u8, u8, u8) {
        match self {
            Self::Dark => PRIMARY_DARK_BG,
            Self::Light => PRIMARY_LIGHT_BG,
        }
    }

    fn muted(self) -> (u8, u8, u8) {
        match self {
            Self::Dark => MUTED_DARK,
            Self::Light => MUTED_LIGHT,
        }
    }

    fn text(self) -> (u8, u8, u8) {
        match self {
            Self::Dark => TEXT_DARK,
            Self::Light => TEXT_LIGHT,
        }
    }

    fn primary_color(self) -> Color {
        let (r, g, b) = self.primary();
        Color::TrueColor(r, g, b)
    }
}

fn detect_terminal_bg() -> TerminalBg {
    if let Ok(forced) = std::env::var("ETHIRA_COLOR_BG") {
        match forced.to_ascii_lowercase().as_str() {
            "light" | "white" => return TerminalBg::Light,
            "dark" | "black" => return TerminalBg::Dark,
            _ => {}
        }
    }
    // COLORFGBG is commonly `fg;bg` with 0–15 palette indexes (0–7 darker).
    if let Ok(fgbg) = std::env::var("COLORFGBG")
        && let Some(bg) = fgbg.split(';').next_back()
        && let Ok(n) = bg.trim().parse::<u8>()
    {
        return if n >= 7 {
            TerminalBg::Light
        } else {
            TerminalBg::Dark
        };
    }
    TerminalBg::Dark
}

pub fn colors_enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    io::stderr().is_terminal() || io::stdout().is_terminal()
}

fn ansi_fg((r, g, b): (u8, u8, u8), bold: bool) -> String {
    if bold {
        format!("1;38;2;{r};{g};{b}")
    } else {
        format!("38;2;{r};{g};{b}")
    }
}

fn paint_rgb(rgb: (u8, u8, u8), text: &str, bold: bool) -> String {
    if !colors_enabled() {
        return text.to_string();
    }
    format!("\x1b[{}m{text}\x1b[0m", ansi_fg(rgb, bold))
}

pub fn primary(text: &str) -> String {
    paint_rgb(TerminalBg::detect().primary(), text, false)
}

pub fn primary_bold(text: &str) -> String {
    paint_rgb(TerminalBg::detect().primary(), text, true)
}

pub fn muted(text: &str) -> String {
    paint_rgb(TerminalBg::detect().muted(), text, false)
}

pub fn warn(text: &str) -> String {
    paint_rgb(WARN, text, false)
}

pub fn error(text: &str) -> String {
    paint_rgb(ERROR, text, true)
}

pub fn success(text: &str) -> String {
    primary_bold(text)
}

/// Ethira-branded dialoguer theme (mint/teal primary adapts to terminal bg).
pub fn dialoguer_theme() -> ColorfulTheme {
    let bg = TerminalBg::detect();
    let primary = Style::new().for_stderr().fg(bg.primary_color());
    let primary_bold = Style::new().for_stderr().fg(bg.primary_color()).bold();

    ColorfulTheme {
        defaults_style: primary.clone(),
        prompt_style: Style::new().for_stderr().bold(),
        prompt_prefix: style("?".to_string()).for_stderr().fg(bg.primary_color()),
        prompt_suffix: style("›".to_string()).for_stderr().fg(bg.primary_color()),
        success_prefix: style("✔".to_string()).for_stderr().fg(bg.primary_color()),
        success_suffix: style("·".to_string()).for_stderr().dim(),
        error_prefix: style("✘".to_string()).for_stderr().red(),
        error_style: Style::new().for_stderr().red(),
        hint_style: Style::new().for_stderr().dim(),
        values_style: primary_bold.clone(),
        active_item_style: primary_bold,
        inactive_item_style: Style::new().for_stderr(),
        active_item_prefix: style("❯".to_string()).for_stderr().fg(bg.primary_color()),
        inactive_item_prefix: style(" ".to_string()).for_stderr(),
        checked_item_prefix: style("✔".to_string()).for_stderr().fg(bg.primary_color()),
        unchecked_item_prefix: style("⬚".to_string()).for_stderr().dim(),
        picked_item_prefix: style("❯".to_string()).for_stderr().fg(bg.primary_color()),
        unpicked_item_prefix: style(" ".to_string()).for_stderr(),
    }
}

/// Why the user cannot run an authenticated command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailure {
    NotLoggedIn,
    Expired,
    Rejected,
}

impl AuthFailure {
    pub fn title(self) -> &'static str {
        match self {
            Self::NotLoggedIn => "You're not signed in",
            Self::Expired => "Your session has expired",
            Self::Rejected => "Your session is no longer valid",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::NotLoggedIn => "Sign in to Ethira to use this command.",
            Self::Expired => "Your access token expired. Sign in again to continue.",
            Self::Rejected => "The API rejected your credentials. Sign in again to continue.",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::NotLoggedIn => "not_logged_in",
            Self::Expired => "session_expired",
            Self::Rejected => "session_rejected",
        }
    }
}

#[derive(Debug)]
pub struct UnauthorizedError {
    pub kind: AuthFailure,
}

impl UnauthorizedError {
    pub fn new(kind: AuthFailure) -> Self {
        Self { kind }
    }
}

impl std::fmt::Display for UnauthorizedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} — run `ethira login`", self.kind.title())
    }
}

impl std::error::Error for UnauthorizedError {}

fn pad_line(content: &str, rgb: (u8, u8, u8), bold: bool, width: usize) -> String {
    let inner = format!(" {content}");
    let visible = inner.chars().count();
    let pad = width.saturating_sub(visible);
    format!("│{}{}│", paint_rgb(rgb, &inner, bold), " ".repeat(pad))
}

fn wrap_text(text: &str, max: usize) -> Vec<String> {
    if text.chars().count() <= max {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
            continue;
        }
        if current.chars().count() + 1 + word.chars().count() <= max {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Print a friendly unauthorized panel to stderr.
pub fn print_unauthorized(kind: AuthFailure, as_json: bool) {
    if as_json {
        let _ = writeln!(
            io::stderr(),
            "{}",
            json!({
                "error": kind.code(),
                "message": kind.title(),
                "detail": kind.detail(),
                "hint": "ethira login",
            })
        );
        return;
    }

    let bg = TerminalBg::detect();
    let primary = bg.primary();
    let muted = bg.muted();
    let text = bg.text();

    let width = 46usize;
    let top = format!("╭{}╮", "─".repeat(width));
    let bot = format!("╰{}╯", "─".repeat(width));
    let blank = format!("│{}│", " ".repeat(width));

    let mut err = io::stderr().lock();
    let _ = writeln!(err);
    let _ = writeln!(err, "{}", paint_rgb(primary, &top, false));
    let _ = writeln!(err, "{blank}");
    let _ = writeln!(err, "{}", pad_line(kind.title(), primary, true, width));
    let _ = writeln!(err, "{blank}");
    for chunk in wrap_text(kind.detail(), width - 2) {
        let _ = writeln!(err, "{}", pad_line(&chunk, muted, false, width));
    }
    let _ = writeln!(err, "{blank}");
    let _ = writeln!(err, "{}", pad_line("Run  ethira login", text, true, width));
    let _ = writeln!(err, "{blank}");
    let _ = writeln!(err, "{}", paint_rgb(primary, &bot, false));
    let _ = writeln!(err);
}

/// If `err` is (or wraps) an [`UnauthorizedError`], print it prettily and return true.
pub fn maybe_print_unauthorized(err: &anyhow::Error, as_json: bool) -> bool {
    if let Some(unauth) = err.downcast_ref::<UnauthorizedError>() {
        print_unauthorized(unauth.kind, as_json);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_adapts_to_bg() {
        assert_eq!(TerminalBg::Dark.primary(), PRIMARY_DARK_BG);
        assert_eq!(TerminalBg::Light.primary(), PRIMARY_LIGHT_BG);
        assert_eq!(PRIMARY_DARK_BG, (0xa9, 0xfb, 0xc0));
        assert_eq!(PRIMARY_LIGHT_BG, (0x01, 0x68, 0x53));
    }

    #[test]
    fn paint_emits_truecolor_when_forced() {
        // colors_enabled is false under cargo test pipes; assert encoder path.
        let code = ansi_fg(PRIMARY_DARK_BG, true);
        assert_eq!(code, "1;38;2;169;251;192");
        let code = ansi_fg(PRIMARY_LIGHT_BG, false);
        assert_eq!(code, "38;2;1;104;83");
    }
}
