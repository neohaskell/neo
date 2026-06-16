use crate::errors::NeoError;
use crate::output::OutputMode;
use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    Router,
};
use crossterm::style::Stylize;
use rust_embed::RustEmbed;
use std::net::{IpAddr, SocketAddr};

/// All files under `assets/ide/dist/` are embedded into the binary at
/// compile time in release builds. In debug builds `rust-embed` reads from
/// disk on each request (live reload during frontend iteration).
#[derive(RustEmbed)]
#[folder = "assets/ide/dist/"]
struct IdeAssets;

pub async fn run(host: IpAddr, port: u16, output_mode: &mut OutputMode) -> miette::Result<()> {
    let addr = SocketAddr::new(host, port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| NeoError::IdeBind { host, port, source: e })?;

    let bound = listener
        .local_addr()
        .map_err(|e| NeoError::IdeBind { host, port, source: e })?;

    let browseable = browseable_url(host, bound);
    let bind_hint = host
        .is_unspecified()
        .then_some("reachable from any interface on this host");

    print_startup(output_mode, &browseable, &bound.to_string(), bind_hint);

    let app = Router::new().fallback(serve_asset);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| NeoError::IdeServe { source: e })?;

    print_shutdown(output_mode);
    Ok(())
}

/// Build the URL a human can click. When the bind host is unspecified
/// (`0.0.0.0` / `::`), `http://0.0.0.0:PORT` is not browseable — substitute
/// the matching loopback address (`127.0.0.1` for v4, `[::1]` for v6) so
/// the URL we print is always clickable.
fn browseable_url(host: IpAddr, bound: SocketAddr) -> String {
    if host.is_unspecified() {
        match host {
            IpAddr::V4(_) => format!("http://127.0.0.1:{}", bound.port()),
            IpAddr::V6(_) => format!("http://[::1]:{}", bound.port()),
        }
    } else {
        // SocketAddr's Display brackets v6 addresses, so this is correct for both.
        format!("http://{bound}")
    }
}

fn print_startup(output_mode: &OutputMode, url: &str, bind: &str, bind_hint: Option<&str>) {
    if output_mode.is_ci() {
        println!("[info] Neo IDE starting");
        println!("[info]   url   {url}");
        match bind_hint {
            Some(hint) => println!("[info]   bind  {bind}  ({hint})"),
            None => println!("[info]   bind  {bind}"),
        }
        println!("[info] press Ctrl+C to stop the server");
    } else {
        // Interactive: plain `println!` with crossterm SGR styling. No ratatui
        // viewport, no raw-mode terminal takeover — the command is a long-
        // running server that has no business stealing the user's terminal.
        // The output stays scrollable, redirect-friendly, and visible even
        // when the harness only allocates a partial TTY.
        println!();
        println!("  {}", "Neo IDE is running".cyan().bold());
        println!();
        println!("  Open this in your browser:");
        println!("      {}", url.cyan().bold().underlined());
        match bind_hint {
            Some(hint) => println!("\n  Bound on {} ({})", bind, hint.dark_grey()),
            None => println!("\n  Bound on {}", bind.dark_grey()),
        }
        println!();
        println!("  Press {} to stop the server.", "Ctrl+C".bold());
        println!();
    }
}

fn print_shutdown(output_mode: &OutputMode) {
    if output_mode.is_ci() {
        println!("[ok] Neo IDE stopped");
    } else {
        println!();
        println!("  {} {}", "✓".green().bold(), "Neo IDE stopped".green().bold());
        println!();
    }
}

async fn serve_asset(uri: Uri) -> Response {
    serve_path(uri.path())
}

fn serve_path(req_path: &str) -> Response {
    let stripped = req_path.trim_start_matches('/');
    let lookup = if stripped.is_empty() { "index.html" } else { stripped };

    match IdeAssets::get(lookup) {
        Some(file) => {
            let mime = file.metadata.mimetype().to_string();
            ([(header::CONTENT_TYPE, mime)], file.data.into_owned()).into_response()
        }
        None => match IdeAssets::get("index.html") {
            Some(file) => (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8".to_string())],
                file.data.into_owned(),
            )
                .into_response(),
            None => (StatusCode::NOT_FOUND, "IDE assets missing").into_response(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use std::net::Ipv4Addr;

    async fn body_string(resp: Response) -> (StatusCode, String, String) {
        let status = resp.status();
        let content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        (status, content_type, String::from_utf8_lossy(&bytes).to_string())
    }

    #[tokio::test]
    async fn serve_path_root_returns_index_html_with_html_mime() {
        let resp = serve_path("/");
        let (status, content_type, body) = body_string(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(content_type.starts_with("text/html"), "bad MIME: {content_type}");
        assert!(body.contains("Neo IDE"), "body missing identifier: {body}");
    }

    #[tokio::test]
    async fn serve_path_unknown_path_falls_back_to_index_html() {
        // SPA fallback: any unknown route serves index.html so the client
        // router can take over.
        let resp = serve_path("/some/spa/route");
        let (status, content_type, body) = body_string(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(content_type.starts_with("text/html"), "bad MIME: {content_type}");
        assert!(body.contains("Neo IDE"), "fallback should serve index.html: {body}");
    }

    #[tokio::test]
    async fn serve_path_strips_leading_slash() {
        // Lookup must work whether the path comes in as "/index.html" or "index.html".
        let resp = serve_path("/index.html");
        let (status, _, body) = body_string(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Neo IDE"));
    }

    #[test]
    fn browseable_url_swaps_unspecified_v4_for_loopback() {
        let url = browseable_url(
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 2323),
        );
        assert_eq!(url, "http://127.0.0.1:2323");
    }

    #[test]
    fn browseable_url_swaps_unspecified_v6_for_loopback() {
        let url = browseable_url(
            IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), 2323),
        );
        assert_eq!(url, "http://[::1]:2323");
    }

    #[test]
    fn browseable_url_preserves_specific_v4_host() {
        let url = browseable_url(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2323),
        );
        assert_eq!(url, "http://127.0.0.1:2323");
    }

    #[test]
    fn browseable_url_brackets_specific_v6_host() {
        // For v6, SocketAddr's Display brackets the address, so the printed URL is browseable.
        let url = browseable_url(
            IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 9000),
        );
        assert_eq!(url, "http://[::1]:9000");
    }
}
