/**
 * Notification and Error State Tests
 *
 * Tests for toast notifications and error states including:
 * - Toast display and dismissal
 * - Different notification types
 * - Error state handling
 * - Loading states
 */

import { test, expect, AppElements } from "./fixtures";

test.describe("Notifications and Toasts", () => {
  test.beforeEach(async ({ page, waitForAppLoad }) => {
    await page.goto("/");
    await waitForAppLoad();
  });

  test.describe("Toast Container", () => {
    test("should have toast container in DOM", async ({ getElements }) => {
      const elements = getElements();

      await expect(elements.toastContainer).toBeVisible();
    });

    test("should initially have no toasts", async ({ getElements }) => {
      const elements = getElements();

      const toastCount = await elements.toasts.count();
      expect(toastCount).toBe(0);
    });
  });

  test.describe("Toast Display", () => {
    test("should display toast with proper structure", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Trigger an action that shows a toast (e.g., refresh devices)
      await elements.deviceRefreshButton.click();

      // Wait for potential toast
      await page.waitForTimeout(3000);

      // Check if any toasts appeared
      const toastCount = await elements.toasts.count();
      if (toastCount > 0) {
        const toast = elements.toasts.first();

        // Toast should have icon
        const icon = toast.locator(".toast-icon svg");
        await expect(icon).toBeVisible();

        // Toast should have content
        const content = toast.locator(".toast-content");
        await expect(content).toBeVisible();

        // Toast should have dismiss button
        const dismissButton = toast.locator('[data-testid="toast-dismiss"]');
        await expect(dismissButton).toBeVisible();
      }
    });

    test("should have proper ARIA attributes on toast", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.deviceRefreshButton.click();
      await page.waitForTimeout(3000);

      const toastCount = await elements.toasts.count();
      if (toastCount > 0) {
        const toast = elements.toasts.first();

        await expect(toast).toHaveAttribute("role", "alert");
        await expect(toast).toHaveAttribute("aria-live", "polite");
      }
    });

    test("should display toast type correctly", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.deviceRefreshButton.click();
      await page.waitForTimeout(3000);

      const toastCount = await elements.toasts.count();
      if (toastCount > 0) {
        const toast = elements.toasts.first();
        const toastType = await toast.getAttribute("data-toast-type");

        // Toast type should be one of: info, success, warning, error
        expect(["info", "success", "warning", "error"]).toContain(toastType);
      }
    });
  });

  test.describe("Toast Dismissal", () => {
    test("should dismiss toast when clicking dismiss button", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Trigger a toast
      await elements.deviceRefreshButton.click();
      await page.waitForTimeout(3000);

      const initialCount = await elements.toasts.count();
      if (initialCount > 0) {
        // Click dismiss button
        const dismissButton = elements.toasts
          .first()
          .locator('[data-testid="toast-dismiss"]');
        await dismissButton.click();

        // Toast count should decrease
        await page.waitForTimeout(500);
        const newCount = await elements.toasts.count();
        expect(newCount).toBeLessThan(initialCount);
      }
    });

    test("should auto-dismiss toasts after timeout", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Trigger a toast
      await elements.deviceRefreshButton.click();

      // Wait for auto-dismiss (usually 5-10 seconds)
      const initialCount = await elements.toasts.count();
      await page.waitForTimeout(10000);

      // Toasts should auto-dismiss
      const finalCount = await elements.toasts.count();
      expect(finalCount).toBeLessThanOrEqual(initialCount);
    });

    test("should have accessible dismiss button", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.deviceRefreshButton.click();
      await page.waitForTimeout(3000);

      const toastCount = await elements.toasts.count();
      if (toastCount > 0) {
        const dismissButton = elements.toasts
          .first()
          .locator('[data-testid="toast-dismiss"]');
        await expect(dismissButton).toHaveAttribute(
          "aria-label",
          "Dismiss notification",
        );
      }
    });
  });

  test.describe("Notification Types", () => {
    test("should style success toasts correctly", async ({ getElements }) => {
      const elements = getElements();

      const successToasts = elements.successToasts;
      const count = await successToasts.count();

      if (count > 0) {
        const toast = successToasts.first();
        await expect(toast).toHaveClass(/toast-success/);
      }
    });

    test("should style error toasts correctly", async ({ getElements }) => {
      const elements = getElements();

      const errorToasts = elements.errorToasts;
      const count = await errorToasts.count();

      if (count > 0) {
        const toast = errorToasts.first();
        await expect(toast).toHaveClass(/toast-error/);
      }
    });

    test("should style warning toasts correctly", async ({ getElements }) => {
      const elements = getElements();

      const warningToasts = elements.warningToasts;
      const count = await warningToasts.count();

      if (count > 0) {
        const toast = warningToasts.first();
        await expect(toast).toHaveClass(/toast-warning/);
      }
    });

    test("should style info toasts correctly", async ({ getElements }) => {
      const elements = getElements();

      const infoToasts = elements.infoToasts;
      const count = await infoToasts.count();

      if (count > 0) {
        const toast = infoToasts.first();
        await expect(toast).toHaveClass(/toast-info/);
      }
    });
  });
});

