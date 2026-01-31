/**
 * Test fixtures and utilities for MP3YouTube Playwright tests.
 *
 * This file provides common utilities, mock data, and helper functions
 * used across all test files.
 */

import { test as base, expect, Page, Locator } from "@playwright/test";

/**
 * Mock device data matching the DeviceInfo struct from Rust.
 */
export interface MockDevice {
  name: string;
  mount_point: string;
  total_bytes: number;
  available_bytes: number;
  file_system: string;
  is_removable: boolean;
}

/**
 * Mock playlist data matching the PlaylistMetadata struct from Rust.
 */
export interface MockPlaylist {
  name: string;
  source_url: string | null;
  created_at: number;
  modified_at: number;
  track_count: number;
  total_bytes: number;
  thumbnail_url: string | null;
}

/**
 * Sample mock devices for testing.
 */
export const mockDevices: MockDevice[] = [
  {
    name: "SanDisk Clip Sport",
    mount_point: "/Volumes/CLIP_SPORT",
    total_bytes: 8_000_000_000, // 8GB
    available_bytes: 5_000_000_000, // 5GB free
    file_system: "FAT32",
    is_removable: true,
  },
  {
    name: "Sony Walkman",
    mount_point: "/Volumes/WALKMAN",
    total_bytes: 16_000_000_000, // 16GB
    available_bytes: 12_000_000_000, // 12GB free
    file_system: "FAT32",
    is_removable: true,
  },
  {
    name: "Generic MP3 Player",
    mount_point: "/Volumes/MP3PLAYER",
    total_bytes: 4_000_000_000, // 4GB
    available_bytes: 500_000_000, // 500MB free (low space)
    file_system: "FAT32",
    is_removable: true,
  },
];

/**
 * Sample mock playlists for testing.
 */
export const mockPlaylists: MockPlaylist[] = [
  {
    name: "Workout Mix",
    source_url: "https://www.youtube.com/playlist?list=PLtest123",
    created_at: Date.now() - 86400000, // 1 day ago
    modified_at: Date.now() - 3600000, // 1 hour ago
    track_count: 25,
    total_bytes: 250_000_000, // 250MB
    thumbnail_url: "https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
  },
  {
    name: "Chill Vibes",
    source_url: "https://www.youtube.com/playlist?list=PLtest456",
    created_at: Date.now() - 604800000, // 1 week ago
    modified_at: Date.now() - 86400000, // 1 day ago
    track_count: 15,
    total_bytes: 150_000_000, // 150MB
    thumbnail_url: null, // No thumbnail - should show fallback icon
  },
  {
    name: "Road Trip Tunes",
    source_url: "https://www.youtube.com/playlist?list=PLtest789",
    created_at: Date.now() - 2592000000, // 30 days ago
    modified_at: Date.now() - 604800000, // 1 week ago
    track_count: 50,
    total_bytes: 500_000_000, // 500MB
    thumbnail_url: "https://img.youtube.com/vi/9bZkp7q19f0/hqdefault.jpg",
  },
];

/**
 * Valid YouTube playlist URLs for testing form validation.
 */
export const validYouTubeUrls = [
  "https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf",
  "https://youtube.com/playlist?list=PLtest123",
  "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLtest456",
  "https://youtu.be/dQw4w9WgXcQ?list=PLtest789",
];

/**
 * Invalid YouTube URLs for testing form validation.
 */
export const invalidYouTubeUrls = [
  "https://www.youtube.com/watch?v=dQw4w9WgXcQ", // Single video, no playlist
  "https://www.google.com",
  "not-a-url",
  "https://vimeo.com/123456",
  "",
];

/**
 * Invalid playlist names for testing form validation.
 */
export const invalidPlaylistNames = [
  "", // Empty
  "test/name", // Contains /
  "test\\name", // Contains \
  "test:name", // Contains :
  "test*name", // Contains *
  "test?name", // Contains ?
  'test"name', // Contains "
  "test<name", // Contains <
  "test>name", // Contains >
  "test|name", // Contains |
  "CON", // Reserved Windows name
  "PRN", // Reserved Windows name
  "AUX", // Reserved Windows name
  "NUL", // Reserved Windows name
];

/**
 * Extended test fixture with helper methods.
 */
export const test = base.extend<{
  /**
   * Wait for the WASM application to fully load.
   */
  waitForAppLoad: () => Promise<void>;
  /**
   * Get common page elements.
   */
  getElements: () => AppElements;
}>({
  waitForAppLoad: async ({ page }, use) => {
    await use(async () => {
      // Wait for the loading spinner to disappear and app to mount
      await page.waitForSelector(".loading", {
        state: "detached",
        timeout: 30000,
      });
      // Wait for the layout to be visible
      await page.waitForSelector(".layout", {
        state: "visible",
        timeout: 10000,
      });
    });
  },
  getElements: async ({ page }, use) => {
    await use(() => new AppElements(page));
  },
});

/**
 * Helper class to access common UI elements.
 */
export class AppElements {
  constructor(private page: Page) {}

