use crate::config::{CommonConfig, TruncateMode};
use crate::gtk_helpers::IronbarLabelExt;
use crate::modules::{Module, ModuleInfo, ModuleParts, ModuleUpdateEvent, WidgetContext};
use crate::{await_sync, glib_recv, module_impl, try_send};
use color_eyre::{Report, Result};
use gtk::prelude::*;
use gtk::Label;
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{info, trace};

#[derive(Debug, Deserialize, Clone)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HyprlandSubmapModule {
    // -- Common --
    /// See [truncate options](module-level-options#truncate-mode).
    ///
    /// **Default**: `null`
    pub truncate: Option<TruncateMode>,

    /// Translates the submap name to the label text.
    #[serde(default)]
    pub map: std::collections::HashMap<String, String>,

    /// See [common options](module-level-options#common-options).
    #[serde(flatten)]
    pub common: Option<CommonConfig>,
}

impl Module<Label> for HyprlandSubmapModule {
    type SendMessage = String;
    type ReceiveMessage = ();

    module_impl!("hyprland_submap");

    fn spawn_controller(
        &self,
        _info: &ModuleInfo,
        context: &WidgetContext<Self::SendMessage, Self::ReceiveMessage>,
        _rx: mpsc::Receiver<Self::ReceiveMessage>,
    ) -> Result<()> {
        info!("Hyprland Mode module started");
        let tx = context.tx.clone();

        await_sync(async move {
            let client = context.ironbar.clients.borrow_mut().hyprland()?;
            client.listen_submap_events(move |submap| {
                try_send!(tx, ModuleUpdateEvent::Update(submap.clone()));
            });

            Ok::<(), Report>(())
        })?;

        Ok(())
    }

    fn into_widget(
        self,
        context: WidgetContext<Self::SendMessage, Self::ReceiveMessage>,
        _info: &ModuleInfo,
    ) -> Result<ModuleParts<Label>> {
        let label = Label::new(None);
        label.set_use_markup(true);

        {
            let label = label.clone();

            if let Some(truncate) = self.truncate {
                label.truncate(truncate);
            }

            let on_mode = move |submap: String| {
                trace!("submap: {:?}", submap);
                if submap == "default" {
                    label.set_label_escaped("");
                } else {
                    let mapped = self.map.get(&submap).unwrap_or(&submap);
                    label.set_label_escaped(mapped);
                }
            };

            glib_recv!(context.subscribe(), mode => on_mode(mode));
        }

        Ok(ModuleParts {
            widget: label,
            popup: None,
        })
    }
}
