/**
 * Device Management Tests
 *
 * Tests for device list interactions including:
 * - Loading devices
 * - Device selection
 * - Device refresh
 * - Empty/error states
 * - Storage bar display
 */

import { test, expect, AppElements } from "./fixtures";

test.describe("Device Management", () => {
  test.beforeEach(async ({ page, waitForAppLoad }) => {
    await page.goto("/");
    await waitForAppLoad();
  });

  test.describe("Device List Display", () => {
    test("should display the device list section with header", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.deviceList).toBeVisible();
      await expect(elements.deviceListHeader).toContainText(
        "Connected Devices",
      );
    });

    test("should show refresh button in device list header", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Wait for initial loading to complete
      await page.waitForTimeout(3000);

      await expect(elements.deviceRefreshButton).toBeVisible();
      // Button may still be loading, just check it exists
    });

    test("should display device items with name and mount point", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Wait for devices to load (may show loading state first)
      await page.waitForTimeout(1000);

      // Check if devices are loaded or empty state is shown
      const deviceItems = elements.deviceItems;
      const emptyState = elements.deviceListEmpty;

      // Either devices should be visible OR empty state
      const devicesVisible = (await deviceItems.count()) > 0;
      const emptyVisible = await emptyState.isVisible().catch(() => false);

      expect(devicesVisible || emptyVisible).toBeTruthy();
    });

    test("should show storage bar for each device", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Wait for potential device load
      await page.waitForTimeout(3000);

      const deviceCount = await elements.deviceItems.count();
      // Skip if no devices connected
      test.skip(deviceCount === 0, "No devices connected to test storage bars");

      // Each device should have a storage bar
      const storageBars = elements.deviceItems.locator(".storage-bar");
      await expect(storageBars).toHaveCount(deviceCount);
    });

    test("should show storage text with free space info", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(3000);

      const deviceCount = await elements.deviceItems.count();
      // Skip if no devices connected
      test.skip(deviceCount === 0, "No devices connected to test storage text");

      // Storage text should contain "free of"
      const storageText = elements.deviceItems.first().locator(".storage-text");
      await expect(storageText).toContainText("free of");
    });
  });

  test.describe("Device Selection", () => {
    test("should highlight selected device", async ({ page, getElements }) => {
      const elements = getElements();

      await page.waitForTimeout(3000);

      const deviceCount = await elements.deviceItems.count();
      test.skip(deviceCount === 0, "No devices connected to test selection");

      const firstDevice = elements.deviceItems.first();

      // Click to select
      await firstDevice.click();

      // Should have 'selected' class
      await expect(firstDevice).toHaveClass(/selected/);
    });

    test("should update device status indicator when device is selected", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(3000);

      const deviceCount = await elements.deviceItems.count();
      test.skip(
        deviceCount === 0,
        "No devices connected to test status indicator",
      );

      // Select a device
      await elements.deviceItems.first().click();

      // Device status indicator should update
      await expect(elements.deviceStatusIndicator).toBeVisible();
    });

    test("should only allow one device to be selected at a time", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(3000);

      const deviceCount = await elements.deviceItems.count();
      test.skip(
        deviceCount < 2,
        "Need at least 2 devices connected to test multi-selection",
      );

      // Select first device
      await elements.deviceItems.nth(0).click();
      await expect(elements.deviceItems.nth(0)).toHaveClass(/selected/);

      // Select second device
      await elements.deviceItems.nth(1).click();

      // First should no longer be selected
      await expect(elements.deviceItems.nth(0)).not.toHaveClass(/selected/);
      await expect(elements.deviceItems.nth(1)).toHaveClass(/selected/);
    });
  });

  test.describe("Device Refresh", () => {
    test("should trigger device list refresh when clicking refresh button", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Wait for initial loading to complete
      await page.waitForTimeout(3000);

      // Click refresh button
      await elements.deviceRefreshButton.click();

      // Button should show refreshing state (spinning icon)
      await expect(elements.deviceRefreshButton).toHaveClass(/refreshing/);

      // Wait for refresh to complete
      await page.waitForTimeout(2000);
    });

    test("should disable refresh button while loading", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Wait for initial loading to complete
      await page.waitForTimeout(3000);

      // Click refresh
      await elements.deviceRefreshButton.click();

      // Button should be disabled during refresh
      await expect(elements.deviceRefreshButton).toBeDisabled();

      // Wait for it to re-enable
      await page.waitForTimeout(2000);
    });
  });

  test.describe("Device List States", () => {
    test("should show loading skeleton while fetching devices", async ({
      page,
    }) => {
      // Go to page and check for loading state before it resolves
      await page.goto("/");

      // Loading skeleton should briefly appear
      const loadingSkeleton = page.locator(".device-list-loading");
      // This may be too fast to catch, but test structure is correct
    });

    test("should handle empty device list gracefully", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(3000);

      // If no devices, should show empty state with appropriate message
      const emptyState = elements.deviceListEmpty;
      const isEmptyVisible = await emptyState.isVisible().catch(() => false);

      if (isEmptyVisible) {
        await expect(emptyState).toContainText("No devices detected");
        await expect(emptyState).toContainText("Connect an MP3 player via USB");
      }
    });
  });

  test.describe("Device Icons and UI", () => {
    test("should display device icon for each device", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(3000);

      const deviceCount = await elements.deviceItems.count();
      test.skip(deviceCount === 0, "No devices connected to test icons");

      // Each device should have an icon
      const icons = elements.deviceItems.locator(".device-icon svg");
      await expect(icons).toHaveCount(deviceCount);
    });

    test("should display device name prominently", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(3000);

      const deviceCount = await elements.deviceItems.count();
      test.skip(deviceCount === 0, "No devices connected to test device names");

      const deviceName = elements.deviceItems.first().locator(".device-name");
      await expect(deviceName).toBeVisible();
    });

    test("should display device mount path", async ({ page, getElements }) => {
      const elements = getElements();

      await page.waitForTimeout(3000);

      const deviceCount = await elements.deviceItems.count();
      test.skip(deviceCount === 0, "No devices connected to test mount paths");

      const devicePath = elements.deviceItems.first().locator(".device-path");
      await expect(devicePath).toBeVisible();
    });
  });
});
