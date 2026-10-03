import * as v from "valibot";

const layoutSchema = v.object({
  selected: v.nullable(v.string()),
  collapsed: v.array(v.string()),
});

type SavedLayout = v.InferOutput<typeof layoutSchema>;

export type RailStorage = {
  read(): SavedLayout | null;
  write(layout: SavedLayout): string | null;
};

export const railStorage = (path: string): RailStorage => {
  const key = `roundup:rail:${path}`;

  return {
    read: () => {
      try {
        const value = window.localStorage.getItem(key);
        const parsed = v.safeParse(layoutSchema, value === null ? null : JSON.parse(value));

        return parsed.success ? parsed.output : null;
      } catch {
        return null;
      }
    },
    write: (layout) => {
      try {
        window.localStorage.setItem(key, JSON.stringify(layout));

        return null;
      } catch (error) {
        return error instanceof Error ? error.message : String(error);
      }
    },
  };
};
