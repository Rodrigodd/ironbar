use serde::Deserialize;

macro_rules! default_function {
    ($(($name:ident, $default:expr),)*) => {
        $(
            fn $name() -> String {
                ($default).to_string()
            }
        )*
    };
}

#[derive(Debug, Deserialize, Clone, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct IconsConfig {
    #[serde(default)]
    pub wired: IconsConfigWired,
    #[serde(default)]
    pub wifi: IconsConfigWifi,
    #[serde(default)]
    pub cellular: IconsConfigCellular,
    #[serde(default)]
    pub vpn: IconsConfigVpn,
}

#[derive(Debug, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct IconsConfigWired {
    #[serde(default = "default_wired_connected")]
    pub connected: String,
    #[serde(default = "default_wired_disconnected")]
    pub disconnected: String,
}
impl Default for IconsConfigWired {
    fn default() -> Self {
        Self {
            connected: default_wired_connected(),
            disconnected: default_wired_disconnected(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct IconsConfigWifi {
    #[serde(default = "default_wifi_levels")]
    pub levels: Vec<String>,
    #[serde(default = "default_wifi_disconnected")]
    pub disconnected: String,
    #[serde(default = "default_wifi_disabled")]
    pub disabled: String,
}

impl Default for IconsConfigWifi {
    fn default() -> Self {
        Self {
            levels: default_wifi_levels(),
            disconnected: default_wifi_disconnected(),
            disabled: default_wifi_disabled(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct IconsConfigCellular {
    #[serde(default = "default_cellular_connected")]
    pub connected: String,
    #[serde(default = "default_cellular_disconnected")]
    pub disconnected: String,
    #[serde(default = "default_cellular_disabled")]
    pub disabled: String,
}
impl Default for IconsConfigCellular {
    fn default() -> Self {
        Self {
            connected: default_cellular_connected(),
            disconnected: default_cellular_disconnected(),
            disabled: default_cellular_disabled(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct IconsConfigVpn {
    #[serde(default = "default_vpn_connected")]
    pub connected: String,
}
impl Default for IconsConfigVpn {
    fn default() -> Self {
        Self {
            connected: default_vpn_connected(),
        }
    }
}

pub fn default_wifi_levels() -> Vec<String> {
    vec![
        "icon:network-wireless-signal-none-symbolic".to_string(),
        "icon:network-wireless-signal-weak-symbolic".to_string(),
        "icon:network-wireless-signal-ok-symbolic".to_string(),
        "icon:network-wireless-signal-good-symbolic".to_string(),
        "icon:network-wireless-signal-excellent-symbolic".to_string(),
    ]
}

default_function! {
    (default_wired_connected,  "icon:network-wired-symbolic"),
    (default_wired_disconnected,  "icon:network-wired-disconnected-symbolic"),

    (default_wifi_disconnected, "icon:network-wireless-offline-symbolic"),
    (default_wifi_disabled, "icon:network-wireless-hardware-disabled-symbolic"),

    (default_cellular_connected,"icon:network-cellular-connected-symbolic"),
    (default_cellular_disconnected,"icon:network-cellular-offline-symbolic"),
    (default_cellular_disabled,"icon:network-cellular-hardware-disabled-symbolic"),

    (default_vpn_connected, "icon:network-vpn-symbolic"),
}
