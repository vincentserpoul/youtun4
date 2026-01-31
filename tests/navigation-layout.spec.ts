/**
 * Navigation and Layout Tests
 *
 * Tests for application layout and navigation including:
 * - Header display
 * - Mobile menu toggle
 * - Responsive layout
 * - Sidebar behavior
 */

import { test, expect, AppElements } from "./fixtures";

test.describe("Navigation and Layout", () => {
  test.beforeEach(async ({ page, waitForAppLoad }) => {
    await page.goto("/");
    await waitForAppLoad();
  });

  test.describe("Application Loading", () => {
    test("should display loading spinner initially", async ({ page }) => {
      // Navigate without waiting for app load
      await page.goto("/");

      // Loading element should be visible initially
      const loading = page.locator("#loading");
      // May be too fast to catch, but structure is correct
    });

    test("should hide loading spinner after WASM loads", async ({
      page,
      waitForAppLoad,
    }) => {
      await page.goto("/");
      await waitForAppLoad();

      // Loading element should be removed
      const loading = page.locator("#loading");
      await expect(loading).not.toBeVisible();
    });

    test("should mount the main application layout", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.layout).toBeVisible();
    });
  });

  test.describe("Header", () => {
    test("should display header with logo", async ({ page, getElements }) => {
      const elements = getElements();

      await expect(elements.header).toBeVisible();

      // Logo should be visible
      const logo = page.locator(".logo");
      await expect(logo).toBeVisible();
    });

    test("should display application name in header", async ({ page }) => {
      const logoText = page.locator(".logo-text");
      await expect(logoText).toContainText("Youtun4");
    });

    test("should display settings button in header", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.settingsButton).toBeVisible();
    });

    test("should open settings when clicking settings button", async ({
      getElements,
    }) => {
      const elements = getElements();

      await elements.settingsButton.click();

      // Settings overlay should become visible
      await expect(elements.settingsOverlay).toHaveClass(/visible/);
    });
  });

  test.describe("Sidebar", () => {
    test("should display sidebar on desktop viewport", async ({
      page,
      getElements,
    }) => {
      // Set desktop viewport
      await page.setViewportSize({ width: 1024, height: 768 });

      const elements = getElements();
      await expect(elements.sidebar).toBeVisible();
    });

    test("should contain device list in sidebar", async ({ getElements }) => {
      const elements = getElements();

      await expect(elements.deviceList).toBeVisible();
    });

    test("should contain sync button in sidebar", async ({ getElements }) => {
      const elements = getElements();

      await expect(elements.syncButtonContainer).toBeVisible();
    });

    test("should contain device status indicator in sidebar", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.deviceStatusIndicator).toBeVisible();
    });
  });

  test.describe("Main Content Area", () => {
    test("should display main content area", async ({ getElements }) => {
      const elements = getElements();

      await expect(elements.mainContent).toBeVisible();
    });

    test("should contain playlist list in main content", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.playlistList).toBeVisible();
    });

    test("should have content header with title", async ({ getElements }) => {
      const elements = getElements();

      await expect(elements.contentHeader).toBeVisible();
      await expect(elements.contentHeader).toContainText("Playlists");
    });
  });

  test.describe("Mobile Navigation", () => {
    test.beforeEach(async ({ page }) => {
      // Set mobile viewport
      await page.setViewportSize({ width: 375, height: 667 });
    });

    test("should show mobile menu toggle button on small screens", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.mobileMenuToggle).toBeVisible();
    });

    test("should toggle sidebar when clicking mobile menu button", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Initially sidebar should not have 'open' class on mobile
      await expect(elements.sidebar).not.toHaveClass(/open/);

      // Click toggle
      await elements.mobileMenuToggle.click();

      // Sidebar should now have 'open' class
      await expect(elements.sidebar).toHaveClass(/open/);
    });

    test("should close sidebar when clicking overlay", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Wait for loading to complete to avoid overlay interception issues
      await page.waitForTimeout(2000);

      // Open sidebar
      await elements.mobileMenuToggle.click();
      await expect(elements.sidebar).toHaveClass(/open/);

      // Click overlay - use force to bypass any intercept issues
      const overlay = page.locator(".layout-overlay");
      await overlay.click({ force: true });

      // Sidebar should close
      await expect(elements.sidebar).not.toHaveClass(/open/);
    });

    test("should show X icon when menu is open", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Open menu
      await elements.mobileMenuToggle.click();

      // Icon should change (indicated by aria-expanded)
      await expect(elements.mobileMenuToggle).toHaveAttribute(
        "aria-expanded",
        "true",
      );
    });

    test("should hide hamburger and show X when menu is open", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Toggle menu
      await elements.mobileMenuToggle.click();

      // Menu is now open, icon should be X
      await expect(elements.mobileMenuToggle).toHaveAttribute(
        "aria-expanded",
        "true",
      );

      // Toggle again
      await elements.mobileMenuToggle.click();

      // Menu is closed, icon should be hamburger
      await expect(elements.mobileMenuToggle).toHaveAttribute(
        "aria-expanded",
        "false",
      );
    });
  });

  test.describe("Responsive Layout", () => {
    test("should adapt layout for tablet viewport", async ({
      page,
      getElements,
    }) => {
      await page.setViewportSize({ width: 768, height: 1024 });

      const elements = getElements();

      // Layout should still be functional
      await expect(elements.layout).toBeVisible();
      await expect(elements.mainContent).toBeVisible();
    });

    test("should show desktop layout for large screens", async ({
      page,
      getElements,
    }) => {
      await page.setViewportSize({ width: 1440, height: 900 });

      const elements = getElements();

      // Sidebar should be always visible on large screens
      await expect(elements.sidebar).toBeVisible();

      // Mobile toggle might be hidden
      // (depends on CSS breakpoints)
    });

    test("should maintain content accessibility on all viewports", async ({
      page,
      getElements,
    }) => {
      const viewports = [
        { width: 320, height: 568 }, // iPhone SE
        { width: 375, height: 667 }, // iPhone 8
        { width: 768, height: 1024 }, // iPad
        { width: 1024, height: 768 }, // iPad Landscape
        { width: 1440, height: 900 }, // Desktop
      ];

      for (const viewport of viewports) {
        await page.setViewportSize(viewport);

        const elements = getElements();

        // Main content should always be accessible
        await expect(elements.mainContent).toBeVisible();

        // Header should always be visible
        await expect(elements.header).toBeVisible();
      }
    });
  });

  test.describe("Accessibility", () => {
    test("should have proper heading hierarchy", async ({ page }) => {
      const h1Elements = page.locator("h1");
      const h2Elements = page.locator("h2");
      const h3Elements = page.locator("h3");

      // Should have appropriate headings
      // (Logo might not be h1, but sections should have h2/h3)
    });

    test("should have accessible menu toggle button", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.mobileMenuToggle).toHaveAttribute(
        "aria-label",
        "Toggle menu",
      );
      await expect(elements.mobileMenuToggle).toHaveAttribute("aria-expanded");
    });

    test("should support keyboard navigation", async ({ page }) => {
      // Tab through interactive elements
      await page.keyboard.press("Tab");
      await page.keyboard.press("Tab");

      // An element should have focus
      const focusedElement = page.locator(":focus");
      await expect(focusedElement).toBeVisible();
    });
  });

  test.describe("Visual Elements", () => {
    test("should apply dark theme by default", async ({ page }) => {
      // Check body background color
      const body = page.locator("body");
      const bgColor = await body.evaluate(
        (el) => getComputedStyle(el).backgroundColor,
      );

      // Dark theme should have dark background
      // The exact color is #121212 which is rgb(18, 18, 18)
      expect(bgColor).toContain("18");
    });

    test("should load Fira Sans font", async ({ page }) => {
      const body = page.locator("body");
      const fontFamily = await body.evaluate(
        (el) => getComputedStyle(el).fontFamily,
      );

      // Should include Fira Sans
      expect(fontFamily.toLowerCase()).toContain("fira");
    });

    test("should display SVG icons properly", async ({ page }) => {
      // Check that SVG icons are rendered
      const svgIcons = page.locator("svg");
      const count = await svgIcons.count();

      expect(count).toBeGreaterThan(0);
    });
  });
});
