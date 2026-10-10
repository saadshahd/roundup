import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { CHECK_IDS, runChecks } from "./checks";
import { withStylesheets } from "./contrast";
import { seedApp } from "./seeds";

afterEach(() => {
  cleanup();
  document.querySelectorAll("style[data-u137]").forEach((sheet) => sheet.remove());
});

const mountSeed = () => {
  const controls = seedApp("agents-10", Date.now());
  render(() => <App app={controls.app} reducedMotion={() => false} clock={Date.now} />);
};

const addSheet = (css: string) => {
  const sheet = document.head.appendChild(document.createElement("style"));
  sheet.dataset.u137 = "";
  sheet.textContent = css;
};

describe("u137 one module measures the checks", () => {
  it("u137_every_id_d1_to_d10_is_present_with_status_value_and_selector", async () => {
    await withStylesheets(async () => {
      mountSeed();
      const checks = runChecks();

      expect(Object.keys(checks)).toEqual(["D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10"]);
      expect(CHECK_IDS).toEqual(Object.keys(checks));

      for (const check of Object.values(checks)) {
        expect(["pass", "fail"]).toContain(check.status);
        expect(check).toHaveProperty("value");
        expect(check.selector).toEqual(expect.any(String));
      }
    });
  });

  it("u137_a_stylesheet_with_a_literal_colour_makes_d1_fail", () => {
    document.head.querySelectorAll("style").forEach((sheet) => sheet.remove());
    document.body.innerHTML = "<p class='rail-row'>x</p>";
    addSheet(".rail-row { color: #ff00ff; }");

    const d1 = runChecks().D1;

    expect(d1.status).toBe("fail");
    expect(d1.value).toEqual(["color: rgb(255, 0, 255)"]);
    expect(d1.selector).toBe(".rail-row");
  });

  it("u136_d1_exempts_terminal_rules_and_still_catches_others", () => {
    document.head.querySelectorAll("style").forEach((sheet) => sheet.remove());
    document.body.innerHTML = "<p class='rail-row'>x</p>";
    addSheet(".xterm .composition-view, .xterm .x { color: #ffffff; transition: opacity 100ms linear; }");

    expect(runChecks().D1.status).toBe("pass");

    addSheet(".rail-row, .xterm .x { color: #ff00ff; }");

    expect(runChecks().D1.status).toBe("fail");
  });

  it("u137_an_inline_literal_colour_makes_d1_fail", () => {
    document.body.innerHTML = `<p style="color: #ff00ff">x</p>`;
    addSheet("p { margin: 0; }");

    expect(runChecks().D1.status).toBe("fail");
  });

  it("u137_a_check_it_cannot_measure_fails_as_not_measurable", () => {
    document.body.innerHTML = "";

    const checks = runChecks();

    for (const id of ["D1", "D2", "D3", "D5", "D6", "D8", "D9"] as const) {
      expect(checks[id].status).toBe("fail");
      expect(checks[id].value).toBe("not measurable");
    }

    expect(checks.D10.status).toBe("fail");
  });

  it("u137_the_result_parses_under_the_checks_json_shape_delta_reads", async () => {
    await withStylesheets(async () => {
      mountSeed();
      // SAFETY: the test reads each entry as the `checks.json` shape and asserts it below.
      const parsed = JSON.parse(JSON.stringify(runChecks())) as Record<string, { status: string; value: unknown; selector: string }>;

      for (const id of CHECK_IDS) {
        const entry = parsed[id]!;

        expect(["pass", "fail"]).toContain(entry.status);
        expect(entry.value).not.toBeUndefined();
      }
    });
  });

  it("u137_reading_the_page_changes_nothing_the_user_sees", async () => {
    await withStylesheets(async () => {
      mountSeed();
      const row = (await screen.findAllByRole("treeitem"))[0]!;
      row.focus();
      const before = [document.body.innerHTML, document.head.innerHTML, String(document.activeElement === row)];

      runChecks();

      expect([document.body.innerHTML, document.head.innerHTML, String(document.activeElement === row)]).toEqual(before);
    });
  });
});
