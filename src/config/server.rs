//! Servers that speak the OpenAI API at an address the user sets: the
//! endpoint that a base URL names, and whether the server is on this machine
//! or this network.

use crate::Result;
use reqwest::Url;
use std::net::{IpAddr, Ipv4Addr};

pub const TRANSCRIPTIONS: &str = "/audio/transcriptions";
pub const CHAT_COMPLETIONS: &str = "/chat/completions";

/// What Hush sends to the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sends {
    Audio,
    Text,
}

/// The full endpoint for `api_path` under `base`. A path that already ends
/// with `api_path` stays as it is, a path that ends with `/v1` gets
/// `api_path`, and any other path gets `/v1` and `api_path`.
pub fn endpoint(base: &str, api_path: &str) -> Result<String> {
    let base = base.trim();
    if base.is_empty() {
        anyhow::bail!("it is empty. Set the server address, such as http://localhost:8080/v1");
    }
    let mut url =
        Url::parse(base).map_err(|e| anyhow::anyhow!("'{}' is not a URL ({})", base, e))?;
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("'{}' does not start with http:// or https://", base);
    }
    if url.host_str().is_none_or(str::is_empty) {
        anyhow::bail!("'{}' has no host", base);
    }
    let path = url.path().trim_end_matches('/');
    let path = if path.ends_with(api_path) {
        path.to_string()
    } else if path.ends_with("/v1") {
        format!("{}{}", path, api_path)
    } else {
        format!("{}/v1{}", path, api_path)
    };
    url.set_path(&path);
    Ok(url.into())
}

/// Whether `url` names this machine or a private network: localhost, a
/// loopback address, 10/8, 172.16/12, 192.168/16, 100.64/10, fc00::/7,
/// fe80::/10, or a `.local` name. The check reads the address as written
/// and never asks DNS.
pub fn is_local(url: &str) -> bool {
    let Ok(url) = Url::parse(url.trim()) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => is_local_v4(v4),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => is_local_v4(v4),
                None => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
            },
        };
    }
    host == "localhost" || host.ends_with(".local")
}

fn is_local_v4(ip: Ipv4Addr) -> bool {
    let [first, second, ..] = ip.octets();
    let shared = first == 100 && (64..=127).contains(&second);
    ip.is_loopback() || ip.is_private() || shared
}

/// The host and port of `url`, such as "192.168.1.5:8080". Credentials and
/// the path stay out, so the result is safe in a log line.
pub fn host_label(url: &str) -> String {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return url.trim().to_string();
    };
    match (parsed.host_str(), parsed.port()) {
        (Some(host), Some(port)) => format!("{}:{}", host, port),
        (Some(host), None) => host.to_string(),
        (None, _) => url.trim().to_string(),
    }
}

/// The model and where it runs, for status lines: a local server, or a
/// service that receives the audio or the text.
pub fn describe(model: &str, url: &str, sends: Sends) -> String {
    let host = host_label(url);
    if is_local(url) {
        return format!("{} on the local server {}", model.trim(), host);
    }
    let what = match sends {
        Sends::Audio => "audio",
        Sends::Text => "text",
    };
    format!(
        "{} on {}, a service that receives the {}",
        model.trim(),
        host,
        what
    )
}

/// The variable that `key_env` names, or `None` when it is blank.
pub fn key_var(key_env: &str) -> Option<&str> {
    Some(key_env.trim()).filter(|var| !var.is_empty())
}

/// Refuse settings that cannot reach a server, with the key to fix.
pub fn validate(
    section: &str,
    url: &str,
    model: &str,
    key_env: &str,
    api_path: &str,
) -> Result<()> {
    endpoint(url, api_path)
        .map_err(|e| anyhow::anyhow!("Invalid {}.server_url: {}", section, e))?;
    if model.trim().is_empty() {
        anyhow::bail!(
            "{}.server_model is empty. Set the model name that the server expects",
            section
        );
    }
    if key_var(key_env).is_some_and(|var| !valid_var_name(var)) {
        anyhow::bail!(
            "Invalid {}.server_key_env: write the name of an environment variable, \
             such as MY_SERVER_KEY, not the key",
            section
        );
    }
    Ok(())
}

