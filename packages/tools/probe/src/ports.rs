//! TCP listener discovery.
//!
//! The parsers are pure functions over command output so they can be tested on
//! any host. Only the thin `collect` wrapper touches the operating system.

use std::process::Command;

/// One listening TCP socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    /// Socket family label, `tcp` or `tcp6`.
    pub proto: String,
    /// Local address, with IPv6 brackets removed.
    pub address: String,
    /// Local port.
    pub port: u16,
    /// Owning process id, when the source reports it (needs privileges on some
    /// systems).
    pub pid: Option<u32>,
}

/// Lists TCP listeners, optionally restricted to one process id.
///
/// Windows parses `netstat -ano`. Other systems try `ss -ltnp` and fall back to
/// `/proc/net/tcp` plus `/proc/net/tcp6`, which reports no process ids.
pub fn collect(filter_pid: Option<u32>) -> Result<Vec<Listener>, String> {
    let mut listeners = raw_listeners()?;
    if let Some(pid) = filter_pid {
        listeners.retain(|listener| listener.pid == Some(pid));
    }
    listeners.sort_by(|left, right| {
        (left.port, &left.address, &left.proto).cmp(&(right.port, &right.address, &right.proto))
    });
    listeners.dedup();
    Ok(listeners)
}

/// Runs the platform-specific listing command and parses its output.
fn raw_listeners() -> Result<Vec<Listener>, String> {
    if cfg!(windows) {
        let output = Command::new("netstat")
            .arg("-ano")
            .output()
            .map_err(|err| format!("could not run netstat: {err}"))?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(parse_netstat(&text))
    } else {
        match Command::new("ss").args(["-ltnp"]).output() {
            Ok(output) if output.status.success() => {
                Ok(parse_ss(&String::from_utf8_lossy(&output.stdout)))
            }
            _ => read_proc_net(),
        }
    }
}

/// Reads the Linux `/proc/net` tables, used when `ss` is not installed.
fn read_proc_net() -> Result<Vec<Listener>, String> {
    let mut listeners = Vec::new();
    let mut read_any = false;

    for (path, proto) in [("/proc/net/tcp", "tcp"), ("/proc/net/tcp6", "tcp6")] {
        if let Ok(content) = std::fs::read_to_string(path) {
            read_any = true;
            listeners.extend(parse_proc_net(&content, proto));
        }
    }

    if read_any {
        Ok(listeners)
    } else {
        Err("could not list listeners: `ss` failed and /proc/net/tcp is unreadable".to_string())
    }
}

/// Parses Windows `netstat -ano` output, keeping TCP rows in `LISTENING` state.
pub fn parse_netstat(output: &str) -> Vec<Listener> {
    let mut listeners = Vec::new();

    for line in output.lines() {
        let mut fields = line.split_whitespace();
        let Some(proto) = fields.next() else {
            continue;
        };
        if !proto.eq_ignore_ascii_case("TCP") {
            continue;
        }
        let (Some(local), Some(_remote), Some(state), Some(pid)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if !state.eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let Some((address, port)) = split_endpoint(local) else {
            continue;
        };
        listeners.push(Listener {
            proto: proto.to_ascii_lowercase(),
            address,
            port,
            pid: pid.parse().ok(),
        });
    }

    listeners
}

/// Parses `ss -ltnp` output, keeping rows whose first column is `LISTEN`.
pub fn parse_ss(output: &str) -> Vec<Listener> {
    let mut listeners = Vec::new();

    for line in output.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 || !fields[0].eq_ignore_ascii_case("LISTEN") {
            continue;
        }
        let Some((address, port)) = split_endpoint(fields[3]) else {
            continue;
        };
        listeners.push(Listener {
            proto: "tcp".to_string(),
            address,
            port,
            pid: find_ss_pid(&fields),
        });
    }

    listeners
}

