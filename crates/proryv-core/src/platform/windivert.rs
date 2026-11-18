//! WinDivert 2.x: загружаем WinDivert.dll динамически (лежит рядом с exe
//! вместе с драйвером WinDivert64.sys) и перехватываем исходящий TCP.

use std::{
    ffi::{c_void, CString},
    io,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use libloading::Library;

use super::{Filter, PacketIo, PlatformError};

type Handle = *mut c_void;
const INVALID_HANDLE: Handle = -1isize as Handle;
const LAYER_NETWORK: u32 = 0;
const SHUTDOWN_BOTH: u32 = 3;
/// Размер WINDIVERT_ADDRESS в WinDivert 2.x.
pub const ADDR_LEN: usize = 80;

type OpenFn = unsafe extern "C" fn(*const i8, u32, i16, u64) -> Handle;
type RecvFn = unsafe extern "C" fn(Handle, *mut c_void, u32, *mut u32, *mut c_void) -> i32;
type SendFn = unsafe extern "C" fn(Handle, *const c_void, u32, *mut u32, *const c_void) -> i32;
type ShutdownFn = unsafe extern "C" fn(Handle, u32) -> i32;
type CloseFn = unsafe extern "C" fn(Handle) -> i32;
type SetParamFn = unsafe extern "C" fn(Handle, u32, u64) -> i32;

/// Очередь драйвера: по умолчанию 4096 пакетов / 4 МБ — при всплесках трафика пакеты теряются.
const QUEUE_PARAMS: [(u32, u64); 3] = [(0, 8192), (1, 2000), (2, 8 << 20)];

pub struct WinDivert {
    handle: Handle,
    recv: RecvFn,
    send: SendFn,
    shutdown: ShutdownFn,
    close: CloseFn,
    _lib: Library,
}

// recv/send/shutdown WinDivert можно звать из разных потоков.
unsafe impl Send for WinDivert {}
unsafe impl Sync for WinDivert {}

/// WinDivert.dll ищется рядом с exe, затем в папке из PRORYV_WINDIVERT_DIR.
pub fn find_dll() -> Option<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf());
            dirs.push(d.join("resources"));
        }
    }
    if let Ok(d) = std::env::var("PRORYV_WINDIVERT_DIR") {
        dirs.push(PathBuf::from(d));
    }
    dirs.into_iter()
        .find(|d| d.join("WinDivert.dll").exists() && d.join("WinDivert64.sys").exists())
        .map(|d| d.join("WinDivert.dll"))
}

pub fn filter_string(f: &Filter) -> String {
    let ports = if f.tcp_ports.is_empty() { vec![443] } else { f.tcp_ports.clone() };
    let ports = ports.iter().map(|p| format!("tcp.DstPort == {p}")).collect::<Vec<_>>().join(" or ");
    let mut parts = vec![format!("(tcp and ({ports}) and tcp.PayloadLength > 0)")];
    if f.block_quic {
        let g = super::nets_condition(&super::google_nets());
        parts.push(format!("(udp and udp.DstPort == 443 and ({g}))"));
    }
    if !f.udp_ranges.is_empty() {
        let r = f.udp_ranges.iter().map(|(a, b)| format!("(udp.DstPort >= {a} and udp.DstPort <= {b})")).collect::<Vec<_>>().join(" or ");
        parts.push(format!("(udp and ({r}))"));
    }
    if (f.game_tcp || f.game_udp) && !f.game_nets.is_empty() {
        let nets = super::nets_condition(&f.game_nets);
        if f.game_tcp {
            parts.push(format!("(tcp and tcp.DstPort >= 1024 and tcp.PayloadLength > 0 and ({nets}))"));
        }
        if f.game_udp {
            parts.push(format!("(udp and udp.DstPort >= 1024 and ({nets}))"));
        }
    }
    let skip: String = f.exclude_ifs.iter().map(|i| format!(" and ifIdx != {i}")).collect();
    let out = format!("outbound and !loopback and !impostor{skip} and ({})", parts.join(" or "));
    if !f.syn_ack {
        return out;
    }
    let ports = if f.tcp_ports.is_empty() { vec![443] } else { f.tcp_ports.clone() };
    let src = ports.iter().map(|p| format!("tcp.SrcPort == {p}")).collect::<Vec<_>>().join(" or ");
    format!("({out}) or (inbound and !loopback{skip} and tcp.Syn and tcp.Ack and ({src}))")
}

/// Результат PowerShell живёт 30 с: запрос медленный, а VPN включают и выключают нечасто.
fn cached<T: Clone>(slot: &Mutex<Option<(Instant, T)>>, load: impl FnOnce() -> T) -> T {
    let mut s = slot.lock().unwrap();
    if let Some((at, v)) = s.as_ref() {
        if at.elapsed() < Duration::from_secs(30) {
            return v.clone();
        }
    }
    let v = load();
    *s = Some((Instant::now(), v.clone()));
    v
}

