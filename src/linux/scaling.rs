//! Match the `XWayland` renderer scale to the compositor's logical coordinates.

use std::{
    io::{self, Read, Write},
    os::unix::{net::UnixStream, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use serde::Deserialize;

const SCALE_ENV: &str = "GPUI_X11_SCALE_FACTOR";

pub(super) fn configure_xwayland_scale() -> io::Result<()> {
    if !super::is_wayland() || std::env::var_os(SCALE_ENV).is_some() {
        return Ok(());
    }
    let Some(scale) = detect_compositor_scale() else {
        return Ok(());
    };
    // GPUI reads the override when it opens the X11 connection. Re-exec with
    // a child environment instead of unsafely mutating this process's environment;
    // setting SCALE_ENV also prevents another re-exec on the next startup.
    Err(Command::new(std::env::current_exe()?)
        .args(std::env::args_os().skip(1))
        .env(SCALE_ENV, scale.to_string())
        .exec())
}

fn detect_compositor_scale() -> Option<f32> {
    let signature = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let socket = PathBuf::from(runtime)
        .join("hypr")
        .join(signature)
        .join(".socket.sock");
    let option = query(&socket, "j/getoption xwayland:force_zero_scaling")?;
    let monitors = query(&socket, "j/monitors")?;
    compositor_scale(&option, &monitors)
}

fn query(path: &Path, request: &str) -> Option<String> {
    let mut socket = UnixStream::connect(path).ok()?;
    let timeout = Some(Duration::from_millis(300));
    socket.set_read_timeout(timeout).ok()?;
    socket.set_write_timeout(timeout).ok()?;
    socket.write_all(request.as_bytes()).ok()?;
    let mut response = String::new();
    socket.take(65_536).read_to_string(&mut response).ok()?;
    Some(response)
}

#[derive(Deserialize)]
struct Monitor {
    scale: f32,
    focused: bool,
}

fn compositor_scale(option: &str, monitors: &str) -> Option<f32> {
    let option: serde_json::Value = serde_json::from_str(option).ok()?;
    // Older Hyprland versions encode boolean configuration values as integers.
    let zero_scaling = option
        .get("bool")
        .and_then(serde_json::Value::as_bool)
        .or_else(|| match option.get("int")?.as_u64()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        })?;
    if !zero_scaling {
        // The compositor already scales XWayland surfaces in this mode.
        return Some(1.0);
    }
    let monitors: Vec<Monitor> = serde_json::from_str(monitors).ok()?;
    let scale = monitors.into_iter().find(|monitor| monitor.focused)?.scale;
    (scale.is_normal() && scale.is_sign_positive()).then_some(scale)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const MONITORS: &str = r#"[
        {"scale": 2.0, "focused": false},
        {"scale": 1.6, "focused": true}
    ]"#;

    #[rstest]
    #[case(r#"{"bool":true}"#, Some(1.6))]
    #[case(r#"{"int":1}"#, Some(1.6))]
    #[case(r#"{"bool":false}"#, Some(1.0))]
    #[case(r#"{"int":0}"#, Some(1.0))]
    #[case("{}", None)]
    #[case("invalid", None)]
    fn test_compositor_scale_respects_xwayland_scaling_policy(
        #[case] option: &str,
        #[case] expected: Option<f32>,
    ) {
        assert_eq!(compositor_scale(option, MONITORS), expected);
    }

    #[rstest]
    #[case("[]")]
    #[case(r#"[{"scale":2.0,"focused":false}]"#)]
    #[case(r#"[{"scale":0.0,"focused":true}]"#)]
    #[case(r#"[{"scale":-1.0,"focused":true}]"#)]
    #[case(r#"[{"scale":null,"focused":true}]"#)]
    #[case("invalid")]
    fn test_compositor_scale_missing_or_invalid_monitor_preserves_gpui_default(
        #[case] monitors: &str,
    ) {
        assert_eq!(compositor_scale(r#"{"bool":true}"#, monitors), None);
    }
}