/// Parses one `/proc/net/tcp` style table (state `0A` means listen).
///
/// The local address is stored as a little-endian hex word, so IPv4 entries are
/// decoded to dotted form. IPv6 entries keep their raw hex, which is enough to
/// tell listeners apart.
pub fn parse_proc_net(content: &str, proto: &str) -> Vec<Listener> {
    let mut listeners = Vec::new();

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 || fields[3] != "0A" {
            continue;
        }
        let Some((hex_address, hex_port)) = fields[1].split_once(':') else {
            continue;
        };
        let Ok(port) = u16::from_str_radix(hex_port, 16) else {
            continue;
        };
        listeners.push(Listener {
            proto: proto.to_string(),
            address: decode_proc_address(hex_address),
            port,
            pid: None,
        });
    }

    listeners
}

/// Splits `host:port` and drops IPv6 brackets. Returns `None` for wildcards.
fn split_endpoint(raw: &str) -> Option<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() || raw.ends_with(":*") {
        return None;
    }
    let (host, port) = raw.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Some((host.to_string(), port))
}

/// Pulls the first `pid=NNN` occurrence out of `ss` user information.
fn find_ss_pid(fields: &[&str]) -> Option<u32> {
    fields
        .iter()
        .filter_map(|field| field.split("pid=").nth(1))
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .next()
}

/// Decodes an 8 digit `/proc` IPv4 word into dotted form, else passes it through.
fn decode_proc_address(hex: &str) -> String {
    if hex.len() != 8 {
        return hex.to_string();
    }
    let Ok(value) = u32::from_str_radix(hex, 16) else {
        return hex.to_string();
    };
    let bytes = value.to_le_bytes();
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETSTAT: &str = "\r\nActive Connections\r\n\r\n  Proto  Local Address          Foreign Address        State           PID\r\n  TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1120\r\n  TCP    127.0.0.1:49701        127.0.0.1:49702        ESTABLISHED     4420\r\n  TCP    [::]:135               [::]:0                 LISTENING       1120\r\n  UDP    0.0.0.0:5353           *:*                                    3384\r\n";

    #[test]
    fn netstat_keeps_only_listeners() {
        let listeners = parse_netstat(NETSTAT);
        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0].address, "0.0.0.0");
        assert_eq!(listeners[0].port, 135);
        assert_eq!(listeners[0].pid, Some(1120));
        assert_eq!(listeners[1].address, "::");
        assert_eq!(listeners[1].port, 135);
    }

    const SS: &str = "State  Recv-Q Send-Q Local Address:Port Peer Address:Port Process\nLISTEN 0      4096   127.0.0.53%lo:53   0.0.0.0:*     users:((\"systemd-resolve\",pid=611,fd=13))\nLISTEN 0      128    0.0.0.0:22           0.0.0.0:*\nESTAB  0      0      10.0.0.2:22          10.0.0.9:51022 users:((\"sshd\",pid=900,fd=4))\n";

    #[test]
    fn ss_parses_listener_rows_and_pid() {
        let listeners = parse_ss(SS);
        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0].address, "127.0.0.53%lo");
        assert_eq!(listeners[0].port, 53);
        assert_eq!(listeners[0].pid, Some(611));
        assert_eq!(listeners[1].port, 22);
        assert_eq!(listeners[1].pid, None);
    }

    const PROC_NET: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 20441 1\n   1: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 20500 1\n   2: 0100007F:C350 0100007F:0016 01 00000000:00000000 00:00000000 00000000     0        0 20501 1\n";

    #[test]
    fn proc_net_decodes_ipv4_and_skips_non_listeners() {
        let listeners = parse_proc_net(PROC_NET, "tcp");
        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0].address, "0.0.0.0");
        assert_eq!(listeners[0].port, 22);
        assert_eq!(listeners[1].address, "127.0.0.1");
        assert_eq!(listeners[1].port, 8080);
        assert_eq!(listeners[1].proto, "tcp");
    }

    #[test]
    fn endpoint_splitting_handles_brackets_and_wildcards() {
        assert_eq!(
            split_endpoint("[::1]:8080"),
            Some(("::1".to_string(), 8080))
        );
        assert_eq!(split_endpoint("0.0.0.0:*"), None);
        assert_eq!(split_endpoint("nonsense"), None);
    }
}
