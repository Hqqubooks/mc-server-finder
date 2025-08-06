use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

#[derive(Debug)]
pub enum PingError {
    Timeout,
    ConnectionRefused,
    NetworkError(String),
    ProtocolError(String),
}

impl std::fmt::Display for PingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PingError::Timeout => write!(f, "Timeout"),
            PingError::ConnectionRefused => write!(f, "Connection refused"),
            PingError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            PingError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
        }
    }
}

impl std::error::Error for PingError {}

#[derive(Debug, Deserialize)]
pub struct ServerStatus {
    pub version: Version,
    pub players: Players,
    #[serde(default)]
    pub description: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct Version {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Players {
    pub max: u32,
    pub online: u32,
}

pub async fn quick_port_check(
    server_ip: &str,
    server_port: u16,
    source_port: Option<u16>,
    timeout_ms: u64,
) -> Result<bool, PingError> {
    let socket_address = format!("{}:{}", server_ip, server_port);

    let stream_result = if let Some(src_port) = source_port {
        let local_addr = format!("0.0.0.0:{}", src_port);
        timeout(Duration::from_millis(timeout_ms), async {
            let socket = tokio::net::TcpSocket::new_v4()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            socket.set_reuseaddr(true)?;
            socket.set_nodelay(true)?;

            socket.bind(local_addr.parse().unwrap())?;
            socket.connect(socket_address.parse().unwrap()).await
        })
        .await
    } else {
        timeout(Duration::from_millis(timeout_ms), async {
            let socket = tokio::net::TcpSocket::new_v4()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            socket.set_nodelay(true)?;
            socket.connect(socket_address.parse().unwrap()).await
        })
        .await
    };

    match stream_result {
        Ok(Ok(_stream)) => Ok(true),
        Ok(Err(e)) => match e.kind() {
            std::io::ErrorKind::ConnectionRefused => Ok(false),
            std::io::ErrorKind::TimedOut => Ok(false),
            std::io::ErrorKind::AddrInUse => Ok(false),
            _ => Ok(false),
        },
        Err(_) => Ok(false),
    }
}

pub async fn ping_server_fast(
    server_ip: &str,
    server_port: u16,
    source_port: Option<u16>,
    connection_timeout_ms: u64,
    protocol_timeout_ms: u64,
    protocol_version: i32,
) -> Result<ServerStatus, PingError> {
    let socket_address = format!("{}:{}", server_ip, server_port);

    let stream = if let Some(src_port) = source_port {
        let local_addr = format!("0.0.0.0:{}", src_port);
        timeout(Duration::from_millis(connection_timeout_ms), async {
            let socket = tokio::net::TcpSocket::new_v4()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            socket.set_reuseaddr(true)?;
            socket.bind(local_addr.parse().unwrap())?;
            socket.connect(socket_address.parse().unwrap()).await
        })
        .await
    } else {
        timeout(
            Duration::from_millis(connection_timeout_ms),
            TcpStream::connect(&socket_address),
        )
        .await
    };

    let stream = match stream {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            return Err(match e.kind() {
                std::io::ErrorKind::ConnectionRefused => PingError::ConnectionRefused,
                std::io::ErrorKind::TimedOut => PingError::Timeout,
                std::io::ErrorKind::AddrInUse => PingError::NetworkError("Port in use".to_string()),
                _ => PingError::NetworkError(e.to_string()),
            });
        }
        Err(_) => return Err(PingError::Timeout),
    };

    let mut tcp_stream = stream;

    let ip_bytes = server_ip.as_bytes();
    let mut handshake_packet = Vec::with_capacity(32);
    handshake_packet.push(0x00);
    handshake_packet.extend(encode_varint(protocol_version));
    handshake_packet.extend(encode_varint(ip_bytes.len() as i32));
    handshake_packet.extend(ip_bytes);
    handshake_packet.push((server_port >> 8) as u8);
    handshake_packet.push((server_port & 0xFF) as u8);
    handshake_packet.push(0x01);

    let mut packet_to_send = Vec::with_capacity(handshake_packet.len() + 8);
    packet_to_send.extend(encode_varint(handshake_packet.len() as i32));
    packet_to_send.extend(handshake_packet);

    let status_request = [0x01, 0x00];
    tcp_stream
        .write_all(&packet_to_send)
        .await
        .map_err(|e| PingError::NetworkError(e.to_string()))?;
    tcp_stream
        .write_all(&status_request)
        .await
        .map_err(|e| PingError::NetworkError(e.to_string()))?;

    let response_result = timeout(Duration::from_millis(protocol_timeout_ms), async {
        let _response_packet_length = read_varint_async(&mut tcp_stream)
            .await
            .map_err(|e| PingError::ProtocolError(e.to_string()))?;
        let _response_packet_id = read_varint_async(&mut tcp_stream)
            .await
            .map_err(|e| PingError::ProtocolError(e.to_string()))?;
        let json_length = read_varint_async(&mut tcp_stream)
            .await
            .map_err(|e| PingError::ProtocolError(e.to_string()))?;

        let mut json_buffer = vec![0; json_length as usize];
        tcp_stream
            .read_exact(&mut json_buffer)
            .await
            .map_err(|e| PingError::NetworkError(e.to_string()))?;

        let json_string =
            String::from_utf8(json_buffer).map_err(|e| PingError::ProtocolError(e.to_string()))?;
        let server_status: ServerStatus = serde_json::from_str(&json_string)
            .map_err(|e| PingError::ProtocolError(e.to_string()))?;

        Ok::<ServerStatus, PingError>(server_status)
    })
    .await;

    match response_result {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(PingError::Timeout),
    }
}

pub fn extract_description(desc: &serde_json::Value) -> String {
    if let Some(text) = desc.get("text") {
        return text.as_str().unwrap_or("").to_string();
    }

    if let Some(extra) = desc.get("extra") {
        if let Some(array) = extra.as_array() {
            return array
                .iter()
                .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("");
        }
    }

    desc.to_string()
}

/// Encode Minecraft protocol varint
fn encode_varint(mut value: i32) -> Vec<u8> {
    let mut out = vec![];
    loop {
        if (value & !0x7F) == 0 {
            out.push(value as u8);
            return out;
        } else {
            out.push(((value & 0x7F) | 0x80) as u8);
            value >>= 7;
        }
    }
}

/// Read Minecraft protocol varint from stream
async fn read_varint_async<R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<i32, Box<dyn std::error::Error>> {
    let mut num_read = 0;
    let mut result = 0;
    loop {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf).await?;
        let byte = buf[0];
        result |= ((byte & 0x7F) as i32) << (7 * num_read);
        num_read += 1;
        if num_read > 5 {
            return Err("VarInt too big".into());
        }
        if (byte & 0x80) == 0 {
            break;
        }
    }
    Ok(result)
}
