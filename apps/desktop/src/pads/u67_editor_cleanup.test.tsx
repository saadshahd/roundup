import { cleanup, render, screen } from "@solidjs/testing-library";
import { Editor, EditorStatus } from "@milkdown/kit/core";
import { afterEach, expect, it, vi } from "vitest";
import PadRichEditor from "./PadRichEditor";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

it("u67_a_failed_editor_reports_creation_failure_without_destroying", async () => {
  const editor = Editor.make();
  vi.spyOn(Editor, "make").mockReturnValue(editor);
  vi.spyOn(editor, "create").mockRejectedValue(new Error("Editor could not open"));
  const destroy = vi.spyOn(editor, "destroy");
  const view = render(() => <PadRichEditor text="" onText={vi.fn()} onBlur={vi.fn()} onReady={vi.fn()} />);

  expect((await screen.findByRole("alert")).textContent).toContain("Editor could not open");
  view.unmount();
  expect(destroy).not.toHaveBeenCalled();
});

it.each([false, true])("u67_unmount_disposes_the_editor_without_a_retry_timer_ready_%s", async (ready) => {
  const editor = Editor.make();
  vi.spyOn(Editor, "make").mockReturnValue(editor);
  const create = vi.spyOn(editor, "create");
  const destroy = vi.spyOn(editor, "destroy");
  const onReady = vi.fn();
  const view = render(() => <PadRichEditor text="# Heading" onText={vi.fn()} onBlur={vi.fn()} onReady={onReady} />);

  try {
    if (ready) await create.mock.results[0]?.value;

    view.unmount();
    await create.mock.results[0]?.value;

    // Milkdown retries destroy after 50 ms if called during create; that timer can outlive jsdom.
    expect(editor.status).not.toBe(EditorStatus.Created);
    expect(destroy).toHaveBeenCalledTimes(1);
    await destroy.mock.results[0]?.value;
    expect(editor.status).toBe(EditorStatus.Destroyed);
    expect(onReady).toHaveBeenCalledTimes(ready ? 1 : 0);
  } finally {
    view.unmount();
    await Promise.all(destroy.mock.results.map((result) => result.value));
  }
});
