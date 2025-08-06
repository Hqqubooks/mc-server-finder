use crate::config::DiscordConfig;
use log::{debug, error};
use reqwest::Client;
use serde_json::json;

#[derive(Clone)]
pub struct DiscordNotifier {
    client: Client,
    config: DiscordConfig,
}

#[derive(Debug)]
pub struct MinecraftServer {
    pub ip: String,
    pub port: u16,
    pub players_online: u32,
    pub players_max: u32,
    pub version: String,
    pub description: String,
    pub country: Option<String>,
}

impl DiscordNotifier {
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn notify_server_found(&self, server: MinecraftServer) {
        let webhook_url = self.get_webhook_for_server(&server);

        if webhook_url.is_empty() {
            debug!(
                "No webhook configured for version {} (players: {})",
                server.version, server.players_online
            );
            return;
        }

        let is_active = server.players_online > 0;
        let status_emoji = if is_active { "🟢" } else { "🔴" };
        let status_text = if is_active {
            "Active Server"
        } else {
            "Empty Server"
        };

        let embed = json!({
            "embeds": [{
                "title": format!("🎮 {} Found!", status_text),
                "color": self.get_color_for_server(&server),
                "fields": [
                    {
                        "name": "🌐 IP Address",
                        "value": format!("{}:{}", server.ip, server.port),
                        "inline": true
                    },
                    {
                        "name": format!("{} Players", status_emoji),
                        "value": format!("{}/{}", server.players_online, server.players_max),
                        "inline": true
                    },
                    {
                        "name": "🌍 Country",
                        "value": server.country.as_ref().unwrap_or(&"Unknown".to_string()).clone(),
                        "inline": true
                    },
                    {
                        "name": "📦 Version",
                        "value": server.version,
                        "inline": true
                    },
                    {
                        "name": "📝 Description",
                        "value": if server.description.is_empty() {
                            "No description".to_string()
                        } else {
                            // Truncate description if too long for Discord
                            if server.description.len() > 1000 {
                                format!("{}...", &server.description[..1000])
                            } else {
                                server.description
                            }
                        },
                        "inline": false
                    }
                ],
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "footer": {
                    "text": "Minecraft Port Scanner"
                }
            }]
        });

        const MAX_RETRIES: u32 = 3;
        for attempt in 1..=MAX_RETRIES {
            match self.client.post(webhook_url).json(&embed).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!(
                            "Successfully sent Discord notification for {}:{} ({})",
                            server.ip,
                            server.port,
                            if is_active { "active" } else { "empty" }
                        );
                        return;
                    } else if response.status().as_u16() == 429 {
                        error!(
                            "Discord webhook rate limited, attempt {}/{}",
                            attempt, MAX_RETRIES
                        );
                        if attempt < MAX_RETRIES {
                            tokio::time::sleep(tokio::time::Duration::from_secs(
                                2 * attempt as u64,
                            ))
                            .await;
                            continue;
                        }
                    } else {
                        error!(
                            "Discord webhook failed with status: {} (attempt {}/{})",
                            response.status(),
                            attempt,
                            MAX_RETRIES
                        );
                        if attempt < MAX_RETRIES {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to send Discord notification (attempt {}/{}): {}",
                        attempt, MAX_RETRIES, e
                    );
                    if attempt < MAX_RETRIES {
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                }
            }
        }
    }

    fn get_webhook_for_server(&self, server: &MinecraftServer) -> &str {
        let is_active = server.players_online > 0;

        if server.version.starts_with("1.21") {
            if is_active {
                &self.config.webhook_121_active
            } else {
                &self.config.webhook_121_empty
            }
        } else if server.version.starts_with("1.20") {
            if is_active {
                &self.config.webhook_120_active
            } else {
                &self.config.webhook_120_empty
            }
        } else if server.version.starts_with("1.19") {
            if is_active {
                &self.config.webhook_119_active
            } else {
                &self.config.webhook_119_empty
            }
        } else {
            if is_active {
                &self.config.webhook_other_active
            } else {
                &self.config.webhook_other_empty
            }
        }
    }

    fn get_color_for_server(&self, server: &MinecraftServer) -> u32 {
        let is_active = server.players_online > 0;

        if server.version.starts_with("1.21") {
            if is_active { 0x00ff00 } else { 0x004400 }
        } else if server.version.starts_with("1.20") {
            if is_active { 0x0099ff } else { 0x003366 }
        } else if server.version.starts_with("1.19") {
            if is_active { 0xffaa00 } else { 0x664400 }
        } else {
            if is_active { 0xff0066 } else { 0x660033 }
        }
    }
}

async fn get_country_from_ip(ip: &str) -> Option<String> {
    let url = format!("http://ip-api.com/json/{}?fields=country", ip);

    match reqwest::get(&url).await {
        Ok(response) => {
            if let Ok(text) = response.text().await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(country) = json.get("country").and_then(|c| c.as_str()) {
                        return Some(country.to_string());
                    }
                }
            }
        }
        Err(e) => {
            debug!("Failed to get country for IP {}: {}", ip, e);
        }
    }
    None
}

pub async fn extract_server_info_with_country(found_message: &str) -> Option<MinecraftServer> {
    let mut server = extract_server_info(found_message)?;

    server.country = get_country_from_ip(&server.ip).await;

    Some(server)
}

pub fn extract_server_info(found_message: &str) -> Option<MinecraftServer> {
    if !found_message.starts_with("[FOUND]") {
        return None;
    }

    let content = found_message.strip_prefix("[FOUND] ")?;
    let parts: Vec<&str> = content.split(" - ").collect();

    if parts.len() < 4 {
        return None;
    }

    let ip_port_parts: Vec<&str> = parts[0].split(':').collect();
    if ip_port_parts.len() != 2 {
        return None;
    }

    let ip = ip_port_parts[0].to_string();
    let port = ip_port_parts[1].parse::<u16>().ok()?;

    let player_parts: Vec<&str> = parts[1].split('/').collect();
    if player_parts.len() != 2 {
        return None;
    }

    let players_online = player_parts[0].parse::<u32>().ok()?;
    let players_max = player_parts[1].parse::<u32>().ok()?;

    let version = parts[2].to_string();

    let description = parts[3..].join(" - ");

    Some(MinecraftServer {
        ip,
        port,
        players_online,
        players_max,
        version,
        description,
        country: None,
    })
}