  // Layout elements
  get layout(): Locator {
    return this.page.locator(".layout");
  }

  get header(): Locator {
    return this.page.locator(".layout-header");
  }

  get sidebar(): Locator {
    return this.page.locator(".layout-sidebar");
  }

  get mainContent(): Locator {
    return this.page.locator(".layout-content");
  }

  get mobileMenuToggle(): Locator {
    return this.page.locator(".layout-menu-toggle");
  }

  get settingsButton(): Locator {
    return this.page.locator(".layout-header-actions .btn-icon");
  }

  // Device list elements
  get deviceList(): Locator {
    return this.page.locator(".device-list");
  }

  get deviceListHeader(): Locator {
    return this.page.locator(".device-list-header");
  }

  get deviceRefreshButton(): Locator {
    return this.page.locator(".device-list-header .btn-icon");
  }

  get deviceItems(): Locator {
    return this.page.locator(".device-item");
  }

  get deviceListLoading(): Locator {
    return this.page.locator(".device-list-loading");
  }

  get deviceListEmpty(): Locator {
    return this.page.locator(".device-list-empty");
  }

  get deviceListError(): Locator {
    return this.page.locator(".device-list-error");
  }

  // Playlist list elements
  get playlistList(): Locator {
    return this.page.locator(".playlist-list");
  }

  get playlistCards(): Locator {
    return this.page.locator(".playlist-card");
  }

  get playlistListEmpty(): Locator {
    return this.page.locator(".playlist-list-empty");
  }

  get playlistListLoading(): Locator {
    return this.page.locator(".playlist-list-loading");
  }

  get playlistListError(): Locator {
    return this.page.locator(".playlist-list-error");
  }

  get playlistSummary(): Locator {
    return this.page.locator(".playlist-list-summary");
  }

  // Content header elements
  get contentHeader(): Locator {
    return this.page.locator(".content-section-header");
  }

  get newPlaylistButton(): Locator {
    return this.page.locator('button:has-text("New Playlist")');
  }

  get selectForSyncButton(): Locator {
    return this.page.locator('button:has-text("Select for Sync")');
  }

  // Create playlist dialog elements
  get createPlaylistDialog(): Locator {
    return this.page.locator(".create-playlist-dialog");
  }

  get createPlaylistOverlay(): Locator {
    return this.page.locator(".create-playlist-dialog-overlay");
  }

  get playlistUrlInput(): Locator {
    return this.page.locator("#playlist-url");
  }

  get playlistNameInput(): Locator {
    return this.page.locator("#playlist-name");
  }

  get createPlaylistSubmitButton(): Locator {
    return this.page.locator(".create-playlist-dialog-footer .btn-primary");
  }

  get createPlaylistCancelButton(): Locator {
    return this.page.locator(".create-playlist-dialog-footer .btn-secondary");
  }

  get urlValidationIndicator(): Locator {
    return this.page.locator(".create-playlist-input-indicator");
  }

  get urlValidationSuccess(): Locator {
    return this.page.locator(".create-playlist-validation-success");
  }

  get urlValidationError(): Locator {
    return this.page.locator(".create-playlist-error-text");
  }

  // Delete confirmation dialog elements
  get deleteDialog(): Locator {
    return this.page.locator(".confirm-dialog");
  }

  get deleteDialogConfirmButton(): Locator {
    return this.page.locator(".confirm-dialog .btn-danger");
  }

  get deleteDialogCancelButton(): Locator {
    return this.page.locator(".confirm-dialog .btn-secondary");
  }

  // Sync button elements
  get syncButtonContainer(): Locator {
    return this.page.locator('[data-testid="sync-button-container"]');
  }

  get syncButton(): Locator {
    return this.page.locator('[data-testid="sync-button"]');
  }

  get syncButtonHint(): Locator {
    return this.page.locator('[data-testid="sync-button-hint"]');
  }

  get syncCapacityError(): Locator {
    return this.page.locator('[data-testid="sync-capacity-error"]');
  }

  get syncCapacityWarning(): Locator {
    return this.page.locator('[data-testid="sync-capacity-warning"]');
  }

  // Settings panel elements
  get settingsOverlay(): Locator {
    return this.page.locator(".settings-overlay");
  }

  get settingsPanel(): Locator {
    return this.page.locator(".settings-panel");
  }

  get storageDirectoryInput(): Locator {
    return this.page.locator("#storage-dir");
  }

  get settingsSaveButton(): Locator {
    return this.page.locator(".settings-footer .btn-primary");
  }

  get settingsResetButton(): Locator {
    return this.page.locator('button:has-text("Reset to Default")');
  }

  get settingsCancelButton(): Locator {
    return this.page.locator(".settings-footer .btn-ghost");
  }

  get settingsCloseButton(): Locator {
    return this.page.locator(".settings-header .btn-icon");
  }

  // Toast/notification elements
  get toastContainer(): Locator {
    return this.page.locator('[data-testid="toast-container"]');
  }

  get toasts(): Locator {
    return this.page.locator('[data-testid="toast"]');
  }