/// VPN-туннели (sing-tun, Wintun, WireGuard, TAP, Radmin…): их трафик обход ломает,
/// а за пределы ПК он всё равно уходит зашифрованным. Возвращает (индекс, описание).
pub fn tunnel_interfaces() -> Vec<(u32, String)> {
    static CACHE: Mutex<Option<(Instant, Vec<(u32, String)>)>> = Mutex::new(None);
    cached(&CACHE, load_tunnel_interfaces)
}

fn load_tunnel_interfaces() -> Vec<(u32, String)> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-NetAdapter -IncludeHidden | Where-Object { $_.Status -eq 'Up' } | ForEach-Object { ($_.ifIndex, $_.InterfaceDescription) -join [char]9 }",
        ])
        .creation_flags(0x0800_0000)
        .output();
    let Ok(out) = out else { return vec![] };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (idx, desc) = l.trim().split_once('\t')?;
            if !is_tunnel(desc) {
                return None;
            }
            Some((idx.parse().ok()?, desc.to_string()))
        })
        .collect()
}

/// IPv4-адрес физического адаптера с маршрутом по умолчанию (не VPN).
/// Запросы, привязанные к нему, идут напрямую к провайдеру — мимо любого туннеля.
pub fn direct_source_ip() -> Option<String> {
    static CACHE: Mutex<Option<(Instant, Option<String>)>> = Mutex::new(None);
    cached(&CACHE, load_direct_source_ip)
}

fn load_direct_source_ip() -> Option<String> {
    use std::os::windows::process::CommandExt;
    let script = "Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object { $_.RouteMetric + $_.InterfaceMetric } | ForEach-Object { $a = Get-NetAdapter -InterfaceIndex $_.ifIndex -ErrorAction SilentlyContinue; $ip = (Get-NetIPAddress -InterfaceIndex $_.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue | Select-Object -First 1).IPAddress; if ($a -and $ip -and $a.Status -eq 'Up') { ($ip, $a.InterfaceDescription) -join [char]9 } }";
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).lines().find_map(|l| {
        let (ip, desc) = l.trim().split_once('\t')?;
        (!is_tunnel(desc)).then(|| ip.to_string())
    })
}

pub fn is_tunnel(desc: &str) -> bool {
    let d = desc.to_ascii_lowercase();
    ["tun", "wintun", "wireguard", "tap-", "tap ", "vpn", "tunnel", "wan miniport", "zerotier", "tailscale", "hamachi"]
        .iter()
        .any(|k| d.contains(k))
}

impl WinDivert {
    pub fn open(filter: &Filter) -> Result<Self, PlatformError> {
        let dll = find_dll().ok_or_else(|| PlatformError::DriverMissing("WinDivert.dll / WinDivert64.sys".into()))?;
        Self::open_with(&dll, filter)
    }

    pub fn open_with(dll: &Path, filter: &Filter) -> Result<Self, PlatformError> {
        unsafe {
            let lib = Library::new(dll).map_err(|e| PlatformError::DriverMissing(e.to_string()))?;
            fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, PlatformError> {
                unsafe { lib.get::<T>(name) }
                    .map(|s| *s)
                    .map_err(|e| PlatformError::DriverMissing(e.to_string()))
            }
            let open: OpenFn = sym(&lib, b"WinDivertOpen\0")?;
            let recv: RecvFn = sym(&lib, b"WinDivertRecv\0")?;
            let send: SendFn = sym(&lib, b"WinDivertSend\0")?;
            let shutdown: ShutdownFn = sym(&lib, b"WinDivertShutdown\0")?;
            let close: CloseFn = sym(&lib, b"WinDivertClose\0")?;
            let set_param: SetParamFn = sym(&lib, b"WinDivertSetParam\0")?;

            let f = CString::new(filter_string(filter)).expect("filter");
            let handle = open(f.as_ptr(), LAYER_NETWORK, 0, 0);
            if handle == INVALID_HANDLE || handle.is_null() {
                let err = io::Error::last_os_error();
                return Err(match err.raw_os_error() {
                    Some(5) => PlatformError::NotElevated,
                    Some(2) | Some(3) => PlatformError::DriverMissing("WinDivert64.sys".into()),
                    Some(577) | Some(1275) | Some(1058) => PlatformError::DriverBlocked(err.to_string()),
                    // FWP_E_IN_USE: фильтры WFP держит драйвер WinDivert другой версии (zapret, GoodbyeDPI).
                    Some(-2144206838) => PlatformError::DriverInUse,
                    _ => PlatformError::Io(err),
                });
            }
            for (param, value) in QUEUE_PARAMS {
                set_param(handle, param, value);
            }
            Ok(WinDivert { handle, recv, send, shutdown, close, _lib: lib })
        }
    }
}

impl PacketIo for WinDivert {
    fn recv(&self, buf: &mut [u8], addr: &mut [u8]) -> Result<usize, PlatformError> {
        assert!(addr.len() >= ADDR_LEN);
        let mut len = 0u32;
        let ok = unsafe { (self.recv)(self.handle, buf.as_mut_ptr().cast(), buf.len() as u32, &mut len, addr.as_mut_ptr().cast()) };
        if ok == 0 {
            let err = io::Error::last_os_error();
            // ERROR_NO_DATA (232) / ERROR_OPERATION_ABORTED (995) — после shutdown.
            return Err(match err.raw_os_error() {
                Some(232) | Some(995) | Some(6) => PlatformError::Shutdown,
                _ => PlatformError::Io(err),
            });
        }
        Ok(len as usize)
    }

