use color_eyre::{Report, Result};

use crate::clients::networkmanager::dbus::{
    AccessPointDbusProxyBlocking, ActiveConnectionDbusProxyBlocking, DeviceDbusProxyBlocking,
    DeviceState, DeviceType, DeviceWirelessDbusProxyBlocking, Ip4ConfigDbusProxyBlocking,
};
use crate::clients::networkmanager::PathMap;
use crate::{error, read_lock, spawn_blocking, spawn_blocking_result, write_lock};

#[derive(Clone, Debug)]
pub struct State {
    pub wired: WiredState,
    pub wifi: WifiState,
    pub cellular: CellularState,
    pub vpn: VpnState,
}

#[derive(Clone, Debug)]
pub enum WiredState {
    Connected,
    Disconnected,
    NotPresent,
    Unknown,
}

#[derive(Clone, Debug)]
pub enum WifiState {
    Connected(WifiConnectedState),
    Disconnected,
    Disabled,
    NotPresent,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct WifiConnectedState {
    /// The SSID of the access point.
    pub ssid: String,
    /// The MAC address of the access point.
    pub bssid: String,
    /// Strength in percentage, from 0 to 100.
    pub strength: u8,
    /// The IPv4 address.
    pub ip4_address: String,
    /// The IPv4 prefix, in bits (also known as the subnet mask length).
    pub ip4_prefix: u32,
}

#[derive(Clone, Debug)]
pub enum CellularState {
    Connected,
    Disconnected,
    Disabled,
    NotPresent,
    Unknown,
}

#[derive(Clone, Debug)]
pub enum VpnState {
    Connected(VpnConnectedState),
    Disconnected,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct VpnConnectedState {
    pub name: String,
}

pub(super) fn determine_wired_state(
    devices: &PathMap<DeviceDbusProxyBlocking>,
) -> Result<WiredState> {
    let mut present = false;
    let mut connected = false;

    for device in devices.values() {
        if device.device_type()? == DeviceType::Ethernet {
            present = true;
            if device.state()?.is_enabled() {
                connected = true;
                break;
            }
        }
    }

    if connected {
        Ok(WiredState::Connected)
    } else if present {
        Ok(WiredState::Disconnected)
    } else {
        Ok(WiredState::NotPresent)
    }
}

pub(super) fn determine_wifi_state(client: &super::Client) -> Result<WifiState> {
    let dbus_connection = &client.0.dbus_connection;
    let devices = &read_lock!(client.0.devices);
    let access_point_ = &client.0.access_point;

    let mut present = false;
    let mut enabled = false;
    let mut connected = None;

    for device in devices.values() {
        if device.device_type()? == DeviceType::Wifi {
            present = true;
            if device.state()?.is_enabled() {
                enabled = true;

                let wireless_device = DeviceWirelessDbusProxyBlocking::builder(dbus_connection)
                    .path(device.path().clone())?
                    .build()?;
                let primary_access_point_path = wireless_device.active_access_point()?;
                if primary_access_point_path.as_str() != "/" {
                    connected = Some((
                        device,
                        AccessPointDbusProxyBlocking::builder(dbus_connection)
                            .path(primary_access_point_path)?
                            .build()?,
                    ));
                    break;
                }
            }
        }
    }

    if let Some((device, access_point)) = connected {
        let ssid = access_point
            .ssid()
            .map(|x| String::from_utf8_lossy(&x).to_string())
            .unwrap_or_else(|_| "unkown".into());
        let bssid = access_point.hw_address()?.to_string();

        let ip4config = Ip4ConfigDbusProxyBlocking::builder(dbus_connection)
            .path(device.ip4_config()?.clone())?
            .build()?;
        let address_data = ip4config.address_data()?;
        // Pick the first address. Not sure if there are cases when there are more than one
        // address.
        // These could fail if the access point concurrently changes.
        let address = &address_data
            .first()
            .ok_or_else(|| Report::msg("No address in IP4Config"))?;
        let ip4_address = address
            .get("address")
            .ok_or_else(|| Report::msg("IP address data object must have a address"))?;
        let ip4_prefix = address
            .get("prefix")
            .ok_or_else(|| Report::msg("IP address data object must have a prefix"))?;

        let strength = access_point.strength()?;

        'block: {
            if let Some((ref path, _)) = *read_lock!(access_point_) {
                if path == access_point.path() {
                    break 'block;
                }
            }

            let access_point_path = access_point.path().to_owned();

            *write_lock!(*access_point_) = Some((access_point_path.clone(), access_point));

            let client = client.0.clone();
            spawn_blocking_result!({
                let changes = {
                    let access_point = read_lock!(client.access_point);
                    let Some((_, access_point)) = access_point.as_ref() else {
                        return Ok(());
                    };
                    access_point.receive_strength_changed()
                };

                for _ in changes {
                    if read_lock!(client.access_point)
                        .as_ref()
                        .map_or(false, |(p, _)| *p != access_point_path)
                    {
                        break;
                    }

                    tracing::trace!("accesspoint strength changed");
                    client.update_state_for_device_change()?;
                }

                Ok(())
            });
        }

        Ok(WifiState::Connected(WifiConnectedState {
            ssid,
            bssid,
            ip4_address: String::try_from(ip4_address.to_owned()).unwrap_or_default(),
            ip4_prefix: u32::try_from(ip4_prefix.to_owned()).unwrap_or_default(),
            strength,
        }))
    } else if enabled {
        Ok(WifiState::Disconnected)
    } else if present {
        Ok(WifiState::Disabled)
    } else {
        Ok(WifiState::NotPresent)
    }
}

pub(super) fn determine_cellular_state(
    devices: &PathMap<DeviceDbusProxyBlocking>,
) -> Result<CellularState> {
    let mut present = false;
    let mut enabled = false;
    let mut connected = false;

    for device in devices.values() {
        if device.device_type()? == DeviceType::Modem {
            present = true;
            if device.state()?.is_enabled() {
                enabled = true;
                if device.state()? == DeviceState::Activated {
                    connected = true;
                    break;
                }
            }
        }
    }

    if connected {
        Ok(CellularState::Connected)
    } else if enabled {
        Ok(CellularState::Disconnected)
    } else if present {
        Ok(CellularState::Disabled)
    } else {
        Ok(CellularState::NotPresent)
    }
}

pub(super) fn determine_vpn_state(
    active_connections: &PathMap<ActiveConnectionDbusProxyBlocking>,
) -> Result<VpnState> {
    for connection in active_connections.values() {
        match connection.type_()?.as_str() {
            "vpn" | "wireguard" => {
                return Ok(VpnState::Connected(VpnConnectedState {
                    name: "unknown".into(),
                }));
            }
            _ => {}
        }
    }
    Ok(VpnState::Disconnected)
}