  get successToasts(): Locator {
    return this.page.locator(
      '[data-testid="toast"][data-toast-type="success"]',
    );
  }

  get errorToasts(): Locator {
    return this.page.locator('[data-testid="toast"][data-toast-type="error"]');
  }

  get warningToasts(): Locator {
    return this.page.locator(
      '[data-testid="toast"][data-toast-type="warning"]',
    );
  }

  get infoToasts(): Locator {
    return this.page.locator('[data-testid="toast"][data-toast-type="info"]');
  }

  // Selection mode elements
  get selectionModeHeader(): Locator {
    return this.page.locator('h2:has-text("Select Playlist to Sync")');
  }

  get selectionCancelButton(): Locator {
    return this.page.locator('button:has-text("Cancel")');
  }

  get syncToDeviceButton(): Locator {
    return this.page.locator('button:has-text("Sync to Device")');
  }

  get playlistSelectionList(): Locator {
    return this.page.locator(".playlist-selection-list");
  }

  get playlistSelectionCards(): Locator {
    return this.page.locator(".playlist-selection-card");
  }

  get playlistSelectionSummary(): Locator {
    return this.page.locator(".playlist-selection-summary");
  }

  // Device status indicator
  get deviceStatusIndicator(): Locator {
    return this.page.locator(".device-status-indicator");
  }

  // Playlist thumbnail elements
  get playlistThumbnails(): Locator {
    return this.page.locator(".playlist-thumbnail");
  }

  get playlistThumbnailImages(): Locator {
    return this.page.locator(".playlist-thumbnail-img");
  }

  get playlistIcons(): Locator {
    return this.page.locator(".playlist-icon");
  }

  // Loading elements
  get spinner(): Locator {
    return this.page.locator(".spinner");
  }

  get skeletons(): Locator {
    return this.page.locator(".skeleton, .skeleton-pulse");
  }

  // Transfer progress panel elements
  get transferProgressPanel(): Locator {
    return this.page.locator('[data-testid="transfer-progress-panel"]');
  }

  get transferStatusIcon(): Locator {
    return this.page.locator('[data-testid="transfer-status-icon"]');
  }

  get transferStatusTitle(): Locator {
    return this.page.locator('[data-testid="transfer-status-title"]');
  }

  get transferStatusSubtitle(): Locator {
    return this.page.locator('[data-testid="transfer-status-subtitle"]');
  }

  get transferCancelBtn(): Locator {
    return this.page.locator('[data-testid="transfer-cancel-btn"]');
  }

  get transferDismissBtn(): Locator {
    return this.page.locator('[data-testid="transfer-dismiss-btn"]');
  }

  get transferCurrentFile(): Locator {
    return this.page.locator('[data-testid="transfer-current-file"]');
  }

  get transferFileProgressBar(): Locator {
    return this.page.locator('[data-testid="transfer-file-progress-bar"]');
  }

  get transferProgressBar(): Locator {
    return this.page.locator('[data-testid="transfer-progress-bar"]');
  }

  get transferProgressPercent(): Locator {
    return this.page.locator('[data-testid="transfer-progress-percent"]');
  }

  get bytesTransferred(): Locator {
    return this.page.locator('[data-testid="bytes-transferred"]');
  }

  get transferStats(): Locator {
    return this.page.locator('[data-testid="transfer-stats"]');
  }

  get transferSpeed(): Locator {
    return this.page.locator('[data-testid="transfer-speed"]');
  }

  get transferEta(): Locator {
    return this.page.locator('[data-testid="transfer-eta"]');
  }

  get transferElapsed(): Locator {
    return this.page.locator('[data-testid="transfer-elapsed"]');
  }

  get transferCounts(): Locator {
    return this.page.locator('[data-testid="transfer-counts"]');
  }

  get filesCompleted(): Locator {
    return this.page.locator('[data-testid="files-completed"]');
  }

  get filesSkipped(): Locator {
    return this.page.locator('[data-testid="files-skipped"]');
  }

  get filesFailed(): Locator {
    return this.page.locator('[data-testid="files-failed"]');
  }

  get transferProgressIndicator(): Locator {
    return this.page.locator('[data-testid="transfer-progress-indicator"]');
  }

  get indicatorProgress(): Locator {
    return this.page.locator('[data-testid="indicator-progress"]');
  }
}

/**
 * Helper function to wait for network idle (useful after actions that trigger API calls).
 */
export async function waitForNetworkIdle(
  page: Page,
  timeout = 5000,
): Promise<void> {
  await page.waitForLoadState("networkidle", { timeout });
}

/**
 * Helper function to check if an element has a specific class.
 */
export async function hasClass(
  locator: Locator,
  className: string,
): Promise<boolean> {
  const classes = await locator.getAttribute("class");
  return classes?.split(" ").includes(className) ?? false;
}

/**
 * Helper function to take a screenshot with a descriptive name.
 */
export async function takeScreenshot(page: Page, name: string): Promise<void> {
  await page.screenshot({
    path: `test-results/screenshots/${name}.png`,
    fullPage: true,
  });
}

export { expect };