    fn send(&self, packet: &[u8], addr: &[u8]) -> Result<(), PlatformError> {
        let mut sent = 0u32;
        let ok = unsafe { (self.send)(self.handle, packet.as_ptr().cast(), packet.len() as u32, &mut sent, addr.as_ptr().cast()) };
        if ok == 0 {
            return Err(PlatformError::Io(io::Error::last_os_error()));
        }
        Ok(())
    }

    fn shutdown(&self) {
        unsafe {
            (self.shutdown)(self.handle, SHUTDOWN_BOTH);
        }
    }

    fn addr_len(&self) -> usize {
        ADDR_LEN
    }

    fn mark_checksums_valid(&self, addr: &mut [u8]) {
        // Битовые поля по смещению 8: ... IPChecksum (бит 21), TCPChecksum (бит 22).
        let mut flags = u32::from_le_bytes([addr[8], addr[9], addr[10], addr[11]]);
        flags |= (1 << 21) | (1 << 22);
        addr[8..12].copy_from_slice(&flags.to_le_bytes());
    }
}

impl Drop for WinDivert {
    fn drop(&mut self) {
        unsafe {
            (self.close)(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_has_ports() {
        let f = filter_string(&Filter { tcp_ports: vec![443, 80], ..Default::default() });
        assert!(f.contains("tcp.DstPort == 443 or tcp.DstPort == 80"));
        assert!(f.starts_with("outbound"));
        assert!(!f.contains("udp"));
        let f = filter_string(&Filter { tcp_ports: vec![443], block_quic: true, exclude_ifs: vec![17, 42], ..Default::default() });
        assert!(f.contains("udp.DstPort == 443"));
        assert!(f.contains("ifIdx != 17 and ifIdx != 42"));
        let f = filter_string(&Filter {
            tcp_ports: vec![443],
            udp_ranges: vec![(50000, 50100)],
            game_udp: true,
            game_nets: vec![crate::platform::parse_cidr("128.116.0.0/17").unwrap()],
            ..Default::default()
        });
        assert!(f.contains("(udp.DstPort >= 50000 and udp.DstPort <= 50100)"));
        assert!(f.contains("ip.DstAddr >= 128.116.0.0 and ip.DstAddr <= 128.116.127.255"));
        assert!(!f.contains("tcp.DstPort >= 1024"));
    }

    /// Строку фильтра проверяет сам WinDivert (WinDivertHelperCompileFilter):
    /// драйвер и права администратора для этого не нужны.
    #[test]
    fn filter_with_ipv6_nets_compiles_in_windivert() {
        type CompileFn = unsafe extern "C" fn(*const i8, u32, *mut u8, u32, *mut *const i8, *mut u32) -> i32;
        let dll = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/windivert/WinDivert.dll");
        let lib = unsafe { Library::new(&dll) }.expect("WinDivert.dll из third_party");
        let compile: CompileFn = unsafe { *lib.get(b"WinDivertHelperCompileFilter\0").unwrap() };
        let f = Filter {
            tcp_ports: vec![443, 80],
            block_quic: true,
            udp_ranges: vec![(19294, 19344)],
            game_tcp: true,
            game_udp: true,
            game_nets: vec![crate::platform::parse_cidr("128.116.0.0/17").unwrap(), crate::platform::parse_cidr("2620:2b:e000::/40").unwrap()],
            exclude_ifs: vec![17],
            syn_ack: true,
        };
        let s = CString::new(filter_string(&f)).unwrap();
        let (mut err, mut pos) = (std::ptr::null(), 0u32);
        let ok = unsafe { compile(s.as_ptr(), LAYER_NETWORK, std::ptr::null_mut(), 0, &mut err, &mut pos) };
        let msg = if err.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(err) }.to_string_lossy().into_owned() };
        assert!(ok != 0, "WinDivert не принял фильтр: {msg} (позиция {pos})\n{}", filter_string(&f));
        let bad = CString::new("outbound and ipv6.DstAddr >= 2a00::zz").unwrap();
        assert_eq!(unsafe { compile(bad.as_ptr(), LAYER_NETWORK, std::ptr::null_mut(), 0, &mut err, &mut pos) }, 0, "сломанный фильтр должен отклоняться");
    }

    #[test]
    fn detects_tunnels() {
        assert!(is_tunnel("sing-tun Tunnel"));
        assert!(is_tunnel("Famatech Radmin VPN Ethernet Adapter"));
        assert!(is_tunnel("WireGuard Tunnel"));
        assert!(!is_tunnel("Remote NDIS Compatible Device"));
        assert!(!is_tunnel("Intel(R) Ethernet Connection (12) I219-V"));
        assert!(!is_tunnel("TP-Link Wireless PCI Express Adapter"));
    }
}
