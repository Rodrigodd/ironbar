use std::sync::Arc;
use std::thread;

use color_eyre::Result;
use futures_signals::signal::{Mutable, MutableSignalCloned};
use tracing::error;
use zbus::blocking::fdo::PropertiesProxy;
use zbus::blocking::Connection;
use zbus::{
    dbus_proxy,
    names::InterfaceName,
    zvariant::{Error as ZVariantError, ObjectPath, Str},
    Error as ZBusError,
};

use crate::{register_fallible_client, spawn, spawn_blocking};

const DBUS_BUS: &str = "org.freedesktop.NetworkManager";
const DBUS_PATH: &str = "/org/freedesktop/NetworkManager";
const DBUS_INTERFACE: &str = "org.freedesktop.NetworkManager";

#[derive(Debug)]
pub struct Client {
    client_state: Mutable<ClientState>,
    interface_name: InterfaceName<'static>,
    dbus_connection: Connection,
}

#[derive(Clone, Debug)]
pub enum ClientState {
    WiredConnected,
    WifiConnected,
    CellularConnected,
    VpnConnected,
    WifiDisconnected,
    Offline,
    Unknown,
}

#[dbus_proxy(
    default_service = "org.freedesktop.NetworkManager",
    interface = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManagerDbus {
    #[dbus_proxy(property)]
    fn active_connections(&self) -> Result<Vec<ObjectPath>>;

    #[dbus_proxy(property)]
    fn devices(&self) -> Result<Vec<ObjectPath>>;

    #[dbus_proxy(property)]
    fn networking_enabled(&self) -> Result<bool>;

    #[dbus_proxy(property)]
    fn primary_connection(&self) -> Result<ObjectPath>;

    #[dbus_proxy(property)]
    fn primary_connection_type(&self) -> Result<Str>;

    #[dbus_proxy(property)]
    fn wireless_enabled(&self) -> Result<bool>;
}

impl Client {
    fn new() -> Result<Self> {
        let client_state = Mutable::new(ClientState::Unknown);
        let dbus_connection = Connection::system()?;
        let interface_name = InterfaceName::from_static_str(DBUS_INTERFACE)?;

        Ok(Self {
            client_state,
            interface_name,
            dbus_connection,
        })
    }

    fn run(&self) -> Result<()> {
        let proxy = NetworkManagerDbusProxyBlocking::new(&self.dbus_connection)?;

        let mut primary_connection = proxy.primary_connection()?;
        let mut primary_connection_type = proxy.primary_connection_type()?;
        let mut wireless_enabled = proxy.wireless_enabled()?;

        todo!()
    }

    pub fn subscribe(&self) -> MutableSignalCloned<ClientState> {
        self.client_state.signal_cloned()
    }
}

pub fn create_client() -> Result<Arc<Client>> {
    let client = Arc::new(Client::new()?);
    {
        let client = client.clone();
        spawn_blocking(move || {
            if let Err(error) = client.run() {
                error!("{}", error);
            };
        });
    }
    Ok(client)
}

fn determine_state(
    primary_connection: &str,
    primary_connection_type: &str,
    wireless_enabled: bool,
) -> ClientState {
    if primary_connection == "/" {
        if wireless_enabled {
            ClientState::WifiDisconnected
        } else {
            ClientState::Offline
        }
    } else {
        match primary_connection_type {
            "802-3-ethernet" | "adsl" | "pppoe" => ClientState::WiredConnected,
            "802-11-olpc-mesh" | "802-11-wireless" | "wifi-p2p" => ClientState::WifiConnected,
            "cdma" | "gsm" | "wimax" => ClientState::CellularConnected,
            "vpn" | "wireguard" => ClientState::VpnConnected,
            _ => ClientState::Unknown,
        }
    }
}

register_fallible_client!(Client, networkmanager);
