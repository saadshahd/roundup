import { describe, expect, it } from "vitest";
import { createFakeApp } from "./fakeApp";

describe("u1 the fake App", () => {
  it.each(["rail.tree", "terminal.list", "todo.list", "pad.list"] as const)(
    "u1_the_fake_answers_%s_with_an_empty_list_by_default",
    async (method) => {
      expect(await createFakeApp().rpc(method, null)).toEqual([]);
    },
  );

  it("u1_the_fake_fails_a_method_no_test_opted_into", async () => {
    await expect(createFakeApp().rpc("todo.get", { id: 1 })).rejects.toMatchObject({
      code: -32601,
      message: "no handler for todo.get",
    });
  });
});
