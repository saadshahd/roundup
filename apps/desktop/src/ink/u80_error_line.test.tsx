import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, expect, it } from "vitest";
import { ErrorLine } from "./ErrorLine";

afterEach(cleanup);

it("u80_a_failure_announces_one_line_and_keeps_the_whole_message_in_its_title", () => {
  render(() => <ErrorLine message={"could not open\r\ndetails\nlast line"} />);
  const line = screen.getByRole("alert");
  expect(line.textContent).toBe("could not open");
  expect(line.title).toBe("could not open\r\ndetails\nlast line");
  expect(line.className).toBe("ink");
  expect(line.querySelectorAll("svg")).toHaveLength(1);
});
