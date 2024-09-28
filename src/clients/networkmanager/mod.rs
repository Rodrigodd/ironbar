use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use color_eyre::Result;
use futures_signals::signal::{Mutable, MutableSignalCloned};
use tracing::error;
use zbus::blocking::Connection;
use zbus::zvariant::ObjectPath;

use crate::clients::networkmanager::dbus::{
    AccessPointDbusProxyBlocking, ActiveConnectionDbusProxyBlocking, DbusProxyBlocking,
    DeviceDbusProxyBlocking,
};
use crate::clients::networkmanager::state::{
    determine_cellular_state, determine_vpn_state, determine_wifi_state, determine_wired_state,
    CellularState, State, VpnState, WifiState, WiredState,
};
use crate::{
    read_lock, register_fallible_client, spawn_blocking, spawn_blocking_result, write_lock,
};

mod dbus;
pub mod state;

type PathMap<'l, ValueType> = HashMap<ObjectPath<'l>, ValueType>;

#[derive(Debug)]
pub struct Client(Arc<ClientInner<'static>>);

#[derive(Debug)]
struct ClientInner<'l> {
    state: Mutable<State>,
    root_object: &'l DbusProxyBlocking<'l>,
    active_connections: RwLock<PathMap<'l, ActiveConnectionDbusProxyBlocking<'l>>>,
    devices: RwLock<PathMap<'l, DeviceDbusProxyBlocking<'l>>>,
    access_point: RwLock<Option<(ObjectPath<'l>, AccessPointDbusProxyBlocking<'l>)>>,
    dbus_connection: Connection,
}
impl ClientInner<'static> {
    /// Query the state information for each device. This method can fail at random if the
    /// connection changes while querying the information.
    fn update_state_for_device_change(self: &Arc<ClientInner<'static>>) -> Result<()> {
        self.state.set(State {
            wired: determine_wired_state(&read_lock!(self.devices))?,
            wifi: determine_wifi_state(&Client(self.clone()))?,
            cellular: determine_cellular_state(&read_lock!(self.devices))?,
            vpn: self.state.get_cloned().vpn,
        });
        Ok(())
    }
}

impl Client {
    fn new() -> Result<Client> {
        let state = Mutable::new(State {
            wired: WiredState::Unknown,
            wifi: WifiState::Unknown,
            cellular: CellularState::Unknown,
            vpn: VpnState::Unknown,
        });
        let dbus_connection = Connection::system()?;
        let root_object = {
            let root_object = DbusProxyBlocking::new(&dbus_connection)?;
            // Workaround for the fact that zbus (unnecessarily) requires a static lifetime here
            Box::leak(Box::new(root_object))
        };

        Ok(Client(Arc::new(ClientInner {
            state,
            root_object,
            active_connections: RwLock::new(HashMap::new()),
            devices: RwLock::new(HashMap::new()),
            access_point: RwLock::new(None),
            dbus_connection,
        })))
    }