fn valid_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A one-shot HTTP server on 127.0.0.1 that answers `reply` as JSON and
/// hands back the raw request it received.
#[cfg(test)]
pub(crate) async fn fake_server(
    reply: String,
) -> (String, tokio::sync::oneshot::Receiver<Vec<u8>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let read = socket.read(&mut chunk).await.unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            let text = String::from_utf8_lossy(&request).to_lowercase();
            let Some(end) = text.find("\r\n\r\n") else {
                continue;
            };
            let head = &text[..end];
            if head.contains("transfer-encoding: chunked") {
                if text.ends_with("0\r\n\r\n") {
                    break;
                }
                continue;
            }
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= end + 4 + length {
                break;
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            reply.len(),
            reply
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        let _ = sender.send(request);
    });
    (url, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcriptions(base: &str) -> String {
        endpoint(base, TRANSCRIPTIONS).unwrap()
    }

    #[test]
    fn a_bare_host_gets_v1_and_the_api_path() {
        assert_eq!(
            transcriptions("http://localhost:8080"),
            "http://localhost:8080/v1/audio/transcriptions"
        );
        assert_eq!(
            transcriptions("http://localhost:8080/"),
            "http://localhost:8080/v1/audio/transcriptions"
        );
        assert_eq!(
            transcriptions("  https://speech.example.com/openai  "),
            "https://speech.example.com/openai/v1/audio/transcriptions"
        );
    }

    #[test]
    fn a_v1_path_gets_the_api_path() {
        assert_eq!(
            transcriptions("http://10.0.0.2:8000/v1"),
            "http://10.0.0.2:8000/v1/audio/transcriptions"
        );
        assert_eq!(
            transcriptions("http://10.0.0.2:8000/v1/"),
            "http://10.0.0.2:8000/v1/audio/transcriptions"
        );
        assert_eq!(
            endpoint("http://box.local:11434/v1", CHAT_COMPLETIONS).unwrap(),
            "http://box.local:11434/v1/chat/completions"
        );
    }

    #[test]
    fn a_full_endpoint_stays_as_it_is() {
        assert_eq!(
            transcriptions("http://host/api/v2/audio/transcriptions"),
            "http://host/api/v2/audio/transcriptions"
        );
        assert_eq!(
            transcriptions("http://host/v1/audio/transcriptions/"),
            "http://host/v1/audio/transcriptions"
        );
        assert_eq!(
            endpoint("https://host/v1/chat/completions", CHAT_COMPLETIONS).unwrap(),
            "https://host/v1/chat/completions"
        );
    }

    #[test]
    fn a_similar_path_is_not_the_v1_path() {
        assert_eq!(
            transcriptions("http://host/apiv1"),
            "http://host/apiv1/v1/audio/transcriptions"
        );
        assert_eq!(
            endpoint("http://host/v1/audio/transcriptions", CHAT_COMPLETIONS).unwrap(),
            "http://host/v1/audio/transcriptions/v1/chat/completions"
        );
    }

    #[test]
    fn a_query_string_stays_after_the_path() {
        assert_eq!(
            transcriptions("http://host/v1?tenant=a"),
            "http://host/v1/audio/transcriptions?tenant=a"
        );
    }

    #[test]
    fn an_empty_or_invalid_url_is_refused() {
        for bad in [
            "",
            "   ",
            "localhost:8080",
            "ftp://host/v1",
            "http://",
            "not a url",
        ] {
            assert!(endpoint(bad, TRANSCRIPTIONS).is_err(), "{bad:?}");
        }
        let message = endpoint("", TRANSCRIPTIONS).unwrap_err().to_string();
        assert!(message.contains("http://localhost:8080/v1"), "{message}");
        let message = endpoint("localhost:8080", TRANSCRIPTIONS)
            .unwrap_err()
            .to_string();
        assert!(message.contains("http://"), "{message}");
    }

    #[test]
    fn loopback_and_private_addresses_are_local() {
        for url in [
            "http://localhost:8080",
            "http://LOCALHOST/v1",
            "http://localhost./v1",
            "http://127.0.0.1:8080",
            "http://127.8.9.10",
            "http://10.1.2.3",
            "http://172.16.0.1",
            "http://172.31.255.254",
            "http://192.168.1.20:9000",
            "http://100.64.0.1",
            "http://100.127.255.255",
            "http://[::1]:8080",
            "http://[fc00::1]",
            "http://[fd12:3456::1]",
            "http://[fe80::1]",
            "http://[febf::1]",
            "http://[::ffff:192.168.1.5]",
            "http://whisper.local:8080",
            "http://Box.Local",
        ] {
            assert!(is_local(url), "{url} is local");
        }
    }

    #[test]
    fn public_addresses_and_names_are_not_local() {
        for url in [
            "https://api.openai.com/v1",
            "http://8.8.8.8",
            "http://172.15.255.255",
            "http://172.32.0.1",
            "http://192.169.0.1",
            "http://100.63.255.255",
            "http://100.128.0.1",
            "http://11.0.0.1",
            "http://[2001:db8::1]",
            "http://[fec0::1]",
            "http://[::ffff:8.8.8.8]",
            "http://localhost.example.com",
            "http://mylocal",
            "http://local",
            "http://example.local.com",
            "not a url",
            "",
        ] {
            assert!(!is_local(url), "{url} is not local");
        }
    }

    #[test]
    fn a_name_is_judged_as_written_without_dns() {
        assert!(!is_local("http://my-nas"), "a bare name is not trusted");
        assert!(is_local("http://my-nas.local"));
    }

    #[test]
    fn the_label_keeps_the_host_and_port_only() {
        assert_eq!(
            host_label("http://user:secret@192.168.1.5:8080/v1/audio/transcriptions"),
            "192.168.1.5:8080"
        );
        assert_eq!(host_label("https://api.example.com/v1"), "api.example.com");
        assert_eq!(host_label("http://[::1]:9000"), "[::1]:9000");
    }

    #[test]
    fn the_description_says_local_server_or_names_what_is_sent() {
        assert_eq!(
            describe("whisper-1", "http://localhost:8080/v1", Sends::Audio),
            "whisper-1 on the local server localhost:8080"
        );
        assert_eq!(
            describe("whisper-1", "https://stt.example.com/v1", Sends::Audio),
            "whisper-1 on stt.example.com, a service that receives the audio"
        );
        assert_eq!(
            describe("llama3", "https://llm.example.com", Sends::Text),
            "llama3 on llm.example.com, a service that receives the text"
        );
    }

    #[test]
    fn validation_names_the_key_to_fix() {
        let ok = validate(
            "transcription",
            "http://localhost:8080",
            "whisper-1",
            "",
            TRANSCRIPTIONS,
        );
        assert!(ok.is_ok());
        let err = validate("transcription", "", "whisper-1", "", TRANSCRIPTIONS)
            .unwrap_err()
            .to_string();
        assert!(err.contains("transcription.server_url"), "{err}");
        let err = validate("llm", "http://x:1", "  ", "", CHAT_COMPLETIONS)
            .unwrap_err()
            .to_string();
        assert!(err.contains("llm.server_model"), "{err}");
        let err = validate("llm", "http://x:1", "m", "sk-live-123", CHAT_COMPLETIONS)
            .unwrap_err()
            .to_string();
        assert!(err.contains("llm.server_key_env"), "{err}");
        assert!(!err.contains("sk-live-123"), "the value is never echoed");
        assert!(validate("llm", "http://x:1", "m", " MY_KEY_2 ", CHAT_COMPLETIONS).is_ok());
    }

    #[test]
    fn a_blank_key_env_means_no_key() {
        assert_eq!(key_var(""), None);
        assert_eq!(key_var("   "), None);
        assert_eq!(key_var(" LOCAL_AI_KEY "), Some("LOCAL_AI_KEY"));
        assert!(valid_var_name("_X1"));
        assert!(!valid_var_name("1X"));
        assert!(!valid_var_name("MY-KEY"));
    }
}