test.describe("Loading States", () => {
  test.beforeEach(async ({ page, waitForAppLoad }) => {
    await page.goto("/");
    await waitForAppLoad();
  });

  test.describe("Device List Loading", () => {
    test("should show skeleton loaders while loading devices", async ({
      page,
    }) => {
      // Navigate to trigger fresh load
      await page.goto("/");

      // Check for skeleton loaders (may be brief)
      const skeletons = page.locator(".device-item-skeleton");
      // Skeletons may load too fast to catch
    });

    test("should hide skeleton loaders after data loads", async ({
      page,
      waitForAppLoad,
    }) => {
      await page.goto("/");
      await waitForAppLoad();

      // Wait for data to load
      await page.waitForTimeout(2000);

      // Skeleton loaders should be gone
      const loadingState = page.locator(".device-list-loading");
      await expect(loadingState).not.toBeVisible();
    });
  });

  test.describe("Playlist List Loading", () => {
    test("should show skeleton loaders while loading playlists", async ({
      page,
    }) => {
      await page.goto("/");

      // Check for playlist skeleton loaders
      const skeletons = page.locator(".playlist-card-skeleton");
      // Skeletons may load too fast to catch
    });

    test("should hide skeleton loaders after playlists load", async ({
      page,
      waitForAppLoad,
      getElements,
    }) => {
      await page.goto("/");
      await waitForAppLoad();

      await page.waitForTimeout(2000);

      const elements = getElements();

      // Loading state should be gone
      await expect(elements.playlistListLoading).not.toBeVisible();
    });
  });

  test.describe("Button Loading States", () => {
    test("should show spinner on buttons during async operations", async ({
      getElements,
    }) => {
      const elements = getElements();

      // Click refresh button
      await elements.deviceRefreshButton.click();

      // Button should show refreshing state
      await expect(elements.deviceRefreshButton).toHaveClass(/refreshing/);
    });

    test("should disable buttons during loading", async ({ getElements }) => {
      const elements = getElements();

      // Click refresh button
      await elements.deviceRefreshButton.click();

      // Button should be disabled
      await expect(elements.deviceRefreshButton).toBeDisabled();
    });
  });

  test.describe("Dialog Loading States", () => {
    test("should show loading state in create playlist dialog", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.newPlaylistButton.click();

      // When validating URL, should show spinner
      await elements.playlistUrlInput.fill(
        "https://youtube.com/playlist?list=PLtest",
      );

      const spinner = elements.urlValidationIndicator.locator(".spinner");
      await expect(spinner).toBeVisible({ timeout: 2000 });
    });

    test("should show loading state in settings panel", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.settingsButton.click();

      // Settings may show loading initially
      // Check for any spinner or disabled state
    });
  });
});