    fn run(&self) -> Result<()> {
        macro_rules! spawn_path_list_watcher {
            (
                $client:expr,
                $property:ident,
                $property_changes:ident,
                $proxy_type:ident,
                |$state_client:ident| $state_update:expr
                $(, |$property_client:ident, $new_path:ident| $property_watcher:expr)*
            ) => {
                let client = $client.clone();
                spawn_blocking_result!({
                    let changes = client.root_object.$property_changes();
                    for _ in changes {
                        let mut new_path_map = HashMap::new();
                        let new_paths = client.root_object.$property()?;
                        {
                            let path_map = read_lock!(client.$property);
                            for new_path in &new_paths {
                                if path_map.contains_key(&new_path) {
                                    let proxy = path_map
                                        .get(new_path)
                                        .expect("Should contain the key, guarded by runtime check");
                                    new_path_map.insert(new_path.clone(), proxy.to_owned());
                                } else {
                                    let new_proxy = $proxy_type::builder(&client.dbus_connection)
                                        .path(new_path.clone())?
                                        .build()?;
                                    new_path_map.insert(new_path.clone(), new_proxy);
                                }
                            }
                        }
                        *write_lock!(client.$property) = new_path_map;

                        for _new_path in &new_paths {
                            $({
                                let $property_client = &client;
                                let $new_path = _new_path;
                                $property_watcher;
                            })*
                        }

                        let $state_client = &client;
                        $state_update;
                    }
                    Ok(())
                });
            }
        }

        macro_rules! spawn_property_watcher {
            (
                $client:expr,
                $path:expr,
                $property_changes:ident,
                $containing_list:ident,
                |$inner_client:ident| $state_update:expr
            ) => {
                let client = $client.clone();
                let path = $path.clone();
                spawn_blocking_result!({
                    let changes = {
                        let path_map = read_lock!(client.$containing_list);
                        let Some(device) = path_map.get(&path) else {
                            // this item could have been removed before the watcher was initialized
                            tracing::warn!("item removed before first iteration");
                            return Ok(());
                        };
                        device.$property_changes()
                    };
                    for _ in changes {
                        if !read_lock!(client.$containing_list).contains_key(&path) {
                            // this item no longer exits
                            break;
                        }
                        let $inner_client = &client;
                        $state_update;
                    }
                    Ok(())
                });
            };
        }

        // initialize active_connections proxys
        {
            let active_connections = HashMap::new();
            for active_connection_path in self.0.root_object.active_connections()? {
                let proxy = ActiveConnectionDbusProxyBlocking::builder(&self.0.dbus_connection)
                    .path(active_connection_path.clone())?
                    .build()?;
                self.0
                    .active_connections
                    .write()
                    .unwrap()
                    .insert(active_connection_path, proxy);
            }
            *write_lock!(self.0.active_connections) = active_connections;
        }

        // initialize devices proxys and watchers
        {
            let devices = self.0.root_object.devices()?;
            let mut path_map = HashMap::new();
            for device_path in &devices {
                let proxy = DeviceDbusProxyBlocking::builder(&self.0.dbus_connection)
                    .path(device_path.clone())?
                    .build()?;
                path_map.insert(device_path.clone(), proxy);
            }
            *write_lock!((self.0).devices) = path_map;

            tracing::debug!("initialize devices: {:?}", devices);

            for device_path in devices {
                spawn_property_watcher!(
                    self.0,
                    device_path,
                    receive_state_changed,
                    devices,
                    |client| {
                        let _ = client.update_state_for_device_change();
                    }
                );
            }
        }

        let _ = self.0.update_state_for_device_change();

        spawn_path_list_watcher!(
            self.0,
            active_connections,
            receive_active_connections_changed,
            ActiveConnectionDbusProxyBlocking,
            |client| {
                tracing::debug!("active connections changed");
                client.state.set(State {
                    wired: client.state.get_cloned().wired,
                    wifi: client.state.get_cloned().wifi,
                    cellular: client.state.get_cloned().cellular,
                    vpn: determine_vpn_state(&read_lock!(client.active_connections))?,
                });
            }
        );
        spawn_path_list_watcher!(
            self.0,
            devices,
            receive_devices_changed,
            DeviceDbusProxyBlocking,
            |client| {
                tracing::debug!("devices changed");
                let _ = client.update_state_for_device_change();
            },
            |client, path| {
                spawn_property_watcher!(client, path, receive_state_changed, devices, |client| {
                    tracing::debug!("device state changed");
                    let _ = client.update_state_for_device_change();
                });
            }
        );

        Ok(())
    }

    pub fn subscribe(&self) -> MutableSignalCloned<State> {
        self.0.state.signal_cloned()
    }
}

pub fn create_client() -> Result<Arc<Client>> {
    let client = Arc::new(Client::new()?);
    {
        let client = client.clone();
        spawn_blocking_result!({
            client.run()?;
            Ok(())
        });
    }
    Ok(client)
}

register_fallible_client!(Client, network_manager);
