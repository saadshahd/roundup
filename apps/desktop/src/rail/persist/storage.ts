import * as v from "valibot";
import { storedValue } from "../../state/storage";

const layoutSchema = v.object({
  selected: v.nullable(v.string()),
  collapsed: v.array(v.string()),
});

type SavedLayout = v.InferOutput<typeof layoutSchema>;

export type RailStorage = {
  read(): SavedLayout | null;
  write(layout: SavedLayout): string | null;
};

export const railStorage = (path: string): RailStorage => storedValue(`roundup:rail:${path}`, layoutSchema);
