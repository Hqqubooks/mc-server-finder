use log::{debug, info};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::config::Config;
use crate::discord::DiscordNotifier;
use crate::minecraft::{extract_description, ping_server_fast, quick_port_check};
use crate::network::{increment_ip, load_subnets, random_ipv4_from_subnets};
use crate::stats::{ScanMessage, StatsCollector};

pub async fn run_scanner() {
    let config = Config::load().expect("Failed to load config");

    let (tx, mut rx) = mpsc::unbounded_channel::<ScanMessage>();
    let subnets = load_subnets();

    let discord_notifier = DiscordNotifier::new(config.discord.clone());

    for ip in &config.test_servers.test_ips {
        info!("[TEST] Ping server {}:{}", ip, config.scanning.port);
        match ping_server_fast(
            ip,
            config.scanning.port,
            None,
            config.timeouts.connection_ms,
            config.timeouts.protocol_response_ms,
            config.minecraft.protocol_version,
        )
        .await
        {
            Ok(info) => {
                let description = extract_description(&info.description);
                info!(
                    "[FOUND][TEST] {}:{} - {}/{} - {} - {}",
                    ip,
                    config.scanning.port,
                    info.players.online,
                    info.players.max,
                    info.version.name,
                    description
                );
            }
            Err(e) => {
                info!(
                    "[MISS][TEST] {}:{} no valid response ({})",
                    ip, config.scanning.port, e
                );
            }
        }
    }

    info!(
        "Starting parallel scan with {} tasks",
        config.scanning.num_tasks
    );

    let stats_interval = config.stats.stats_interval_seconds;
    let stats_handle = tokio::spawn(async move {
        let mut stats = StatsCollector::new().with_discord(discord_notifier);

        while let Some(msg) = rx.recv().await {
            stats.update(msg);

            if stats.should_report_stats(stats_interval) {
                stats.report_stats(stats_interval);
            }
        }
    });

    let mut handles = Vec::new();

    for task_id in 0..config.scanning.num_tasks {
        let subnets_clone = subnets.clone();
        let tx_clone = tx.clone();
        let port = config.scanning.port;
        let max_range_size = config.scanning.max_range_size;
        let consecutive_threshold = config.scanning.consecutive_threshold;
        let chunk_size = config.scanning.chunk_size;
        let base_source_port = config.networking.base_source_port
            + (task_id as u16 * config.networking.port_range_per_task);
        let port_check_timeout = config.timeouts.port_check_ms;
        let connection_timeout = config.timeouts.connection_ms;
        let protocol_timeout = config.timeouts.protocol_response_ms;
        let protocol_version = config.minecraft.protocol_version;

        let handle = tokio::spawn(async move {
            let mut source_port_counter = 0u16;

            loop {
                let thread_ip = random_ipv4_from_subnets(&subnets_clone);
                let start_time = Instant::now();
                debug!(
                    "[TASK {}] New start IP {} (from subnet)",
                    task_id + 1,
                    thread_ip
                );

                let mut current_ip = thread_ip;
                let mut local_scanned = 0;
                let mut local_found = 0;
                let mut consecutive_empty = 0;

                while local_scanned < max_range_size && consecutive_empty < consecutive_threshold {
                    let current_chunk_size = chunk_size.min(max_range_size - local_scanned);
                    let chunk_ips: Vec<String> = (0..current_chunk_size)
                        .map(|i| {
                            let ip = increment_ip(&current_ip, i as u32);
                            ip.to_string()
                        })
                        .collect();

                    let mut port_scan_tasks = Vec::new();
                    for ip_str in &chunk_ips {
                        let ip_str_clone = ip_str.clone();

                        let source_port = base_source_port + (source_port_counter % 255);
                        source_port_counter = source_port_counter.wrapping_add(1);

                        let task = tokio::spawn(async move {
                            match quick_port_check(
                                &ip_str_clone,
                                port,
                                Some(source_port),
                                port_check_timeout,
                            )
                            .await
                            {
                                Ok(true) => Some((ip_str_clone, true)),
                                _ => Some((ip_str_clone, false)),
                            }
                        });
                        port_scan_tasks.push(task);
                    }

                    let mut open_ips = Vec::new();
                    for task in port_scan_tasks {
                        if let Ok(Some((ip, is_open))) = task.await {
                            if is_open {
                                open_ips.push(ip.clone());
                                let _ = tx_clone.send(ScanMessage::OpenPort(ip));
                            }
                        }
                    }

                    let mut mc_ping_tasks = Vec::new();
                    for ip_str in &open_ips {
                        let ip_str_clone = ip_str.clone();

                        let source_port = base_source_port + (source_port_counter % 255);
                        source_port_counter = source_port_counter.wrapping_add(1);

                        let task = tokio::spawn(async move {
                            match ping_server_fast(
                                &ip_str_clone,
                                port,
                                Some(source_port),
                                connection_timeout,
                                protocol_timeout,
                                protocol_version,
                            )
                            .await
                            {
                                Ok(info) => {
                                    let description = extract_description(&info.description);
                                    Some((
                                        format!(
                                            "[FOUND] {}:{} - {}/{} - {} - {}",
                                            ip_str_clone,
                                            port,
                                            info.players.online,
                                            info.players.max,
                                            info.version.name,
                                            description
                                        ),
                                        true,
                                    ))
                                }
                                _ => Some((String::new(), false)),
                            }
                        });
                        mc_ping_tasks.push(task);
                    }

                    let mut chunk_found = 0;
                    for task in mc_ping_tasks {
                        if let Ok(Some((message, found))) = task.await {
                            if found {
                                chunk_found += 1;
                                local_found += 1;
                                consecutive_empty = 0;
                                let _ = tx_clone.send(ScanMessage::Found(message));
                            }
                        }
                    }

                    local_scanned += current_chunk_size;
                    let _ = tx_clone.send(ScanMessage::Scanned(current_chunk_size as u64));

                    if chunk_found == 0 && open_ips.is_empty() {
                        consecutive_empty += current_chunk_size;
                    }

                    current_ip = increment_ip(&current_ip, current_chunk_size as u32);
                }

                let elapsed = start_time.elapsed();
                let scans_per_minute = if elapsed.as_secs() > 0 {
                    (local_scanned as f64 * 60.0) / elapsed.as_secs() as f64
                } else {
                    local_scanned as f64
                };

                let end_ip = increment_ip(&thread_ip, (local_scanned - 1) as u32);

                if local_found > 0 {
                    debug!(
                        "[TASK {}] [RANGE] {}-{} - Found {} servers in {} IPs in {:.2}s ({:.1} scans/min) - Density: {:.2}%",
                        task_id + 1,
                        thread_ip,
                        end_ip,
                        local_found,
                        local_scanned,
                        elapsed.as_secs_f64(),
                        scans_per_minute,
                        (local_found as f64 / local_scanned as f64) * 100.0
                    );
                } else {
                    debug!(
                        "[TASK {}] Range {}-{} - {} scans in {:.2}s ({:.1} scans/min)",
                        task_id + 1,
                        thread_ip,
                        end_ip,
                        local_scanned,
                        elapsed.as_secs_f64(),
                        scans_per_minute
                    );
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    drop(tx);
    let _ = stats_handle.await;
}
