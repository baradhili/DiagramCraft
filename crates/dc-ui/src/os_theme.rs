//! OS light/dark preference detection (GitHub #52).
//!
//! eframe on Linux reports no system theme and egui's own fallback is
//! Dark, so the OS setting has to be probed explicitly (gsettings with
//! the XDG portal as fallback). Everywhere else eframe tracks the OS
//! theme natively — probing is Linux-only and [`detect`] defers to
//! `ThemePreference::System`. All mapping is plain data-in →
//! preference-out, unit-testable without a desktop; only [`detect_now`]
//! touches the environment.

use egui::ThemePreference;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// Map a `gsettings get … color-scheme` reply to a preference.
/// gsettings prints the value quoted: `'prefer-dark'`, `'prefer-light'`
/// or `'default'`. `default` (no preference) maps to **Light** — the
/// app's native Visio look — rather than egui's Dark fallback (#48).
pub fn from_gsettings(out: &str) -> Option<ThemePreference> {
    let v = out.trim().trim_matches(|c| c == '\'' || c == '"');
    match v {
        "prefer-dark" => Some(ThemePreference::Dark),
        "prefer-light" | "default" => Some(ThemePreference::Light),
        _ => None,
    }
}

/// Map the XDG portal's `org.freedesktop.appearance.color-scheme`
/// value: 1 = dark, 2 = light, anything else = no preference (→ Light).
pub fn from_portal(value: u32) -> Option<ThemePreference> {
    match value {
        1 => Some(ThemePreference::Dark),
        2 => Some(ThemePreference::Light),
        _ => Some(ThemePreference::Light),
    }
}

