//! UI components for `Youtun4`.

pub mod button;
pub mod confirm_dialog;
pub mod connected_devices_bar;
pub mod create_playlist_dialog;
pub mod device_list;
pub mod device_selector_panel;
pub mod device_status_indicator;
pub mod download_progress_panel;
pub mod empty_state;
pub mod header;
pub mod layout;
pub mod loading;
pub mod navigation;
pub mod playlist_card;
pub mod playlist_detail;
pub mod playlist_list;
pub mod playlist_selection;
pub mod playlist_table;
pub mod settings_panel;
pub mod toast;
pub mod track_list;
pub mod transfer_progress_panel;

pub use button::Button;
pub use confirm_dialog::{ConfirmDialog, DeletePlaylistDialog};
pub use connected_devices_bar::ConnectedDevicesBar;
pub use create_playlist_dialog::CreatePlaylistDialog;
pub use device_list::DeviceList;
pub use device_selector_panel::{DeviceCard, DeviceSelectorPanel};
pub use device_status_indicator::{ConnectionStatus, DeviceStatusIndicator};
pub use download_progress_panel::{
    DownloadErrorInfo, DownloadPanelState, DownloadProgressIndicator, DownloadProgressPanel,
};
pub use empty_state::{
    EmptyFolderState, EmptyState, EmptyStateIcon, EmptyStateSize, ErrorEmptyState,
    NoDeviceEmptyState, NoPlaylistsEmptyState, NoSearchResultsEmptyState, NoTracksEmptyState,
    NothingToSyncEmptyState,
};
pub use header::Header;
pub use layout::{
    ContentHeader, ContentSection, Layout, LayoutHeaderActions, LayoutMain, LayoutSidebar,
    MobileMenuContext, ResponsiveGrid,
};
pub use loading::{
    ButtonLoader, ContentLoader, InlineLoader, LoadingIndicator, LoadingOverlay, LoadingState,
    Skeleton, SkeletonBlock, SkeletonListItem, SkeletonText, Spinner,
};
pub use navigation::{NavItem, NavSection, icons as nav_icons};
pub use playlist_card::PlaylistCard;
pub use playlist_detail::{PlaylistDetailState, PlaylistDetailView};
pub use playlist_list::{PlaylistList, PlaylistListState};
pub use playlist_selection::{
    PlaylistSelectionCard, PlaylistSelectionList, PlaylistSelectionState, PlaylistSelectionSummary,
};
pub use playlist_table::PlaylistTable;
pub use settings_panel::SettingsPanel;
pub use toast::{NotificationContext, NotificationProvider, ToastContainer, use_notifications};
pub use track_list::{TrackItemCompact, TrackList, TrackListState};
pub use transfer_progress_panel::{
    TransferPanelState, TransferProgressIndicator, TransferProgressPanel,
};
