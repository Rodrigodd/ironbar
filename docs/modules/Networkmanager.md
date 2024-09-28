Displays the current network connection state of NetworkManager.
Supports wired ethernet, wifi, cellular data and VPN connections among others.

> [!NOTE]
> This module uses NetworkManager's so-called primary connection, and therefore inherits its limitation of only being able to display the "top-level" connection.
> For example, if we have a VPN connection over a wifi connection it will only display the former, until it is disconnected, at which point it will display the latter.
> A solution to this is currently in the works.

## Configuration

> Type: `networkmanager`

| Name                          | Type       | Default                                               | Description                                       |
| ----------------------------- | ---------- | ----------------------------------------------------- | ------------------------------------------------- |
| `icon_size`                   | `integer`  | `24`                                                  | Size to render icon at.                           |
| `icons.wired.connected`       | `string`   | `icon:network-wired-symbolic`                         | Icon to show when there is a wired connection     |
| `icons.wired.disconnected`    | `string`   | `icon:network-wired-symbolic`                         | Icon to show when there is no wired connection    |
| `icons.wifi.levels`           | `string[]` | `["icon:network-wireless-signal-none-symbolic", ...]` | Icon to show when there is no wifi connection     |
| `icons.wifi.disconnected`     | `string`   | `icon:network-wireless-offline-symbolic`              | Icon to show when there is no wifi connection     |
| `icons.wifi.disabled`         | `string`   | `icon:network-wireless-hardware-disabled-symbolic`    | Icon to show when wifi is disabled                |
| `icons.cellular.connected`    | `string`   | `icon:network-cellular-connected-symbolic`            | Icon to show when there is a cellular connection  |
| `icons.cellular.disconnected` | `string`   | `icon:network-cellular-offline-symbolic`              | Icon to show when there is no cellular connection |
| `icons.cellular.disabled`     | `string`   | `icon:network-cellular-hardware-disabled-symbolic`    | Icon to show when cellular connection is disabled |
| `icons.vpn.connected`         | `string`   | `icon:network-vpn-symbolic`                           | Icon to show when there is a VPN connection       |

<details>
<summary>JSON</summary>

```json
{
  "end": [
    {
      "type": "networkmanager",
      "icon_size": 32
    }
  ]
}
```

</details>

<details>
<summary>TOML</summary>

```toml
[[end]]
type = "networkmanager"
icon_size = 32
```

</details>

<details>
<summary>YAML</summary>

```yaml
end:
  - type: "networkmanager"
    icon_size: 32
```

</details>

<details>
<summary>Corn</summary>

```corn
{
  end = [
    {
      type = "networkmanager"
      icon_size = 32
    }
  ]
}
```

</details>

## Styling

| Selector               | Description                      |
| ---------------------- | -------------------------------- |
| `.networkmanager`      | NetworkManager widget container. |
| `.networkmanger .icon` | NetworkManager widget icons.     |

For more information on styling, please see the [styling guide](styling-guide).