/// Extract the uint32 from a gdbus `ReadOne` reply, e.g. `(uint32 1,)`.
fn portal_uint(out: &str) -> Option<u32> {
    let i = out.find("uint32 ")? + "uint32 ".len();
    let rest = &out[i..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Probe the OS preference: gsettings first, the XDG portal as the
/// fallback. Returns the answering source alongside the preference so
/// callers can log *how* the mode was detected. Spawns subprocesses —
/// call at startup and from the watcher thread, never per frame.
#[cfg(target_os = "linux")]
pub fn probe_os() -> Option<(&'static str, ThemePreference)> {
    let gsettings = run_quiet(
        "gsettings",
        &["get", "org.gnome.desktop.interface", "color-scheme"],
    );
    if let Some(out) = gsettings {
        if let Some(p) = from_gsettings(&out) {
            return Some(("gsettings color-scheme", p));
        }
    }
    let portal = run_quiet(
        "gdbus",
        &[
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Settings.ReadOne",
            "org.freedesktop.appearance",
            "color-scheme",
        ],
    );
    if let Some(out) = portal {
        if let Some(v) = portal_uint(&out) {
            if let Some(p) = from_portal(v) {
                return Some(("XDG portal color-scheme", p));
            }
        }
    }
    None
}

/// One-shot startup detection. On non-Linux targets eframe follows the
/// OS theme natively, so the preference is `System` and nothing is
/// probed.
pub fn detect() -> ThemePreference {
    #[cfg(target_os = "linux")]
    {
        probe_os()
            .map(|(_, pref)| pref)
            .unwrap_or(ThemePreference::Light)
    }
    #[cfg(not(target_os = "linux"))]
    {
        ThemePreference::System
    }
}

/// Silent detection for the watcher's polling loop (its probe must not
/// print every period).
#[cfg(target_os = "linux")]
fn detect_quiet() -> ThemePreference {
    probe_os()
        .map(|(_, pref)| pref)
        .unwrap_or(ThemePreference::Light)
}

#[cfg(target_os = "linux")]
fn run_quiet(prog: &str, args: &[&str]) -> Option<String> {
    use std::io::Read;
    // Bounded wait: a hung gsettings/portal call must never block app
    // startup (or the watcher period) indefinitely.
    const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
    let mut child = std::process::Command::new(prog)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => return None,
        }
    };
    if !status.success() {
        return None;
    }
    // Replies are a few bytes; reading after exit cannot deadlock.
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Delivers OS preference changes to the UI thread (GitHub #52): a
/// background thread re-probes on a period and sends only *changes*;
/// [`Self::poll`] is drained once per frame in `App::draw`, and the
/// app asks the context for a repaint on the same period so an idle
/// UI still wakes up to pick a flip up — no restart needed.
pub struct OsThemeWatcher {
    rx: Receiver<ThemePreference>,
}

impl OsThemeWatcher {
    /// Watch the live OS setting (2 s period — GNOME switches are
    /// user-paced; sub-second reaction buys nothing). Linux only: the
    /// OS theme needs explicit probing there (#52); other targets get
    /// it natively from eframe.
    #[cfg(target_os = "linux")]
    pub fn spawn() -> Self {
        Self::spawn_with(detect_quiet, Duration::from_secs(2))
    }

    /// Testable core: run `probe` on a `period`, deliver changes.
    pub fn spawn_with(probe: fn() -> ThemePreference, period: Duration) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("dc-os-theme".into())
            .spawn(move || {
                let mut last = probe();
                loop {
                    std::thread::sleep(period);
                    let cur = probe();
                    if cur != last {
                        last = cur;
                        if tx.send(cur).is_err() {
                            return; // UI end is gone
                        }
                    }
                }
            })
            .expect("spawn os-theme watcher");
        Self { rx }
    }

    /// The latest preference change since the last poll, if any
    /// (coalesced — only the most recent pending value matters).
    pub fn poll(&self) -> Option<ThemePreference> {
        let mut last = self.rx.try_recv().ok()?;
        while let Ok(next) = self.rx.try_recv() {
            last = next;
        }
        Some(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};

    #[test]
    fn gsettings_scheme_maps_to_preference() {
        assert_eq!(
            from_gsettings("'prefer-dark'\n"),
            Some(ThemePreference::Dark)
        );
        assert_eq!(
            from_gsettings("'prefer-light'"),
            Some(ThemePreference::Light)
        );
        // "default" = no preference → the app's native light look.
        assert_eq!(from_gsettings("'default'"), Some(ThemePreference::Light));
        assert_eq!(from_gsettings("default"), Some(ThemePreference::Light));
        assert_eq!(from_gsettings(""), None);
        assert_eq!(from_gsettings("'banana'"), None);
    }

    #[test]
    fn portal_reply_maps_to_preference() {
        assert_eq!(portal_uint("(uint32 1,)"), Some(1));
        assert_eq!(portal_uint("(uint32 2,)"), Some(2));
        assert_eq!(portal_uint("(uint32 0,)"), Some(0));
        assert_eq!(portal_uint("Error ..."), None);
        assert_eq!(from_portal(1), Some(ThemePreference::Dark));
        assert_eq!(from_portal(2), Some(ThemePreference::Light));
        assert_eq!(from_portal(0), Some(ThemePreference::Light));
    }

    #[test]
    fn watcher_delivers_a_flip() {
        // 0 = light, 1 = dark — the probe reads shared state the test
        // flips mid-flight, standing in for the OS setting. CALLS
        // handshakes on the watcher's initial probe so the flip can
        // never land before `last` is seeded (a flip seen by the
        // seeding probe itself would look like "no change").
        static PROBE: AtomicU8 = AtomicU8::new(0);
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        fn probe() -> ThemePreference {
            CALLS.fetch_add(1, Ordering::SeqCst);
            if PROBE.load(Ordering::SeqCst) == 1 {
                ThemePreference::Dark
            } else {
                ThemePreference::Light
            }
        }
        let watcher = OsThemeWatcher::spawn_with(probe, Duration::from_millis(5));
        let seeded = std::time::Instant::now() + Duration::from_secs(5);
        while CALLS.load(Ordering::SeqCst) == 0 {
            assert!(
                std::time::Instant::now() < seeded,
                "watcher thread never ran its initial probe"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            watcher.poll().is_none(),
            "no change reported before the flip"
        );
        PROBE.store(1, Ordering::SeqCst);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(p) = watcher.poll() {
                assert_eq!(p, ThemePreference::Dark);
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "watcher never delivered the flip"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
