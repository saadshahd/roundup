import * as v from "valibot";

export const storedValue = <T>(key: string, schema: v.GenericSchema<unknown, T>) => ({
  read: (): T | null => {
    try {
      const value = window.localStorage.getItem(key);

      const parsed = v.safeParse(schema, value === null ? null : JSON.parse(value));

      return parsed.success ? parsed.output : null;
    } catch {
      return null;
    }
  },
  write: (value: T): string | null => {
    try {
      window.localStorage.setItem(key, JSON.stringify(value));

      return null;
    } catch (error) {
      return error instanceof Error ? error.message : String(error);
    }
  },
});
