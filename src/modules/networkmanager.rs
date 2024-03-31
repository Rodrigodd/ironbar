use color_eyre::Result;
use futures_lite::StreamExt;
use gtk::prelude::*;
use gtk::{Image, Orientation};
use serde::Deserialize;
use tokio::sync::mpsc::Receiver;
use zbus::fdo::PropertiesProxy;
use zbus::names::InterfaceName;
use zbus::zvariant::ObjectPath;

use crate::config::CommonConfig;
use crate::gtk_helpers::IronbarGtkExt;
use crate::image::ImageProvider;
use crate::modules::{Module, ModuleInfo, ModuleParts, ModuleUpdateEvent, WidgetContext};
use crate::{glib_recv, send_async, spawn};

#[derive(Debug, Deserialize, Clone)]
pub struct NetworkManagerModule {
    #[serde(default = "default_icon_size")]
    icon_size: i32,

    #[serde(flatten)]
    pub common: Option<CommonConfig>,
}

const fn default_icon_size() -> i32 {
    24
}

#[derive(Clone, Debug)]
pub enum NetworkManagerState {
    Cellular,
    Offline,
    Unknown,
    Vpn,
    Wired,
    Wireless,
    WirelessDisconnected,
}

impl Module<gtk::Box> for NetworkManagerModule {
    type SendMessage = NetworkManagerState;
    type ReceiveMessage = ();

    fn name() -> &'static str {
        "networkmanager"
    }

    fn spawn_controller(
        &self,
        _: &ModuleInfo,
        context: &WidgetContext<NetworkManagerState, ()>,
        _: Receiver<()>,
    ) -> Result<()> {
        let tx = context.tx.clone();

        spawn(async move {
            /* TODO: This should be moved into a client à la the upower module, however that
            requires additional refactoring as both would request a PropertyProxy but on
            different buses. The proper solution will be to rewrite both to use trait-derived
            proxies. */
            let nm_proxy = {
                let dbus = zbus::Connection::system().await?;
                PropertiesProxy::builder(&dbus)
                    .destination("org.freedesktop.NetworkManager")?
                    .path("/org/freedesktop/NetworkManager")?
                    .build()
                    .await?
            };
            let device_interface_name =
                InterfaceName::from_static_str("org.freedesktop.NetworkManager")?;

            let state = get_network_state(&nm_proxy, &device_interface_name).await?;
            send_async!(tx, ModuleUpdateEvent::Update(state));

            let mut prop_changed_stream = nm_proxy.receive_properties_changed().await?;
            while let Some(signal) = prop_changed_stream.next().await {
                let args = signal.args()?;
                if args.interface_name != device_interface_name {
                    continue;
                }

                let state = get_network_state(&nm_proxy, &device_interface_name).await?;
                send_async!(tx, ModuleUpdateEvent::Update(state));
            }

            Result::<()>::Ok(())
        });

        Ok(())
    }

    fn into_widget(
        self,
        context: WidgetContext<NetworkManagerState, ()>,
        info: &ModuleInfo,
    ) -> Result<ModuleParts<gtk::Box>> {
        let container = gtk::Box::new(Orientation::Horizontal, 0);
        let icon = Image::new();
        icon.add_class("icon");
        container.add(&icon);

        let icon_theme = info.icon_theme.clone();

        let initial_icon_name = "icon:content-loading-symbolic";
        ImageProvider::parse(initial_icon_name, &icon_theme, false, self.icon_size)
            .map(|provider| provider.load_into_image(icon.clone()));

        let rx = context.subscribe();
        glib_recv!(rx, state => {
            let icon_name = match state {
                NetworkManagerState::Cellular => "network-cellular-symbolic",
                NetworkManagerState::Offline => "network-wireless-disabled-symbolic",
                NetworkManagerState::Unknown => "dialog-question-symbolic",
                NetworkManagerState::Vpn => "network-vpn-symbolic",
                NetworkManagerState::Wired => "network-wired-symbolic",
                NetworkManagerState::Wireless => "network-wireless-symbolic",
                NetworkManagerState::WirelessDisconnected => "network-wireless-acquiring-symbolic",
            };
            ImageProvider::parse(icon_name, &icon_theme, false, self.icon_size)
                .map(|provider| provider.load_into_image(icon.clone()));
        });

        Ok(ModuleParts::new(container, None))
    }
}

async fn get_network_state(
    nm_proxy: &PropertiesProxy<'_>,
    device_interface_name: &InterfaceName<'_>,
) -> Result<NetworkManagerState> {
    let properties = nm_proxy.get_all(device_interface_name.clone()).await?;

    let primary_connection_path = properties["PrimaryConnection"]
        .downcast_ref::<ObjectPath>()
        .expect("PrimaryConnection was not an object path, violation of NetworkManager D-Bus interface");

    if primary_connection_path != "/" {
        let primary_connection_type = properties["PrimaryConnectionType"]
            .downcast_ref::<str>()
            .expect("PrimaryConnectionType was not a string, violation of NetworkManager D-Bus interface")
            .to_string();

        match primary_connection_type.as_str() {
            "802-11-olpc-mesh" => Ok(NetworkManagerState::Wireless),
            "802-11-wireless" => Ok(NetworkManagerState::Wireless),
            "802-3-ethernet" => Ok(NetworkManagerState::Wired),
            "adsl" => Ok(NetworkManagerState::Wired),
            "cdma" => Ok(NetworkManagerState::Cellular),
            "gsm" => Ok(NetworkManagerState::Cellular),
            "pppoe" => Ok(NetworkManagerState::Wired),
            "vpn" => Ok(NetworkManagerState::Vpn),
            "wifi-p2p" => Ok(NetworkManagerState::Wireless),
            "wimax" => Ok(NetworkManagerState::Cellular),
            "wireguard" => Ok(NetworkManagerState::Vpn),
            "wpan" => Ok(NetworkManagerState::Wireless),
            _ => Ok(NetworkManagerState::Unknown),
        }
    } else {
        let wireless_enabled = *properties["WirelessEnabled"]
            .downcast_ref::<bool>()
            .expect("WirelessEnabled was not a boolean, violation of NetworkManager D-Bus interface");
        if wireless_enabled {
            Ok(NetworkManagerState::WirelessDisconnected)
        } else {
            Ok(NetworkManagerState::Offline)
        }
    }
}
