import { fireEvent, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, beforeEach, expect, vi } from "vitest";

/** U491a: the deletion question is a named `alertdialog`; these read it and answer it as a user does. */
export const questionDialog = (): Promise<HTMLElement> => screen.findByRole("alertdialog");

/** Press the bar's `Delete N` and wait for the question it opens. */
export const askToDelete = async (count: number): Promise<HTMLElement> => {
  fireEvent.click(screen.getByRole("button", { name: `Delete ${count}` }));

  return questionDialog();
};

/** Press the question's `Delete N`. */
export const confirmDeletion = async (): Promise<void> => {
  fireEvent.click(within(await questionDialog()).getByRole("button", { name: /^Delete \d+$/ }));
};

export const questionClosed = (): Promise<void> => waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());

/** jsdom has no layout, so Ark's focus trap finds nothing visible to focus; give every element a height for the test. Call once at the top of a test file. */
export const stubDialogLayout = (): void => {
  beforeEach(() => { vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(20); });
  afterEach(() => { vi.restoreAllMocks(); });
};