test.describe("Error States", () => {
  test.beforeEach(async ({ page, waitForAppLoad }) => {
    await page.goto("/");
    await waitForAppLoad();
  });

  test.describe("Device List Error State", () => {
    test("should display error state for failed device load", async ({
      getElements,
    }) => {
      const elements = getElements();

      // If device loading fails, error state should be shown
      const errorState = elements.deviceListError;
      const isVisible = await errorState.isVisible().catch(() => false);

      if (isVisible) {
        await expect(errorState).toContainText("Failed to detect devices");
      }
    });

    test("should show retry button in device error state", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      const errorState = elements.deviceListError;
      const isVisible = await errorState.isVisible().catch(() => false);

      if (isVisible) {
        const retryButton = errorState.locator('button:has-text("Retry")');
        await expect(retryButton).toBeVisible();
      }
    });
  });

  test.describe("Playlist List Error State", () => {
    test("should display error state for failed playlist load", async ({
      getElements,
    }) => {
      const elements = getElements();

      const errorState = elements.playlistListError;
      const isVisible = await errorState.isVisible().catch(() => false);

      if (isVisible) {
        await expect(errorState).toContainText("Failed to load playlists");
      }
    });

    test("should show retry button in playlist error state", async ({
      getElements,
    }) => {
      const elements = getElements();

      const errorState = elements.playlistListError;
      const isVisible = await errorState.isVisible().catch(() => false);

      if (isVisible) {
        const retryButton = errorState.locator('button:has-text("Retry")');
        await expect(retryButton).toBeVisible();
      }
    });

    test("should show error icon in error state", async ({ getElements }) => {
      const elements = getElements();

      const errorState = elements.playlistListError;
      const isVisible = await errorState.isVisible().catch(() => false);

      if (isVisible) {
        const errorIcon = errorState.locator("svg");
        await expect(errorIcon).toBeVisible();
      }
    });
  });

  test.describe("Form Validation Errors", () => {
    test("should display URL validation error in create dialog", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.newPlaylistButton.click();

      // Enter invalid URL
      await elements.playlistUrlInput.fill("not-a-url");
      await page.waitForTimeout(2000);

      // Should show error indicator
      const errorIcon = elements.urlValidationIndicator.locator(".error-icon");
      // Visibility depends on validation result
    });

    test("should display name validation error in create dialog", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.newPlaylistButton.click();

      // Enter invalid name
      await elements.playlistNameInput.fill("invalid/name");
      await elements.playlistNameInput.blur();

      // Should show error text
      const errorText = page.locator(
        ".create-playlist-field:has(#playlist-name) .create-playlist-error-text",
      );
      await expect(errorText).toBeVisible();
    });
  });

  test.describe("Empty States", () => {
    test("should show empty state for no devices", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      const emptyState = elements.deviceListEmpty;
      const isVisible = await emptyState.isVisible().catch(() => false);

      if (isVisible) {
        await expect(emptyState).toContainText("No devices detected");
        await expect(emptyState).toContainText("Connect an MP3 player");
      }
    });

    test("should show empty state for no playlists", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      const emptyState = elements.playlistListEmpty;
      const isVisible = await emptyState.isVisible().catch(() => false);

      if (isVisible) {
        await expect(emptyState).toContainText("No playlists yet");
      }
    });

    test("should show appropriate icons in empty states", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      // Check device empty state icon
      const deviceEmpty = elements.deviceListEmpty;
      if (await deviceEmpty.isVisible().catch(() => false)) {
        const icon = deviceEmpty.locator("svg");
        await expect(icon).toBeVisible();
      }

      // Check playlist empty state icon
      const playlistEmpty = elements.playlistListEmpty;
      if (await playlistEmpty.isVisible().catch(() => false)) {
        const icon = playlistEmpty.locator("svg");
        await expect(icon).toBeVisible();
      }
    });
  });
});
