use crate::discord::{DiscordNotifier, extract_server_info_with_country};
use log::info;
use tokio::time::Instant;

#[derive(Debug)]
pub enum ScanMessage {
    Scanned(u64),
    OpenPort(String),
    Found(String),
}

pub struct StatsCollector {
    start_time: Instant,
    scanned_total: u64,
    servers_found: u64,
    ports_open: u64,
    scanned_last: u64,
    servers_last: u64,
    ports_last: u64,
    last_report_time: Instant,
    discord: Option<DiscordNotifier>,
}

impl StatsCollector {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            scanned_total: 0,
            servers_found: 0,
            ports_open: 0,
            scanned_last: 0,
            servers_last: 0,
            ports_last: 0,
            last_report_time: now,
            discord: None,
        }
    }

    /// Adds Discord notifications to the stats collector
    pub fn with_discord(mut self, discord_notifier: DiscordNotifier) -> Self {
        self.discord = Some(discord_notifier);
        self
    }

    /// Updates counters based on scan results and notifies Discord
    pub fn update(&mut self, message: ScanMessage) {
        match message {
            ScanMessage::Scanned(count) => self.scanned_total += count,
            ScanMessage::OpenPort(_ip) => self.ports_open += 1,
            ScanMessage::Found(message) => {
                self.servers_found += 1;
                info!("{}", message);

                if let Some(discord_notifier) = &self.discord {
                    let discord_notifier = discord_notifier.clone();
                    let message_clone = message.clone();
                    tokio::spawn(async move {
                        if let Some(server_info) =
                            extract_server_info_with_country(&message_clone).await
                        {
                            discord_notifier.notify_server_found(server_info).await;
                        }
                    });
                }
            }
        }
    }

    /// Checks if enough time has passed to report stats
    pub fn should_report_stats(&self, interval: u64) -> bool {
        self.last_report_time.elapsed().as_secs() >= interval
    }

    /// Logs statistics and updates last reported values
    pub fn report_stats(&mut self, interval: u64) {
        let runtime = self.start_time.elapsed();

        let scan_delta = self.scanned_total - self.scanned_last;
        let server_delta = self.servers_found - self.servers_last;
        let port_delta = self.ports_open - self.ports_last;

        let total_rate = if runtime.as_secs() > 0 {
            (self.scanned_total as f64 * 60.0) / runtime.as_secs() as f64
        } else {
            0.0
        };

        let recent_rate = if scan_delta > 0 {
            (scan_delta as f64 * 60.0) / interval as f64
        } else {
            0.0
        };

        let success_rate = if self.scanned_total > 0 {
            (self.servers_found as f64 / self.scanned_total as f64) * 100.0
        } else {
            0.0
        };

        let open_rate = if self.scanned_total > 0 {
            (self.ports_open as f64 / self.scanned_total as f64) * 100.0
        } else {
            0.0
        };

        info!(
            "[STATS] Total: {} IPs scanned, {} open ports ({:.3}%), {} MC servers ({:.3}%) | Rates: {:.1} scans/min total, {:.1} scans/min recent | Runtime: {:.1}m",
            self.scanned_total,
            self.ports_open,
            open_rate,
            self.servers_found,
            success_rate,
            total_rate,
            recent_rate,
            runtime.as_secs_f64() / 60.0
        );

        if server_delta > 0 || port_delta > 0 {
            info!(
                "[STATS] Recent activity: +{} scans, +{} open ports, +{} MC servers in last {}s",
                scan_delta, port_delta, server_delta, interval
            );
        }

        self.scanned_last = self.scanned_total;
        self.servers_last = self.servers_found;
        self.ports_last = self.ports_open;
        self.last_report_time = Instant::now();
    }
}
