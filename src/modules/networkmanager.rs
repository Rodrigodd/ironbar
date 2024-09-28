mod config;

use color_eyre::Result;
use futures_lite::StreamExt;
use futures_signals::signal::SignalExt;
use gtk::prelude::{ContainerExt, WidgetExt};
use gtk::{Box as GtkBox, Image, Orientation};
use serde::Deserialize;
use tokio::sync::mpsc::Receiver;

use crate::clients::networkmanager::state::{
    CellularState, State, VpnState, WifiState, WiredState,
};
use crate::clients::networkmanager::Client;
use crate::config::CommonConfig;
use crate::gtk_helpers::IronbarGtkExt;
use crate::image::ImageProvider;
use crate::modules::{Module, ModuleInfo, ModuleParts, ModuleUpdateEvent, WidgetContext};
use crate::{glib_recv, module_impl, send_async, spawn};

#[derive(Debug, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NetworkManagerModule {
    #[serde(default = "default_icon_size")]
    icon_size: i32,

    #[serde(default)]
    icons: config::IconsConfig,

    #[serde(flatten)]
    pub common: Option<CommonConfig>,
}

const fn default_icon_size() -> i32 {
    24
}

impl Module<GtkBox> for NetworkManagerModule {
    type SendMessage = State;
    type ReceiveMessage = ();

    fn spawn_controller(
        &self,
        _: &ModuleInfo,
        context: &WidgetContext<State, ()>,
        _: Receiver<()>,
    ) -> Result<()> {
        let client = context.try_client::<Client>()?;
        let mut client_signal = client.subscribe().to_stream();
        let widget_transmitter = context.tx.clone();

        spawn(async move {
            while let Some(state) = client_signal.next().await {
                send_async!(widget_transmitter, ModuleUpdateEvent::Update(state));
            }
        });

        Ok(())
    }

    fn into_widget(
        self,
        context: WidgetContext<State, ()>,
        info: &ModuleInfo,
    ) -> Result<ModuleParts<GtkBox>> {
        let container = GtkBox::new(Orientation::Horizontal, 0);

        // Wired icon
        let wired_icon = Image::new();
        wired_icon.add_class("icon");
        wired_icon.add_class("wired-icon");
        container.add(&wired_icon);

        // Wifi icon
        let wifi_icon = Image::new();
        wifi_icon.add_class("icon");
        wifi_icon.add_class("wifi-icon");
        container.add(&wifi_icon);

        // Cellular icon
        let cellular_icon = Image::new();
        cellular_icon.add_class("icon");
        cellular_icon.add_class("cellular-icon");
        container.add(&cellular_icon);

        // VPN icon
        let vpn_icon = Image::new();
        vpn_icon.add_class("icon");
        vpn_icon.add_class("vpn-icon");
        container.add(&vpn_icon);

        let icon_theme = info.icon_theme.clone();
        glib_recv!(context.subscribe(), state => {
            macro_rules! update_icon {
                (
                    $icon_var:expr,
                    $state_type:ident,
                    {$($state:pat => $icon_name:expr,)+}
                ) => {
                    let icon_name = match state.$state_type {
                        $($state => $icon_name,)+
                    };
                    if icon_name.is_empty() {
                        $icon_var.hide();
                    } else {
                        ImageProvider::parse(icon_name, &icon_theme, false, self.icon_size)
                            .map(|provider| provider.load_into_image(&$icon_var));
                        $icon_var.show();
                    }
                };
            }

            match &state.wifi {
                WifiState::Connected(state) => {
                    let tooltip = format!("{}\n{}/{}", state.ssid, state.ip4_address, state.ip4_prefix);
                    wifi_icon.set_tooltip_text(Some(&tooltip));
                },
                _ => {
                    wifi_icon.set_tooltip_text(None);
                },
            }

            update_icon!(wired_icon, wired, {
                WiredState::Connected => &self.icons.wired.connected,
                WiredState::Disconnected => &self.icons.wired.disconnected,
                WiredState::NotPresent | WiredState::Unknown => "",
            });
            update_icon!(wifi_icon, wifi, {
                WifiState::Connected(state) => {
                    let n = strengh_to_level(state.strength, self.icons.wifi.levels.len());
                    &self.icons.wifi.levels[n]
                },
                WifiState::Disconnected => &self.icons.wifi.disconnected,
                WifiState::Disabled => &self.icons.wifi.disabled,
                WifiState::NotPresent | WifiState::Unknown => "",
            });
            update_icon!(cellular_icon, cellular, {
                CellularState::Connected => &self.icons.cellular.connected,
                CellularState::Disconnected => &self.icons.cellular.disconnected,
                CellularState::Disabled => &self.icons.cellular.disabled,
                CellularState::NotPresent | CellularState::Unknown => "",
            });
            update_icon!(vpn_icon, vpn, {
                VpnState::Connected(_) => &self.icons.vpn.connected,
                VpnState::Disconnected | VpnState::Unknown => "",
            });
        });

        Ok(ModuleParts::new(container, None))
    }

    module_impl!("networkmanager");
}

/// Convert strength level (from 0-100), to a level (from 0 to `number_of_levels-1`).
const fn strengh_to_level(strength: u8, number_of_levels: usize) -> usize {
    // Strength levels based for the one show by [`nmcli dev wifi list`](https://github.com/NetworkManager/NetworkManager/blob/83a259597000a88217f3ccbdfe71c8114242e7a6/src/libnmc-base/nm-client-utils.c#L700-L727):
    // match strength {
    //     0..=4 => 0,
    //     5..=29 => 1,
    //     30..=54 => 2,
    //     55..=79 => 3,
    //     80.. => 4,
    // }

    // to make it work with a custom number of levels, we approach the logic above with the logic
    // below (0 for < 5, and a linear interpolation for 5 to 105).
    // TODO: if there are more than 20 levels, the last level will be out of scale, and never be
    // reach.
    if strength < 5 {
        return 0;
    }
    (strength as usize - 5) * (number_of_levels - 1) / 100 + 1
}

// Just to make sure my implementation still follow the original logic
#[cfg(test)]
#[test]
fn test_strength_to_level() {
    assert_eq!(strengh_to_level(0, 5), 0);
    assert_eq!(strengh_to_level(4, 5), 0);
    assert_eq!(strengh_to_level(5, 5), 1);
    assert_eq!(strengh_to_level(6, 5), 1);
    assert_eq!(strengh_to_level(29, 5), 1);
    assert_eq!(strengh_to_level(30, 5), 2);
    assert_eq!(strengh_to_level(54, 5), 2);
    assert_eq!(strengh_to_level(55, 5), 3);
    assert_eq!(strengh_to_level(79, 5), 3);
    assert_eq!(strengh_to_level(80, 5), 4);
    assert_eq!(strengh_to_level(100, 5), 4);
}
