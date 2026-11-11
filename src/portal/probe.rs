//! Which portals the desktop has, for `hush doctor`. A probe reads
//! properties only: it opens no session, so no dialog shows and nothing is
//! bound.

use super::PortalError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDesktopInfo {
    pub version: u32,
    /// Whether the portal offers a keyboard.
    pub keyboard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalReport {
    pub remote_desktop: Result<RemoteDesktopInfo, PortalError>,
    pub global_shortcuts: Result<u32, PortalError>,
    pub background: Result<u32, PortalError>,
}

pub async fn probe() -> PortalReport {
    use ashpd::desktop::background::BackgroundProxy;
    use ashpd::desktop::global_shortcuts::GlobalShortcuts;
    use ashpd::desktop::remote_desktop::{DeviceType, RemoteDesktop};
    let remote_desktop = async {
        let error = |e| PortalError::from_ashpd("RemoteDesktop", e);
        let proxy = RemoteDesktop::new().await.map_err(error)?;
        let types = proxy.available_device_types().await.map_err(error)?;
        Ok(RemoteDesktopInfo {
            version: proxy.version(),
            keyboard: types.contains(DeviceType::Keyboard),
        })
    }
    .await;
    let global_shortcuts = GlobalShortcuts::new()
        .await
        .map(|proxy| proxy.version())
        .map_err(|e| PortalError::from_ashpd("GlobalShortcuts", e));
    let background = BackgroundProxy::new()
        .await
        .map(|proxy| proxy.version())
        .map_err(|e| PortalError::from_ashpd("Background", e));
    PortalReport {
        remote_desktop,
        global_shortcuts,
        background,
    }
}
