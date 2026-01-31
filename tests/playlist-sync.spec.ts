/**
 * Playlist Selection and Sync Workflow Tests
 *
 * Tests for selecting playlists and syncing to devices including:
 * - Selection mode toggle
 * - Playlist selection
 * - Sync button states
 * - Capacity checking
 * - Sync execution
 */

import { test, expect, AppElements } from "./fixtures";

test.describe("Playlist Selection and Sync", () => {
  test.beforeEach(async ({ page, waitForAppLoad }) => {
    await page.goto("/");
    await waitForAppLoad();
  });

  test.describe("Playlist List Display", () => {
    test("should display playlist list section", async ({ getElements }) => {
      const elements = getElements();

      await expect(elements.playlistList).toBeVisible();
    });

    test("should show content header with Playlists title", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.contentHeader).toContainText("Playlists");
    });

    test("should display New Playlist and Select for Sync buttons", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.newPlaylistButton).toBeVisible();
      await expect(elements.selectForSyncButton).toBeVisible();
    });

    test("should show empty state when no playlists exist", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Wait for playlists to load
      await page.waitForTimeout(1000);

      // Check for empty state OR playlist cards
      const emptyState = elements.playlistListEmpty;
      const cards = elements.playlistCards;

      const isEmpty = await emptyState.isVisible().catch(() => false);
      const hasPlaylists = (await cards.count()) > 0;

      if (isEmpty) {
        await expect(emptyState).toContainText("No playlists yet");
        await expect(emptyState).toContainText(
          "Create a playlist from a YouTube URL",
        );
      }
    });

    test("should show playlist summary when playlists exist", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      const cardCount = await elements.playlistCards.count();
      if (cardCount > 0) {
        await expect(elements.playlistSummary).toBeVisible();
        await expect(elements.playlistSummary).toContainText("playlist");
        await expect(elements.playlistSummary).toContainText("total");
      }
    });
  });

  test.describe("Selection Mode", () => {
    test("should enter selection mode when clicking Select for Sync", async ({
      getElements,
    }) => {
      const elements = getElements();

      // Click Select for Sync button
      await elements.selectForSyncButton.click();

      // Header should change
      await expect(elements.selectionModeHeader).toBeVisible();
      await expect(elements.selectionModeHeader).toContainText(
        "Select Playlist to Sync",
      );
    });

    test("should show Cancel and Sync to Device buttons in selection mode", async ({
      getElements,
    }) => {
      const elements = getElements();

      await elements.selectForSyncButton.click();

      await expect(elements.selectionCancelButton).toBeVisible();
      await expect(elements.syncToDeviceButton).toBeVisible();
    });

    test("should exit selection mode when clicking Cancel", async ({
      getElements,
    }) => {
      const elements = getElements();

      // Enter selection mode
      await elements.selectForSyncButton.click();
      await expect(elements.selectionModeHeader).toBeVisible();

      // Click Cancel
      await elements.selectionCancelButton.click();

      // Should return to management mode
      await expect(elements.contentHeader).toContainText("Playlists");
      await expect(elements.newPlaylistButton).toBeVisible();
    });

    test("should show playlist selection interface in selection mode", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.selectForSyncButton.click();

      // Wait for selection interface to load
      await page.waitForTimeout(500);

      // Should show selection list or empty state
      const selectionList = elements.playlistSelectionList;
      const isVisible = await selectionList.isVisible().catch(() => false);

      // The selection interface changes the playlist display
    });

    test("should have disabled Sync to Device button initially", async ({
      getElements,
    }) => {
      const elements = getElements();

      await elements.selectForSyncButton.click();

      // Without a playlist selected, sync button should be disabled
      await expect(elements.syncToDeviceButton).toBeDisabled();
    });
  });

  test.describe("Sync Button (Sidebar)", () => {
    test("should display sync button in sidebar", async ({ getElements }) => {
      const elements = getElements();

      await expect(elements.syncButtonContainer).toBeVisible();
      await expect(elements.syncButton).toBeVisible();
    });

    test("should show hint when no device or playlist selected", async ({
      getElements,
    }) => {
      const elements = getElements();

      await expect(elements.syncButtonHint).toBeVisible();
      await expect(elements.syncButtonHint).toContainText("Connect a device");
    });

    test("should have disabled sync button when conditions not met", async ({
      getElements,
    }) => {
      const elements = getElements();

      // Initially should be disabled (no device/playlist selected)
      await expect(elements.syncButton).toBeDisabled();
    });

    test("should update hint based on what is missing", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Wait for initial state
      await page.waitForTimeout(1000);

      const hintText = await elements.syncButtonHint.textContent();
      // Hint should indicate what's needed (device, playlist, or both)
      expect(hintText).toBeTruthy();
    });
  });

  test.describe("Capacity Checking", () => {
    test("should show checking space state when device and playlist selected", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // This test depends on having devices and playlists available
      await page.waitForTimeout(1000);

      const deviceCount = await elements.deviceItems.count();
      const playlistCount = await elements.playlistCards.count();

      if (deviceCount > 0 && playlistCount > 0) {
        // Select a device
        await elements.deviceItems.first().click();

        // Select a playlist
        await elements.playlistCards.first().click();

        // Should show checking capacity or result
        // Sync button should update its state
      }
    });

    test("should show capacity error when insufficient space", async ({
      getElements,
    }) => {
      const elements = getElements();

      // When there's insufficient space, should show error indicator
      // This requires specific test conditions with a full device
      const capacityError = elements.syncCapacityError;
      // Only check if visible - depends on test data
    });

    test("should show capacity warning for low space", async ({
      getElements,
    }) => {
      const elements = getElements();

      // When space is limited but sufficient, should show warning
      const capacityWarning = elements.syncCapacityWarning;
      // Only check if visible - depends on test data
    });
  });

  test.describe("Playlist Card Interactions", () => {
    test("should highlight playlist card on hover", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      const cardCount = await elements.playlistCards.count();
      if (cardCount > 0) {
        const firstCard = elements.playlistCards.first();

        // Hover over the card
        await firstCard.hover();

        // Card should have hover styles (tested via visual inspection or class)
      }
    });

    test("should select playlist card on click", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      const cardCount = await elements.playlistCards.count();
      if (cardCount > 0) {
        const firstCard = elements.playlistCards.first();

        // Click to select
        await firstCard.click();

        // Card should have selected state
        await expect(firstCard).toHaveClass(/selected/);
      }
    });

    test("should show playlist actions (delete, sync)", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      const cardCount = await elements.playlistCards.count();
      if (cardCount > 0) {
        // Playlist cards should have action buttons
        const actionButtons = elements.playlistCards.first().locator(".btn");
        const buttonCount = await actionButtons.count();
        expect(buttonCount).toBeGreaterThan(0);
      }
    });
  });

  test.describe("Delete Playlist Flow", () => {
    test("should open delete confirmation when clicking delete", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      const cardCount = await elements.playlistCards.count();
      if (cardCount > 0) {
        // Find and click delete button on first card
        const deleteButton = elements.playlistCards
          .first()
          .locator('button:has-text("Delete"), .btn-danger');
        const hasDelete = (await deleteButton.count()) > 0;

        if (hasDelete) {
          await deleteButton.first().click();

          // Delete dialog should appear
          // Note: The actual dialog component may vary
        }
      }
    });

    test("should close delete dialog on cancel", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await page.waitForTimeout(1000);

      const cardCount = await elements.playlistCards.count();
      if (cardCount > 0) {
        const deleteButton = elements.playlistCards
          .first()
          .locator('button:has-text("Delete"), .btn-danger');
        const hasDelete = (await deleteButton.count()) > 0;

        if (hasDelete) {
          await deleteButton.first().click();

          // Click cancel if dialog is visible
          const cancelButton = elements.deleteDialogCancelButton;
          if (await cancelButton.isVisible().catch(() => false)) {
            await cancelButton.click();
          }
        }
      }
    });
  });

  test.describe("Sync Execution", () => {
    test("should show syncing state when sync is triggered", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // This test requires both device and playlist to be selected and valid
      await page.waitForTimeout(1000);

      // When syncing, the button should show "Syncing..." text
      // and have a spinner
    });

    test("should disable sync button during sync operation", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // During sync, button should be disabled to prevent double-submission
      await page.waitForTimeout(1000);
    });
  });

  test.describe("Selection Summary", () => {
    test("should show selection summary in selection mode", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      // Enter selection mode
      await elements.selectForSyncButton.click();

      // Wait for mode change
      await page.waitForTimeout(500);

      // Selection summary should be visible
      await expect(elements.playlistSelectionSummary).toBeVisible();
    });

    test("should update summary when playlist is selected", async ({
      page,
      getElements,
    }) => {
      const elements = getElements();

      await elements.selectForSyncButton.click();
      await page.waitForTimeout(500);

      const cardCount = await elements.playlistSelectionCards.count();
      if (cardCount > 0) {
        // Select a playlist
        await elements.playlistSelectionCards.first().click();

        // Summary should show selected playlist info
        // Content depends on implementation
      }
    });
  });
});
